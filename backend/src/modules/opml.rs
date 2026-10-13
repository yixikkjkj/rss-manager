use rusqlite::OptionalExtension;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use axum::extract::{Extension, Json, State};
use axum::response::{IntoResponse, Response};
use quick_xml::events::Event;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};

use crate::auth::AuthUser;
use crate::db::{with_conn, Db};
use crate::dto::{Resp, TraceId};
use crate::error::{ApiResult, AppError};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct OpmlImportRequest {
  pub content: String,
}
#[derive(Debug, Deserialize)]
pub struct OpmlImportReportRequest {
  pub import_id: i64,
}

#[derive(Debug, Serialize)]
pub struct OpmlImportResultVO {
  pub import_id: i64,
  pub parsed: i32,
  pub failed: i32,
  pub first_fetch_ok: i32,
}
#[derive(Debug, Serialize, Clone)]
pub struct NormalizeItem {
  pub feed_title: String,
  pub action: String,
  pub before: String,
  pub after: String,
}
#[derive(Debug, Serialize, Clone)]
pub struct ImportFailure {
  pub feed_title: String,
  pub reason: String,
}
#[derive(Debug, Serialize)]
pub struct OpmlImportReportVO {
  pub import_id: i64,
  pub parsed: i32,
  pub failed: i32,
  pub first_fetch_ok: i32,
  pub total_first_fetch: i32,
  pub completed: i32,
  pub normalized: Vec<NormalizeItem>,
  pub failures: Vec<ImportFailure>,
}
#[derive(Debug, Serialize)]
pub struct OpmlExportVO {
  pub content: String,
}

// ============ 内存态导入报告（无独立表，对齐 DDL） ============

struct ImportReport {
  parsed: i32,
  failed: i32,
  first_fetch_ok: i32,
  total_first_fetch: i32,
  completed: i32,
  normalized: Vec<NormalizeItem>,
  failures: Vec<ImportFailure>,
}

fn report_store() -> &'static Mutex<HashMap<i64, ImportReport>> {
  static STORE: OnceLock<Mutex<HashMap<i64, ImportReport>>> = OnceLock::new();
  STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

// ============ OPML 解析 ============

struct ParsedFeed {
  text: String,
  xml_url: String,
  html_url: Option<String>,
  group: Vec<String>,
}

fn xml_escape(s: &str) -> String {
  s.replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
    .replace('"', "&quot;")
    .replace('\'', "&apos;")
}

fn unescape(s: &str) -> String {
  quick_xml::escape::unescape(s)
    .map(|v| v.into_owned())
    .unwrap_or_else(|_| s.to_owned())
}

fn get_attr(e: &quick_xml::events::BytesStart, name: &[u8]) -> Option<String> {
  e.attributes()
    .flatten()
    .find(|a| a.key.as_ref() == name)
    .map(|a| unescape(&String::from_utf8_lossy(&a.value)))
}

struct ParsedOpml {
  feeds: Vec<ParsedFeed>,
  folders: Vec<Vec<String>>,
}

fn parse_opml(content: &str) -> Result<ParsedOpml, ()> {
  let mut reader = Reader::from_str(content);
  reader.config_mut().trim_text(true);

  let mut feeds = Vec::new();
  let mut folders = Vec::new();
  let mut group_stack: Vec<String> = Vec::new();
  let mut group_flags: Vec<bool> = Vec::new();
  let mut saw_opml = false;
  let mut depth = 0usize;

  loop {
    match reader.read_event() {
      Ok(Event::Start(e)) => {
        let name = e.name().as_ref().to_vec();
        if depth == 0 && (saw_opml || name != b"opml") {
          return Err(());
        }
        depth += 1;
        if name == b"opml" {
          saw_opml = true;
        } else if name == b"outline" {
          let xml_url = get_attr(&e, b"xmlUrl");
          let text = get_attr(&e, b"text")
            .or_else(|| get_attr(&e, b"title"))
            .unwrap_or_default();
          let html_url = get_attr(&e, b"htmlUrl");
          if let Some(xu) = xml_url {
            if !xu.is_empty() {
              feeds.push(ParsedFeed {
                text,
                xml_url: xu,
                html_url,
                group: group_stack.clone(),
              });
            }
            group_flags.push(false);
          } else {
            group_stack.push(if text.trim().is_empty() { "未命名文件夹".into() } else { text });
            folders.push(group_stack.clone());
            group_flags.push(true);
          }
        }
      },
      Ok(Event::Empty(e)) => {
        let name = e.name().as_ref().to_vec();
        if name == b"outline" {
          let xml_url = get_attr(&e, b"xmlUrl");
          let text = get_attr(&e, b"text")
            .or_else(|| get_attr(&e, b"title"))
            .unwrap_or_default();
          let html_url = get_attr(&e, b"htmlUrl");
          if let Some(xu) = xml_url {
            if !xu.is_empty() {
              feeds.push(ParsedFeed {
                text,
                xml_url: xu,
                html_url,
                group: group_stack.clone(),
              });
            }
          } else {
            let mut path = group_stack.clone();
            path.push(if text.trim().is_empty() { "未命名文件夹".into() } else { text });
            folders.push(path);
          }
        }
      },
      Ok(Event::End(e)) => {
        depth = depth.checked_sub(1).ok_or(())?;
        let name = e.name().as_ref().to_vec();
        if name == b"outline" && group_flags.pop().unwrap_or(false) {
          group_stack.pop();
        }
      },
      Ok(Event::Eof) => break,
      Err(_) => return Err(()),
      _ => {},
    }
  }

  if !saw_opml || (feeds.is_empty() && folders.is_empty()) || depth != 0 {
    return Err(());
  }
  Ok(ParsedOpml { feeds, folders })
}

