//! Per-feed credentials are deliberately absent from list/export/log payloads.
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, COOKIE};
use serde::{Deserialize, Serialize};

use crate::error::{ApiResult, AppError};

#[derive(Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct RequestConfig {
  pub headers: Vec<RequestHeader>,
  // Compatibility with configurations saved before Cookie became a regular header row.
  pub cookie: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RequestHeader {
  pub name: String,
  pub value: String,
}

impl RequestConfig {
  pub fn from_json(value: &str) -> ApiResult<Self> {
    serde_json::from_str(value).map_err(|_| AppError::internal())
  }

  pub fn to_json(&self) -> ApiResult<String> {
    self.to_headers()?;
    serde_json::to_string(self).map_err(|_| AppError::internal())
  }

  pub fn to_headers(&self) -> ApiResult<HeaderMap> {
    if self.headers.len() > 32
      || self.cookie.len() + self.headers.iter().map(|h| h.name.len() + h.value.len()).sum::<usize>() > 16384
    {
      return Err(AppError::invalid("请求头最多 32 项，含 Cookie 总长度不能超过 16 KB"));
    }
    let mut result = HeaderMap::new();
    for header in &self.headers {
      let name = HeaderName::from_bytes(header.name.as_bytes()).map_err(|_| AppError::invalid("请求头名称无效"))?;
      // Transport, proxy and cache validators belong to FetchClient/the scheduler.
      if matches!(
        name.as_str(),
        "host"
          | "cookie2"
          | "connection"
          | "content-length"
          | "transfer-encoding"
          | "te"
          | "trailer"
          | "upgrade"
          | "keep-alive"
          | "accept-encoding"
          | "expect"
          | "range"
          | "if-range"
          | "if-match"
          | "if-unmodified-since"
          | "if-none-match"
          | "if-modified-since"
      ) || name.as_str().starts_with("proxy-")
        || name.as_str().starts_with("sec-")
      {
        return Err(AppError::invalid("该请求头由系统管理，不能自定义覆盖"));
      }
      if result.contains_key(&name) {
        return Err(AppError::invalid("请求头名称重复（不区分大小写）"));
      }
      let mut value =
        HeaderValue::from_str(&header.value).map_err(|_| AppError::invalid("请求头值无效，不能包含换行或控制字符"))?;
      value.set_sensitive(true);
      result.insert(name, value);
    }
    if !self.cookie.is_empty() {
      if result.contains_key(COOKIE) {
        return Err(AppError::invalid("Cookie 配置重复，请仅保留一个 Cookie 请求头"));
      }
      let mut cookie =
        HeaderValue::from_str(&self.cookie).map_err(|_| AppError::invalid("Cookie 无效，请填写单行 Cookie 值"))?;
      cookie.set_sensitive(true);
      result.insert(COOKIE, cookie);
    }
    Ok(result)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  fn config(name: &str, value: &str) -> RequestConfig {
    RequestConfig {
      headers: vec![RequestHeader {
        name: name.into(),
        value: value.into(),
      }],
      cookie: "session=example".into(),
    }
  }
  #[test]
  fn cookie_is_a_regular_header_and_legacy_values_remain_supported() {
    let mut current = config("cOoKiE", "session=new");
    assert!(current.to_headers().is_err()); // Ambiguous legacy + header configuration.
    current.cookie.clear();
    let headers = current.to_headers().unwrap();
    assert_eq!(headers["cookie"], "session=new");
    assert!(headers["cookie"].is_sensitive());
    let round_trip = RequestConfig::from_json(&current.to_json().unwrap()).unwrap();
    assert_eq!(round_trip.to_headers().unwrap()["cookie"], "session=new");
    current.headers.push(RequestHeader {
      name: "Cookie".into(),
      value: "another=value".into(),
    });
    assert!(current.to_headers().is_err());
  }
  #[test]
  fn validates_headers_and_hides_values_from_debug() {
    let headers = config("Authorization", "Bearer example").to_headers().unwrap();
    assert_eq!(headers["authorization"], "Bearer example");
    assert_eq!(headers["cookie"], "session=example");
    assert!(!format!("{headers:?}").contains("example"));
    for name in [
      "Host",
      "Connection",
      "Proxy-Authorization",
      "Content-Length",
      "If-None-Match",
      "Accept-Encoding",
      "Range",
    ] {
      assert!(config(name, "example").to_headers().is_err());
    }
    for value in ["ok\r\nX-Injected: yes", "bad\0value"] {
      assert!(config("X-Test", value).to_headers().is_err());
      assert!(RequestConfig {
        cookie: value.into(),
        ..Default::default()
      }
      .to_headers()
      .is_err());
    }
    let mut duplicate = config("User-Agent", "Reader");
    duplicate.headers.push(RequestHeader {
      name: "user-agent".into(),
      value: "Other".into(),
    });
    assert!(duplicate.to_headers().is_err());
    assert!(config("X-Test", &"x".repeat(16385)).to_headers().is_err());
  }
}
