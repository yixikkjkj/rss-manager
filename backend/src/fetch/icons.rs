//! Website icon discovery shares the feed client's URL, redirect and size limits.
use std::time::Duration;

use reqwest::{header::HeaderMap, Url};
use rusqlite::{params, OptionalExtension};
use scraper::{Html, Selector};

use crate::db::{with_conn, Db};
use crate::error::ApiResult;
use crate::outbound::{validate_url, FetchClient};

pub(super) fn resolve_url(base: &Url, href: &str) -> Option<String> {
  let href = href.trim();
  if href.is_empty() {
    return None;
  }
  let url = base.join(href).ok()?;
  validate_url(url.as_str()).ok().map(|url| url.to_string())
}

fn discover_icon(html: &str, page_url: &Url) -> Option<String> {
  let doc = Html::parse_document(html);
  let base_selector = Selector::parse("head base[href]").unwrap();
  let base = doc
    .select(&base_selector)
    .next()
    .and_then(|element| resolve_url(page_url, element.value().attr("href")?))
    .and_then(|url| Url::parse(&url).ok())
    .unwrap_or_else(|| page_url.clone());
  let links = Selector::parse("head link[rel][href]").unwrap();
  doc
    .select(&links)
    .filter_map(|element| {
      let rel = element.value().attr("rel")?;
      let priority = if rel
        .split_ascii_whitespace()
        .any(|token| token.eq_ignore_ascii_case("icon"))
      {
        0
      } else if rel.split_ascii_whitespace().any(|token| {
        token.eq_ignore_ascii_case("apple-touch-icon") || token.eq_ignore_ascii_case("apple-touch-icon-precomposed")
      }) {
        1
      } else {
        return None;
      };
      Some((priority, resolve_url(&base, element.value().attr("href")?)?))
    })
    .min_by_key(|(priority, _)| *priority)
    .map(|(_, url)| url)
}

pub(super) async fn refresh(
  db: &Db,
  client: &FetchClient,
  feed_id: i64,
  xml_url: String,
  discovered_site: Option<String>,
) -> ApiResult<()> {
  let expected_xml = xml_url.clone();
  let snapshot = with_conn(db, move |c| {
    Ok(
      c.query_row(
        "SELECT html_url,icon_checked_at FROM t_feed WHERE id=?1 AND xml_url=?2 AND deleted=0",
        params![feed_id, expected_xml],
        |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?)),
      )
      .optional()?,
    )
  })
  .await?;
  let Some((old_site, checked_at)) = snapshot else {
    return Ok(());
  };
  let site = old_site.clone().or(discovered_site);
  let recent = checked_at
    .as_deref()
    .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
    .is_some_and(|checked| checked > chrono::Utc::now() - chrono::Duration::days(1));
  if recent && site == old_site {
    return Ok(());
  }
  let target = site.clone().or_else(|| {
    let url = validate_url(&xml_url).ok()?;
    resolve_url(&url, "/")
  });
  let mut icon = None;
  let mut fetched = false;
  if let Some(target) = target {
    // Icon failures must neither fail RSS ingestion nor hold up a fetch for 30 seconds.
    if let Ok(Ok(response)) = tokio::time::timeout(Duration::from_secs(5), client.get(&target, HeaderMap::new())).await
    {
      if response.status.is_success() {
        icon = tokio::task::spawn_blocking(move || {
          discover_icon(&String::from_utf8_lossy(&response.body), &response.final_url)
        })
        .await
        .ok()
        .flatten();
        fetched = true;
      }
    }
  }
  with_conn(db, move |c| {
    // Do not overwrite a website edit or a deleted subscription while HTTP was in flight.
    c.execute(
      "UPDATE t_feed SET html_url=?1,icon_url=CASE WHEN ?2 THEN ?3 ELSE icon_url END,icon_checked_at=?4
       WHERE id=?5 AND xml_url=?6 AND html_url IS ?7 AND deleted=0",
      params![site, fetched, icon, crate::dto::now_iso(), feed_id, xml_url, old_site],
    )?;
    Ok(())
  })
  .await
}

#[cfg(test)]
mod tests {
  use super::*;

  fn discover(html: &str) -> Option<String> {
    discover_icon(html, &Url::parse("https://example.com/blog/index.html").unwrap())
  }

  #[test]
  fn resolves_relative_icons_and_prefers_standard_icon_over_touch_icon() {
    assert_eq!(
      discover(
        r#"<head>
      <link rel="apple-touch-icon" href="/touch.png">
      <link rel="SHORTCUT ICON" href="icons/site.svg?v=2&amp;theme=light">
    </head>"#
      )
      .as_deref(),
      Some("https://example.com/blog/icons/site.svg?v=2&theme=light")
    );
  }

  #[test]
  fn honors_base_and_protocol_relative_cdn_urls() {
    assert_eq!(
      discover(r#"<head><base href="/assets/"><link rel="icon" href="site.png"></head>"#).as_deref(),
      Some("https://example.com/assets/site.png")
    );
    assert_eq!(
      discover(r#"<head><link rel="icon" href="//cdn.example.com/logo.svg"></head>"#).as_deref(),
      Some("https://cdn.example.com/logo.svg")
    );
  }

  #[test]
  fn ignores_unsafe_empty_and_non_icon_links_and_uses_touch_fallback() {
    assert_eq!(
      discover(
        r#"<head>
      <link rel="stylesheet" href="/style.css">
      <link rel="icon" href="javascript:alert(1)">
      <link rel="icon" href="http://127.0.0.1/private">
      <link rel="icon" href="data:image/png;base64,AAAA">
      <link rel="icon" href=" ">
      <link rel="apple-touch-icon-precomposed" href="/touch.png">
    </head>"#
      )
      .as_deref(),
      Some("https://example.com/touch.png")
    );
    assert_eq!(discover("<head></head><body><link rel='icon' href='/body.png'></body>"), None);
    assert_eq!(discover("<head><link rel='mask-icon' href='/mask.svg'></head>"), None);
  }

  #[tokio::test]
  async fn caches_failed_discovery_without_losing_an_icon_or_changing_feed_health() {
    let db = crate::db::open(":memory:").await.unwrap();
    with_conn(&db, |c| {
      c.execute("INSERT INTO t_user(id,username,password_hash) VALUES(1,'icons','unused')", [])?;
      // A legacy private website is rejected before any network access.
      c.execute(
        "INSERT INTO t_feed(id,user_id,title,xml_url,html_url,icon_url)
        VALUES(1,1,'Icons','https://example.com/feed','http://127.0.0.1/','https://example.com/old.png')",
        [],
      )?;
      Ok(())
    })
    .await
    .unwrap();
    let client = FetchClient::new();
    refresh(&db, &client, 1, "https://example.com/feed".into(), None)
      .await
      .unwrap();
    let checked: String = with_conn(&db, |c| {
      let (icon, health, checked): (String, String, String) =
        c.query_row("SELECT icon_url,health_status,icon_checked_at FROM t_feed WHERE id=1", [], |r| {
          Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?;
      assert_eq!(icon, "https://example.com/old.png");
      assert_eq!(health, "healthy");
      Ok(checked)
    })
    .await
    .unwrap();
    refresh(&db, &client, 1, "https://example.com/feed".into(), None)
      .await
      .unwrap();
    with_conn(&db, move |c| {
      assert_eq!(
        c.query_row("SELECT icon_checked_at FROM t_feed WHERE id=1", [], |r| r.get::<_, String>(0))?,
        checked
      );
      Ok(())
    })
    .await
    .unwrap();
  }
}