// ============ 规范化 ============

fn normalize_url(raw: &str) -> Option<String> {
  crate::outbound::normalize_url(raw).ok()
}

fn host_of(url: &str) -> Option<String> {
  let rest = url.split_once("://")?.1;
  let host = rest.split('/').next()?.split(':').next()?;
  Some(host.to_string())
}

/// http→https 升级尝试（失败保留 http）
async fn try_upgrade_https(client: &crate::outbound::FetchClient, url: &str) -> String {
  if !url.starts_with("http://") {
    return url.to_string();
  }
  let https = url.replacen("http://", "https://", 1);
  let probe =
    tokio::time::timeout(std::time::Duration::from_secs(3), client.get(&https, reqwest::header::HeaderMap::new()))
      .await;
  match probe {
    Ok(Ok(resp)) if resp.status.is_success() => https,
    _ => url.to_string(),
  }
}

// ============ 导入 ============

pub async fn opml_import(
  State(state): State<AppState>,
  Extension(auth): Extension<AuthUser>,
  Extension(tid): Extension<TraceId>,
  Json(req): Json<OpmlImportRequest>,
) -> Response {
  let parsed_opml = match parse_opml(&req.content) {
    Ok(f) => f,
    Err(_) => return AppError::opml_parse_failed().with_trace(&tid.0).into_response(),
  };

  // Validate and create complete paths, including empty folders, before importing feeds.
  for path in &parsed_opml.folders {
    if path.len() > 64
      || path
        .iter()
        .any(|name| super::subscriptions::validate_name(name).is_err())
    {
      return AppError::invalid("OPML 文件夹名称无效或超过 64 层")
        .with_trace(&tid.0)
        .into_response();
    }
  }
  for path in &parsed_opml.folders {
    if let Err(error) = ensure_path(&state.db, auth.user_id, path.clone()).await {
      return error.with_trace(&tid.0).into_response();
    }
  }
  let feeds = parsed_opml.feeds;

  static NEXT_ID: std::sync::OnceLock<std::sync::atomic::AtomicI64> = std::sync::OnceLock::new();
  let import_id = NEXT_ID
    .get_or_init(|| std::sync::atomic::AtomicI64::new(chrono::Utc::now().timestamp_millis()))
    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
  let mut normalized: Vec<NormalizeItem> = Vec::new();
  let mut failures: Vec<ImportFailure> = Vec::new();
  let mut inserted_ids: Vec<i64> = Vec::new();
  let mut parsed = 0i32;
  let mut failed = 0i32;

  for pf in &feeds {
    match import_one(&state.db, &state, auth.user_id, pf, &mut normalized).await {
      Ok(Some(feed_id)) => {
        parsed += 1;
        inserted_ids.push(feed_id);
      },
      Ok(None) => {
        // 已存在（xml_url 去重），仍算解析成功
        parsed += 1;
      },
      Err(e) => {
        failed += 1;
        failures.push(ImportFailure {
          feed_title: pf.text.clone(),
          reason: e.msg.to_string(),
        });
      },
    }
  }

  let total_first_fetch = inserted_ids.len() as i32;
  {
    let mut store = report_store().lock().unwrap();
    if store.len() >= 100 {
      if let Some(oldest) = store.keys().min().copied() {
        store.remove(&oldest);
      }
    }
    store.insert(
      import_id,
      ImportReport {
        parsed,
        failed,
        first_fetch_ok: 0,
        total_first_fetch,
        completed: 0,
        normalized: normalized.clone(),
        failures: failures.clone(),
      },
    );
  }

  // 后台首抓（异步），成功数回填报告
  if !inserted_ids.is_empty() {
    let bg_state = state.clone();
    tokio::spawn(async move {
      let mut tasks = tokio::task::JoinSet::new();
      for id in inserted_ids {
        let scheduler = bg_state.scheduler.clone();
        tasks.spawn(async move { scheduler.fetch_wait(id).await });
      }
      while let Some(result) = tasks.join_next().await {
        let ok = matches!(result, Ok(Ok(ref out)) if matches!(out.result.as_str(), "success" | "not_modified"));
        if let Ok(mut store) = report_store().lock() {
          if let Some(r) = store.get_mut(&import_id) {
            r.completed += 1;
            if ok {
              r.first_fetch_ok += 1;
            }
          }
        }
      }
    });
  }

  let vo = OpmlImportResultVO {
    import_id,
    parsed,
    failed,
    first_fetch_ok: 0,
  };
  Resp::ok(vo, tid.0).into_response()
}

