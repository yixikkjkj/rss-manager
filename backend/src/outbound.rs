//! The only network exit for feeds, OPML probes and article extraction.
//! Resolve every hop, reject non-public addresses, and pin the checked DNS result.
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use reqwest::header::{HeaderMap, LOCATION};
use reqwest::{StatusCode, Url};
use tokio::sync::Semaphore;

use crate::error::{ApiResult, AppError};

const MAX_BYTES: usize = 5 * 1024 * 1024;
const MAX_REDIRECTS: usize = 5;
const TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct FetchClient {
  permits: Arc<Semaphore>,
  #[cfg(test)]
  fixture: Option<Arc<TestResponse>>,
}

#[cfg(test)]
type TestResponse = dyn Fn(&str, HeaderMap) -> ApiResult<FetchResponse> + Send + Sync;

pub struct FetchResponse {
  pub status: StatusCode,
  pub headers: HeaderMap,
  pub body: Vec<u8>,
  pub final_url: Url,
}

impl FetchClient {
  pub fn new() -> Self {
    Self {
      permits: Arc::new(Semaphore::new(5)),
      #[cfg(test)]
      fixture: None,
    }
  }

  #[cfg(test)]
  pub fn with_fixture(fixture: impl Fn(&str, HeaderMap) -> ApiResult<FetchResponse> + Send + Sync + 'static) -> Self {
    Self {
      fixture: Some(Arc::new(fixture)),
      ..Self::new()
    }
  }

  pub async fn get(&self, raw: &str, headers: HeaderMap) -> ApiResult<FetchResponse> {
    let url = validate_url(raw)?;
    let _permit = self.permits.acquire().await.map_err(|_| AppError::internal())?;
    #[cfg(test)]
    if let Some(fixture) = &self.fixture {
      return fixture(raw, headers);
    }
    tokio::time::timeout(TIMEOUT, self.request(url, headers))
      .await
      .map_err(|_| AppError::fetch_failed("抓取超时"))?
  }

  async fn request(&self, mut url: Url, mut headers: HeaderMap) -> ApiResult<FetchResponse> {
    for hop in 0..=MAX_REDIRECTS {
      let host = url.host_str().ok_or_else(AppError::bad_request)?;
      let addresses: Vec<SocketAddr> = match url.host() {
        Some(url::Host::Ipv4(ip)) => vec![SocketAddr::new(
          IpAddr::V4(ip),
          url.port_or_known_default().unwrap_or(80),
        )],
        Some(url::Host::Ipv6(ip)) => vec![SocketAddr::new(
          IpAddr::V6(ip),
          url.port_or_known_default().unwrap_or(80),
        )],
        _ => tokio::net::lookup_host((host, url.port_or_known_default().unwrap_or(80)))
          .await
          .map_err(|_| AppError::fetch_failed("DNS 解析失败"))?
          .collect(),
      };
      validate_addresses(&addresses)?;
      // Never use ambient proxies: they could resolve a different destination.
      let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(TIMEOUT)
        .user_agent(concat!("rss-manager/", env!("CARGO_PKG_VERSION")))
        .resolve_to_addrs(host, &addresses)
        .build()
        .map_err(|_| AppError::internal())?;
      let mut response = client
        .get(url.clone())
        .headers(headers.clone())
        .send()
        .await
        .map_err(|_| AppError::fetch_failed("无法连接订阅或文章地址"))?;
      let status = response.status();
      if matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308) {
        if hop == MAX_REDIRECTS {
          return Err(AppError::fetch_failed("重定向次数过多"));
        }
        let target = response
          .headers()
          .get(LOCATION)
          .and_then(|v| v.to_str().ok())
          .ok_or_else(|| AppError::fetch_failed("重定向地址无效"))?;
        let next = url.join(target).map_err(|_| AppError::unsafe_url())?;
        validate_url(next.as_str())?;
        strip_redirect_headers(&url, &next, &mut headers);
        url = next;
        continue;
      }
      let response_headers = response.headers().clone();
      if response.content_length().is_some_and(|n| n > MAX_BYTES as u64) {
        return Err(AppError::fetch_failed("响应内容超过 5 MB"));
      }
      let mut body = Vec::new();
      while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| AppError::fetch_failed("读取响应失败"))?
      {
        if body.len() + chunk.len() > MAX_BYTES {
          return Err(AppError::fetch_failed("响应内容超过 5 MB"));
        }
        body.extend_from_slice(&chunk);
      }
      return Ok(FetchResponse {
        status,
        headers: response_headers,
        body,
        final_url: url,
      });
    }
    Err(AppError::internal())
  }
}

fn strip_redirect_headers(from: &Url, to: &Url, headers: &mut HeaderMap) {
  // Custom headers may contain credentials under any name, not just Authorization.
  if from.origin() != to.origin() {
    headers.clear();
  }
}

