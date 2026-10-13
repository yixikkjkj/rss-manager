//! Content is untrusted, including HTML already stored by older releases.
use reqwest::Url;

pub fn sanitize(html: &str, base: Option<&str>) -> String {
  let mut builder = ammonia::Builder::default();
  builder.url_schemes(["http", "https"].into_iter().collect());
  if let Some(base) = base.and_then(|b| Url::parse(b).ok()) {
    builder.url_relative(ammonia::UrlRelative::RewriteWithBase(base));
  } else {
    builder.url_relative(ammonia::UrlRelative::Deny);
  }
  builder.clean(html).to_string()
}

pub fn plain_text(html: &str, max_chars: usize) -> String {
  let doc = scraper::Html::parse_fragment(html);
  doc
    .root_element()
    .text()
    .collect::<Vec<_>>()
    .join(" ")
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ")
    .chars()
    .take(max_chars)
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn strips_active_content_and_resolves_relative_images() {
    let clean = sanitize(
      r#"<p>Hello</p><script>alert(1)</script><img src="/image.png" onerror="alert(1)"><a href="javascript:alert(1)">x</a><iframe src="/"></iframe>"#,
      Some("https://example.com/story"),
    );
    assert!(!clean.contains("onerror"));
    assert!(!clean.contains("<script"));
    assert!(!clean.contains("javascript:"));
    assert!(!clean.contains("<iframe"));
    assert!(clean.contains("https://example.com/image.png"));
    assert!(clean.contains("<p>Hello</p>"));
  }
}