/// 导入单个源：规范化 → https 升级 → 建分组 → 建源（xml_url 去重）
async fn import_one(
  db: &Db,
  state: &AppState,
  user_id: i64,
  pf: &ParsedFeed,
  normalized: &mut Vec<NormalizeItem>,
) -> ApiResult<Option<i64>> {
  let title = if pf.text.trim().is_empty() {
    pf.xml_url.clone()
  } else {
    pf.text.trim().to_string()
  };

  // 1. URL 规范化与公网地址校验；保留路径和查询参数内有意义的斜杠。
  let raw_url = normalize_url(&pf.xml_url).ok_or(AppError::opml_parse_failed())?;
  if raw_url != pf.xml_url {
    normalized.push(NormalizeItem {
      feed_title: title.clone(),
      action: "URL 规范化".into(),
      before: pf.xml_url.clone(),
      after: raw_url.clone(),
    });
  }

  // 2. http→https 升级
  let client = state.scheduler.client();
  let upgraded = try_upgrade_https(&client, &raw_url).await;
  if upgraded != raw_url {
    normalized.push(NormalizeItem {
      feed_title: title.clone(),
      action: "http->https".into(),
      before: raw_url.clone(),
      after: upgraded.clone(),
    });
  }
  let final_url = upgraded;

  // 3. 域名不一致提示
  if let (Some(html), Some(xhost)) = (&pf.html_url, host_of(&final_url)) {
    if let Some(hhost) = host_of(html) {
      if hhost != xhost {
        normalized.push(NormalizeItem {
          feed_title: title.clone(),
          action: "域名不一致提示".into(),
          before: xhost,
          after: hhost,
        });
      }
    }
  }

  // 4. Preserve the full folder path.
  let folder_id = ensure_path(db, user_id, pf.group.clone()).await?;

  // 5. 建源（去重）
  let html_url = pf.html_url.clone();
  with_conn(db, move |c| {
    let exists: Option<i64> = c
      .query_row(
        "SELECT id FROM t_feed WHERE user_id = ?1 AND xml_url = ?2 AND deleted = 0",
        rusqlite::params![user_id, final_url],
        |r| r.get(0),
      )
      .optional()?;
    if exists.is_some() {
      return Ok(None);
    }
    c.execute(
      "UPDATE t_feed SET deleted=0,folder_id=?1,title=?2,health_status='healthy',consecutive_failures=0,next_fetch_at=?3 WHERE user_id=?4 AND xml_url=?5 AND deleted=1",
      rusqlite::params![folder_id,title,crate::dto::now_iso(),user_id,final_url],
    )?;
    if let Ok(id) = c.query_row("SELECT id FROM t_feed WHERE user_id=?1 AND xml_url=?2 AND deleted=0",rusqlite::params![user_id,final_url],|r|r.get::<_,i64>(0)) { return Ok(Some(id)); }
    c.execute(
      "INSERT INTO t_feed (user_id, folder_id, title, feed_title, xml_url, html_url, fetch_interval_sec, \
             use_readability, health_status, consecutive_failures, next_fetch_at, created_time, updated_time, deleted) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 3600, 0, 'healthy', 0, ?7, ?7, ?7, 0)",
      rusqlite::params![
        user_id,
        folder_id,
        title,
        title,
        final_url,
        html_url,
        crate::dto::now_iso()
      ],
    )?;
    Ok(Some(c.last_insert_rowid()))
  })
  .await
}

