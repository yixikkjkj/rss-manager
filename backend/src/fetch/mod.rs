//! Feed ingestion and article extraction. All HTTP goes through FetchClient.
mod icons;

use chrono::{DateTime, Utc};
use reqwest::header::{HeaderMap, ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};
use rusqlite::OptionalExtension;

use crate::config::Config;
use crate::db::{with_conn, Db};
use crate::error::{ApiResult, AppError};
use crate::outbound::FetchClient;

#[derive(Debug, Clone, serde::Serialize)]
pub struct FetchOutcome {
  pub result: String,
  pub http_status: Option<i64>,
  pub new_articles: i64,
  pub error_message: Option<String>,
}

pub async fn fetch_single_feed(
  db: &Db,
  client: &FetchClient,
  feed_id: i64,
  config: &Config,
) -> ApiResult<FetchOutcome> {
  let (url, etag, modified) = with_conn(db, move |c| {
    c.query_row("SELECT xml_url,etag,last_modified FROM t_feed WHERE id=?1 AND deleted=0", [feed_id], |r| {
      Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?))
    })
    .optional()?
    .ok_or_else(AppError::not_found)
  })
  .await?;
  let started = std::time::Instant::now();
  let headers = conditional_headers(etag.as_deref(), modified.as_deref());
  let mut new_etag = etag;
  let mut new_modified = modified;
  let mut discovered_site = None;
  let outcome = match client.get(&url, headers).await {
    Err(e) => FetchOutcome {
      result: "network_error".into(),
      http_status: None,
      new_articles: 0,
      error_message: Some(e.msg.into()),
    },
    Ok(response) => {
      let status = response.status.as_u16() as i64;
      if status == 304 {
        new_etag = header_string(&response.headers, ETAG).or(new_etag);
        new_modified = header_string(&response.headers, LAST_MODIFIED).or(new_modified);
        FetchOutcome {
          result: "not_modified".into(),
          http_status: Some(status),
          new_articles: 0,
          error_message: None,
        }
      } else if !response.status.is_success() {
        FetchOutcome {
          result: "http_error".into(),
          http_status: Some(status),
          new_articles: 0,
          error_message: Some(format!("HTTP {status}")),
        }
      } else {
        new_etag = header_string(&response.headers, ETAG);
        new_modified = header_string(&response.headers, LAST_MODIFIED);
        let parsed = tokio::task::spawn_blocking(move || {
          feed_rs::parser::parse(response.body.as_slice())
            .map(|f| {
              let title = f.title.map(|t| t.content).unwrap_or_default();
              let entries = f.entries.iter().map(extract_entry).collect::<Vec<_>>();
              let site = f
                .links
                .iter()
                .filter(|link| link.rel.as_deref().is_none_or(|rel| rel == "alternate"))
                .filter(|link| {
                  link
                    .media_type
                    .as_deref()
                    .is_none_or(|kind| matches!(kind, "text/html" | "application/xhtml+xml"))
                })
                .find_map(|link| icons::resolve_url(&response.final_url, &link.href));
              (title, entries, site)
            })
            .map_err(|_| AppError::fetch_failed("Feed 格式无法解析"))
        })
        .await
        .map_err(|_| AppError::internal())?;
        match parsed {
          Ok((title, entries, site)) => {
            discovered_site = site;
            let inserted = upsert_articles(db, feed_id, title, entries).await?;
            FetchOutcome {
              result: "success".into(),
              http_status: Some(status),
              new_articles: inserted,
              error_message: None,
            }
          },
          Err(e) => FetchOutcome {
            result: "parse_error".into(),
            http_status: Some(status),
            new_articles: 0,
            error_message: Some(e.msg.into()),
          },
        }
      }
    },
  };
  record_outcome(
    db,
    feed_id,
    &outcome,
    new_etag,
    new_modified,
    config.fetch_interval,
    started.elapsed().as_millis() as i64,
  )
  .await?;
  if let Err(error) = icons::refresh(db, client, feed_id, url, discovered_site).await {
    tracing::warn!(feed_id, error = %error.msg, "无法更新订阅源图标");
  }
  Ok(outcome)
}