/// Normalize only URL syntax. Do not collapse slashes inside paths or query values.
pub fn normalize_url(raw: &str) -> ApiResult<String> {
  let raw = raw.trim();
  let value = raw
    .strip_prefix("feed://")
    .map(|s| format!("http://{s}"))
    .unwrap_or_else(|| raw.to_owned());
  let mut url = validate_url(&value)?;
  url.set_fragment(None);
  Ok(url.to_string())
}

pub fn validate_url(raw: &str) -> ApiResult<Url> {
  let url = Url::parse(raw).map_err(|_| AppError::unsafe_url())?;
  if !matches!(url.scheme(), "http" | "https")
    || !url.username().is_empty()
    || url.password().is_some()
    || !matches!(url.port_or_known_default(), Some(80 | 443))
  {
    return Err(AppError::unsafe_url());
  }
  match url.host() {
    Some(url::Host::Ipv4(ip)) if !public_v4(ip) => return Err(AppError::unsafe_url()),
    Some(url::Host::Ipv6(ip)) if !public_v6(ip) => return Err(AppError::unsafe_url()),
    Some(url::Host::Domain(host)) if host == "localhost" || host.ends_with(".localhost") => {
      return Err(AppError::unsafe_url())
    },
    None => return Err(AppError::unsafe_url()),
    _ => {},
  }
  Ok(url)
}

fn validate_addresses(addresses: &[SocketAddr]) -> ApiResult<()> {
  if addresses.is_empty()
    || addresses.iter().any(|a| !match a.ip() {
      IpAddr::V4(ip) => public_v4(ip),
      IpAddr::V6(ip) => public_v6(ip),
    })
  {
    return Err(AppError::unsafe_url());
  }
  Ok(())
}

fn public_v4(ip: Ipv4Addr) -> bool {
  let [a, b, c, _] = ip.octets();
  !(matches!(a, 0 | 10 | 127)
    || a >= 224
    || (a == 100 && (64..=127).contains(&b))
    || (a == 169 && b == 254)
    || (a == 172 && (16..=31).contains(&b))
    || (a == 192 && (b == 168 || (b == 0 && (c == 0 || c == 2)) || (b == 88 && c == 99)))
    || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
    || (a == 203 && b == 0 && c == 113))
}

fn public_v6(ip: Ipv6Addr) -> bool {
  // Fail closed outside global unicast, including mapped IPv4, NAT64 and local ranges.
  let s = ip.segments();
  (s[0] & 0xe000) == 0x2000
    && !(s[0] == 0x2001 && (s[1] <= 0x01ff || s[1] == 0x0db8))
    && s[0] != 0x2002
    && !(s[0] == 0x3fff && s[1] <= 0x0fff)
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn redirects_keep_same_origin_headers_but_strip_all_custom_credentials_on_origin_change() {
    let source = Url::parse("https://example.com/feed").unwrap();
    let mut headers = HeaderMap::new();
    headers.insert("cookie", "session=test".parse().unwrap());
    headers.insert("x-api-key", "test".parse().unwrap());
    headers.insert("authorization", "Bearer test".parse().unwrap());
    strip_redirect_headers(&source, &source.join("/rss").unwrap(), &mut headers);
    assert_eq!(headers.len(), 3);
    for target in [
      "https://other.example/feed",
      "http://example.com/feed",
      "https://example.com:444/feed",
    ] {
      let mut next = headers.clone();
      strip_redirect_headers(&source, &Url::parse(target).unwrap(), &mut next);
      assert!(next.is_empty());
    }
  }
  #[test]
  fn blocks_local_special_and_alternate_ip_notation() {
    for url in [
      "http://127.0.0.1",
      "http://2130706433",
      "http://0x7f000001",
      "http://10.1.2.3",
      "http://169.254.169.254",
      "http://100.64.0.1",
      "http://[::1]",
      "http://[::ffff:127.0.0.1]",
      "http://[fc00::1]",
      "http://[64:ff9b::7f00:1]",
      "file:///etc/passwd",
      "http://example.com:8080",
      "https://user:pass@example.com",
      "http://localhost",
    ] {
      assert!(validate_url(url).is_err(), "{url}");
    }
    assert!(validate_url("https://example.com/feed").is_ok());
    assert!(validate_url("https://[2606:4700:4700::1111]/").is_ok());
  }
  #[test]
  fn rejects_mixed_dns_answers() {
    assert!(validate_addresses(&["1.1.1.1:443".parse().unwrap(), "10.0.0.1:443".parse().unwrap()]).is_err());
    assert!(validate_addresses(&[]).is_err());
  }
  #[test]
  fn preserves_query_urls() {
    assert_eq!(
      normalize_url("https://example.com/feed?url=https://other.example/a//b").unwrap(),
      "https://example.com/feed?url=https://other.example/a//b"
    );
  }
}
