//! Diagnose failed responses without exposing response bodies or credentials.
use feed_rs::model::Feed;
use reqwest::header::CONTENT_TYPE;
use scraper::{Html, Selector};

use super::FetchOutcome;
use crate::outbound::FetchResponse;

pub fn failure(result: &str, status: Option<i64>, message: &str) -> FetchOutcome {
  FetchOutcome {
    result: result.into(),
    http_status: status,
    new_articles: 0,
    error_message: Some(message.into()),
  }
}

pub fn parse_response(response: &FetchResponse) -> Result<Feed, FetchOutcome> {
  let status = response.status.as_u16() as i64;
  let fail = |kind, message| failure(kind, Some(status), message);
  if status == 401 {
    return Err(fail("auth_required", "源站要求身份认证，请检查 Cookie 或 Authorization 请求头"));
  }
  if status == 429 {
    return Err(fail("rate_limited", "源站限制请求频率，请稍后重试或增大抓取间隔"));
  }
  // Parse first: an actual feed can contain articles about CAPTCHAs or login.
  if response.status.is_success() {
    if let Ok(feed) = feed_rs::parser::parse(response.body.as_slice()) {
      return Ok(feed);
    }
  }
  let prefix = String::from_utf8_lossy(&response.body[..response.body.len().min(65536)]).to_lowercase();
  let content_type = response
    .headers
    .get(CONTENT_TYPE)
    .and_then(|v| v.to_str().ok())
    .unwrap_or("");
  let html = content_type.to_ascii_lowercase().contains("text/html")
    || content_type.to_ascii_lowercase().contains("application/xhtml+xml")
    || prefix.contains("<html")
    || prefix.contains("<!doctype html");
  if html {
    if [
      "captcha",
      "cf-chl-",
      "challenge-platform",
      "verify you are human",
      "checking your browser",
      "checking if the site connection is secure",
      "_waftokenid",
      "机器人检测",
      "人机验证",
      "安全验证",
    ]
    .iter()
    .any(|marker| prefix.contains(marker))
    {
      return Err(fail("challenge_required", "疑似源站人机验证或防护页面，请在浏览器确认访问状态并检查 Cookie"));
    }
    let document = Html::parse_document(&prefix);
    let password = Selector::parse("input[type=password]").expect("static selector");
    if document.select(&password).next().is_some() {
      return Err(fail("auth_required", "源站返回疑似登录页面，请检查登录状态及 Cookie"));
    }
  }
  if status == 403 {
    return Err(fail("access_denied", "源站拒绝访问（HTTP 403），可能与访问权限或防护策略有关"));
  }
  if !response.status.is_success() {
    return Err(fail("http_error", &format!("源站返回 HTTP {status}")));
  }
  if html {
    return Err(fail("unexpected_content", "源站返回 HTML 网页，未得到可解析的 Feed，请检查订阅地址或访问配置"));
  }
  Err(fail("parse_error", "响应不是有效的 RSS、Atom 或 JSON Feed，或内容不完整"))
}

#[cfg(test)]
mod tests {
  use super::*;
  use reqwest::{header::HeaderMap, StatusCode, Url};
  pub fn response(status: u16, content_type: &str, body: &str) -> FetchResponse {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, content_type.parse().unwrap());
    FetchResponse {
      status: StatusCode::from_u16(status).unwrap(),
      headers,
      body: body.as_bytes().to_vec(),
      final_url: Url::parse("https://example.com/feed").unwrap(),
    }
  }
  #[test]
  fn distinguishes_access_failures_and_bad_content_without_echoing_secrets() {
    for (status, body, expected) in [
      (401, "secret", "auth_required"),
      (403, "Access denied secret", "access_denied"),
      (429, "secret", "rate_limited"),
      (503, "unavailable secret", "http_error"),
      (200, "<html>Verify you are human secret</html>", "challenge_required"),
      (403, "<html><script>_waftokenid='secret'</script></html>", "challenge_required"),
      (200, "<html><form><input type='password'></form>secret</html>", "auth_required"),
      (200, "<html>ordinary website secret</html>", "unexpected_content"),
    ] {
      let error = parse_response(&response(status, "text/html", body)).unwrap_err();
      assert_eq!(error.result, expected);
      assert_eq!(error.http_status, Some(status as i64));
      assert!(!serde_json::to_string(&error).unwrap().contains("secret"));
    }
    assert_eq!(
      parse_response(&response(200, "application/xml", "<rss>broken"))
        .unwrap_err()
        .result,
      "parse_error"
    );
  }
  #[test]
  fn accepts_feeds_even_with_wrong_content_type_and_challenge_words_in_articles() {
    let xml = r#"<rss version="2.0"><channel><title>Test</title><link>https://example.com</link><description>Feed</description><item><guid>1</guid><title>CAPTCHA and 人机验证</title></item></channel></rss>"#;
    assert_eq!(parse_response(&response(200, "text/html", xml)).unwrap().entries.len(), 1);
    let atom = r#"<feed xmlns="http://www.w3.org/2005/Atom"><title>Test</title><id>urn:test</id><updated>2026-01-01T00:00:00Z</updated></feed>"#;
    assert!(parse_response(&response(200, "application/atom+xml", atom)).is_ok());
    let json = r#"{"version":"https://jsonfeed.org/version/1","title":"Test","items":[]}"#;
    assert!(parse_response(&response(200, "application/feed+json", json)).is_ok());
  }
}