fn header_string(headers: &HeaderMap, name: reqwest::header::HeaderName) -> Option<String> {
  headers.get(name).and_then(|v| v.to_str().ok()).map(str::to_owned)
}
fn conditional_headers(etag: Option<&str>, modified: Option<&str>) -> HeaderMap {
  let mut headers = HeaderMap::new();
  if let Some(v) = etag.and_then(|s| s.parse().ok()) {
    headers.insert(IF_NONE_MATCH, v);
  }
  if let Some(v) = modified.and_then(|s| s.parse().ok()) {
    headers.insert(IF_MODIFIED_SINCE, v);
  }
  headers
}

struct EntryData {
  guid: String,
  title: String,
  author: Option<String>,
  link: Option<String>,
  published_at: Option<String>,
  content: Option<String>,
}
fn extract_entry(entry: &feed_rs::model::Entry) -> EntryData {
  let link = entry
    .links
    .iter()
    .find(|l| l.rel.as_deref().is_none_or(|r| r == "alternate"))
    .map(|l| l.href.clone())
    .filter(|s| reqwest::Url::parse(s).is_ok_and(|u| matches!(u.scheme(), "http" | "https")));
  let title = entry
    .title
    .as_ref()
    .map(|t| crate::content::plain_text(&t.content, 1000))
    .filter(|s| !s.is_empty())
    .unwrap_or_else(|| "(无标题)".into());
  let content = entry
    .content
    .as_ref()
    .and_then(|c| c.body.as_ref())
    .or_else(|| entry.summary.as_ref().map(|s| &s.content))
    .map(|s| crate::content::sanitize(s, link.as_deref()));
  let guid = if entry.id.trim().is_empty() {
    link.clone().unwrap_or_else(|| crate::auth::hash_token(&title))
  } else {
    entry.id.clone()
  };
  EntryData {
    guid,
    title,
    author: entry.authors.first().map(|a| a.name.clone()),
    link,
    published_at: entry.published.as_ref().or(entry.updated.as_ref()).map(dt_iso),
    content,
  }
}
fn dt_iso(dt: &DateTime<Utc>) -> String {
  dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

async fn upsert_articles(db: &Db, feed_id: i64, title: String, entries: Vec<EntryData>) -> ApiResult<i64> {
  with_conn(db, move |c| {
    let tx=c.transaction()?;
    // A deleted feed must not receive articles from a request already in flight.
    let active:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM t_feed WHERE id=?1 AND deleted=0)",[feed_id],|r|r.get(0))?;
    if !active {return Ok(0);}
    tx.execute("UPDATE t_feed SET feed_title=?1 WHERE id=?2",rusqlite::params![title,feed_id])?;
    let mut count=0;
    for e in entries {
      let now=crate::dto::now_iso();
      let inserted=tx.execute(
        "INSERT INTO t_article(feed_id,guid,title,author,link,published_at,content,created_time,updated_time,deleted)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?8,0) ON CONFLICT(feed_id,guid) DO NOTHING",
        rusqlite::params![feed_id,e.guid,e.title,e.author,e.link,e.published_at,e.content,now])?;
      count+=inserted as i64;
      if inserted==0 {
        tx.execute("UPDATE t_article SET title=?3,author=?4,link=?5,published_at=?6,content=?7,updated_time=?8
          WHERE feed_id=?1 AND guid=?2 AND (title IS NOT ?3 OR content IS NOT ?7 OR link IS NOT ?5 OR author IS NOT ?4 OR published_at IS NOT ?6)",
          rusqlite::params![feed_id,e.guid,e.title,e.author,e.link,e.published_at,e.content,now])?;
      }
    }
    tx.commit()?;
    Ok(count)
  }).await
}

