use super::subscriptions::{get_feed, validate_interval, Items};
use crate::dto::respond;
use crate::{auth::AuthUser, db::with_conn, dto::TraceId, state::AppState};
use axum::extract::{Extension, Json, State};
use axum::response::Response;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct FetchNowRequest {
  pub feed_id: Option<i64>,
}
#[derive(Deserialize)]
pub struct ConfigRequest {
  pub fetch_interval_sec: i64,
}
#[derive(Serialize)]
pub struct ConfigVO {
  pub fetch_interval_sec: i64,
}
#[derive(Deserialize)]
pub struct LogRequest {
  pub feed_id: Option<i64>,
  pub page: Option<i64>,
}
#[derive(Serialize)]
pub struct LogVO {
  pub id: i64,
  pub feed_id: i64,
  pub feed_title: String,
  pub fetch_at: Option<String>,
  pub result: String,
  pub http_status: Option<i64>,
  pub error_message: Option<String>,
  pub new_article_count: i64,
  pub duration_ms: Option<i64>,
}
pub async fn fetch_now(
  State(s): State<AppState>,
  Extension(a): Extension<AuthUser>,
  Extension(t): Extension<TraceId>,
  Json(req): Json<FetchNowRequest>,
) -> Response {
  let r = async {
    if let Some(id) = req.feed_id {
      get_feed(&s.db, a.user_id, id).await?;
    }
    s.scheduler.trigger(req.feed_id).await
  }
  .await;
  respond(r, t)
}
pub async fn fetch_status(
  State(s): State<AppState>,
  Extension(_): Extension<AuthUser>,
  Extension(t): Extension<TraceId>,
) -> Response {
  respond(Ok(s.scheduler.status().await), t)
}
pub async fn fetch_config_get(
  State(s): State<AppState>,
  Extension(_): Extension<AuthUser>,
  Extension(t): Extension<TraceId>,
) -> Response {
  let default = s.config.fetch_interval;
  let r = with_conn(&s.db, move |c| {
    let value = c
      .query_row("SELECT value FROM t_setting WHERE key='fetch_interval_sec'", [], |r| r.get::<_, i64>(0))
      .optional()?;
    Ok(ConfigVO {
      fetch_interval_sec: value.unwrap_or(default),
    })
  })
  .await;
  respond(r, t)
}
pub async fn fetch_config_update(
  State(s): State<AppState>,
  Extension(_): Extension<AuthUser>,
  Extension(t): Extension<TraceId>,
  Json(req): Json<ConfigRequest>,
) -> Response {
  let r=async{
  validate_interval(Some(req.fetch_interval_sec))?;
  with_conn(&s.db,move|c|{
   let tx=c.transaction()?;
   tx.execute("INSERT INTO t_setting(key,value) VALUES('fetch_interval_sec',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[req.fetch_interval_sec])?;
   // Apply the new interval to inheriting feeds without repeatedly fetching now.
   let next=(chrono::Utc::now()+chrono::Duration::seconds(req.fetch_interval_sec)).format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
   tx.execute("UPDATE t_feed SET next_fetch_at=?1 WHERE deleted=0 AND interval_override_sec IS NULL AND health_status='healthy'",[next])?;
   tx.commit()?;Ok(ConfigVO{fetch_interval_sec:req.fetch_interval_sec})
  }).await
 }.await;
  respond(r, t)
}
pub async fn fetch_log_list(
  State(s): State<AppState>,
  Extension(a): Extension<AuthUser>,
  Extension(t): Extension<TraceId>,
  Json(req): Json<LogRequest>,
) -> Response {
  let r=with_conn(&s.db,move|c|{
  let offset=(req.page.unwrap_or(1).clamp(1,100000)-1)*50;
  let mut stmt=c.prepare("SELECT l.id,l.feed_id,f.title,l.fetch_at,l.result,l.http_status,l.error_message,l.new_article_count,l.duration_ms
   FROM t_fetch_log l JOIN t_feed f ON f.id=l.feed_id WHERE f.user_id=?1 AND (?2 IS NULL OR l.feed_id=?2) ORDER BY l.id DESC LIMIT 50 OFFSET ?3")?;
  let items=stmt.query_map(params![a.user_id,req.feed_id,offset],|r|Ok(LogVO{id:r.get(0)?,feed_id:r.get(1)?,feed_title:r.get(2)?,fetch_at:r.get(3)?,result:r.get(4)?,http_status:r.get(5)?,error_message:r.get(6)?,new_article_count:r.get(7)?,duration_ms:r.get(8)?}))?.collect::<Result<Vec<_>,_>>()?;
  Ok(Items{items})
 }).await;
  respond(r, t)
}