async fn ensure_path(db: &Db, user: i64, path: Vec<String>) -> ApiResult<Option<i64>> {
  with_conn(db, move |c| {
    let tx = c.transaction()?;
    let mut parent = None;
    for name in path {
      parent = Some(super::folders::ensure_folder(&tx, user, parent, &name)?);
    }
    tx.commit()?;
    Ok(parent)
  })
  .await
}

pub async fn opml_import_report(
  State(_state): State<AppState>,
  Extension(_auth): Extension<AuthUser>,
  Extension(tid): Extension<TraceId>,
  Json(req): Json<OpmlImportReportRequest>,
) -> Response {
  let store = report_store().lock().unwrap();
  match store.get(&req.import_id) {
    Some(r) => {
      let vo = OpmlImportReportVO {
        import_id: req.import_id,
        parsed: r.parsed,
        failed: r.failed,
        first_fetch_ok: r.first_fetch_ok,
        total_first_fetch: r.total_first_fetch,
        completed: r.completed,
        normalized: r.normalized.clone(),
        failures: r.failures.clone(),
      };
      Resp::ok(vo, tid.0).into_response()
    },
    None => AppError::not_found().with_trace(&tid.0).into_response(),
  }
}

type ExportFolder = (i64, Option<i64>, String);
type ExportFeed = (Option<i64>, String, String, Option<String>);
fn export_children(parent: Option<i64>, folders: &[ExportFolder], feeds: &[ExportFeed], body: &mut String) {
  for (id, _, name) in folders.iter().filter(|(_, p, _)| *p == parent) {
    body.push_str(&format!("<outline text=\"{}\">\n", xml_escape(name)));
    export_children(Some(*id), folders, feeds, body);
    body.push_str("</outline>\n");
  }
  for (_, title, xml, html) in feeds.iter().filter(|(p, _, _, _)| *p == parent) {
    let html_attr = html
      .as_ref()
      .map(|h| format!(" htmlUrl=\"{}\"", xml_escape(h)))
      .unwrap_or_default();
    body.push_str(&format!(
      "<outline text=\"{}\" title=\"{}\" type=\"rss\" xmlUrl=\"{}\"{}/>\n",
      xml_escape(title),
      xml_escape(title),
      xml_escape(xml),
      html_attr
    ));
  }
}
pub async fn opml_export(
  State(state): State<AppState>,
  Extension(auth): Extension<AuthUser>,
  Extension(tid): Extension<TraceId>,
) -> Response {
  let result=with_conn(&state.db,move|c| {
    let folders={
      let mut stmt=c.prepare("SELECT id,parent_id,name FROM t_folder WHERE user_id=?1 AND deleted=0 ORDER BY sort_order,id")?;
      let rows = stmt.query_map([auth.user_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?.collect::<Result<Vec<ExportFolder>,_>>()?;
      rows
    };
    let feeds={
      let mut stmt=c.prepare("SELECT folder_id,title,xml_url,html_url FROM t_feed WHERE user_id=?1 AND deleted=0 ORDER BY id")?;
      let rows = stmt.query_map([auth.user_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?.collect::<Result<Vec<ExportFeed>,_>>()?;
      rows
    };
    let mut body=String::new();
    export_children(None,&folders,&feeds,&mut body);
    Ok(OpmlExportVO{content:format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<opml version=\"1.0\"><head><title>rss-manager subscriptions</title></head><body>{body}</body></opml>")})
  }).await;
  crate::dto::respond(result, tid)
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn parses_nested_groups_and_decodes_xml_once() {
    let feeds=parse_opml(r#"<opml><body><outline text="Parent"><outline text="Child"><outline text="A &amp;amp; B" xmlUrl="https://example.com/feed?a=1&amp;b=2"/></outline></outline></body></opml>"#).unwrap();
    assert_eq!(feeds.feeds.len(), 1);
    assert_eq!(feeds.feeds[0].group, vec!["Parent", "Child"]);
    assert_eq!(feeds.feeds[0].text, "A &amp; B");
    assert!(feeds.feeds[0].xml_url.ends_with("a=1&b=2"));
  }
  #[test]
  fn rejects_truncated_documents() {
    assert!(parse_opml(r#"<opml><body><outline xmlUrl="https://example.com/feed"/>"#).is_err());
  }
}