#[allow(clippy::too_many_arguments)]
async fn record_outcome(
  db: &Db,
  id: i64,
  out: &FetchOutcome,
  etag: Option<String>,
  modified: Option<String>,
  default_interval: i64,
  duration: i64,
) -> ApiResult<()> {
  let out = out.clone();
  with_conn(db,move |c|{
    let tx=c.transaction()?;
    let (override_sec, failures):(Option<i64>,i64)=tx.query_row(
      "SELECT interval_override_sec,consecutive_failures FROM t_feed WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?)))?;
    let global:Option<i64>=tx.query_row("SELECT value FROM t_setting WHERE key='fetch_interval_sec'",[],|r|r.get(0)).optional()?;
    let interval=effective_interval(override_sec,global.unwrap_or(default_interval));
    let success=matches!(out.result.as_str(),"success"|"not_modified");
    let failures=if success {0} else {failures.saturating_add(1)};
    let delay=if success {interval} else {interval.saturating_mul(1 << (failures-1).clamp(0,10)).min(86400.max(interval))};
    let now=crate::dto::now_iso();
    let next=dt_iso(&(Utc::now()+chrono::Duration::seconds(delay)));
    let health=if success {"healthy"} else if failures>=3 {"dead"} else {"failing"};
    tx.execute("UPDATE t_feed SET health_status=?1,consecutive_failures=?2,last_fetch_at=?3,next_fetch_at=?4,updated_time=?3,
      etag=CASE WHEN ?5 THEN ?6 ELSE etag END,last_modified=CASE WHEN ?5 THEN ?7 ELSE last_modified END,
      last_success_at=CASE WHEN ?5 THEN ?3 ELSE last_success_at END WHERE id=?8 AND deleted=0",
      rusqlite::params![health,failures,now,next,success,etag,modified,id])?;
    tx.execute("INSERT INTO t_fetch_log(feed_id,fetch_at,http_status,result,error_message,new_article_count,duration_ms,created_time)
      VALUES(?1,?2,?3,?4,?5,?6,?7,?2)",rusqlite::params![id,now,out.http_status,out.result,out.error_message,out.new_articles,duration])?;
    tx.execute("DELETE FROM t_fetch_log WHERE fetch_at < ?1",[dt_iso(&(Utc::now()-chrono::Duration::days(30)))])?;
    tx.commit()?;
    Ok(())
  }).await
}
pub fn effective_interval(override_sec: Option<i64>, global: i64) -> i64 {
  override_sec.unwrap_or(global).clamp(1800, 604800)
}

pub async fn fetch_full_content(client: &FetchClient, url: &str) -> ApiResult<String> {
  let response = client.get(url, HeaderMap::new()).await?;
  if !response.status.is_success() {
    return Err(AppError::fetch_failed("原文页面返回错误"));
  }
  tokio::task::spawn_blocking(move || {
    let html = String::from_utf8_lossy(&response.body);
    let options = readabilityrs::ReadabilityOptions::builder()
      .max_elems_to_parse(30000)
      .build();
    let reader = readabilityrs::Readability::new(&html, Some(response.final_url.as_str()), Some(options))
      .map_err(|_| AppError::fetch_failed("正文解析失败"))?;
    let content = reader
      .parse()
      .and_then(|a| a.content)
      .filter(|s| !s.trim().is_empty())
      .ok_or_else(|| AppError::fetch_failed("未能提取正文，可查看原文"))?;
    Ok(crate::content::sanitize(&content, Some(response.final_url.as_str())))
  })
  .await
  .map_err(|_| AppError::internal())?
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn overrides_can_be_shorter_than_global() {
    assert_eq!(effective_interval(Some(1800), 3600), 1800);
    assert_eq!(effective_interval(None, 7200), 7200);
  }
  #[test]
  fn sends_request_validators() {
    let h = conditional_headers(Some("\"v1\""), Some("Wed, 21 Oct 2015 07:28:00 GMT"));
    assert!(h.contains_key(IF_NONE_MATCH));
    assert!(h.contains_key(IF_MODIFIED_SINCE));
    assert!(!h.contains_key(ETAG));
    assert!(!h.contains_key(LAST_MODIFIED));
  }
}
