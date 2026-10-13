//! Subscription and group RPCs. Updates are validated before a transaction writes.
use axum::extract::{Extension, Json, State};
use axum::response::Response;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::auth::AuthUser;
use crate::db::{with_conn, Db};
use crate::dto::{respond, TraceId};
use crate::error::{ApiResult, AppError};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct IdRequest {
  pub id: i64,
}
#[derive(Deserialize)]
pub struct FeedListRequest {
  pub folder_id: Option<i64>,
}
#[derive(Deserialize)]
pub struct FeedCreateRequest {
  pub xml_url: String,
  pub folder_id: Option<i64>,
  pub title: Option<String>,
  pub html_url: Option<String>,
  pub fetch_interval_sec: Option<i64>,
  pub use_readability: Option<bool>,
}
#[derive(Deserialize)]
pub struct FeedUpdateRequest {
  pub id: i64,
  pub xml_url: String,
  pub folder_id: Option<i64>,
  pub title: String,
  pub html_url: Option<String>,
  pub fetch_interval_sec: Option<i64>,
  pub use_readability: bool,
}
#[derive(Serialize)]
pub struct FeedVO {
  pub id: i64,
  pub folder_id: Option<i64>,
  pub title: String,
  pub feed_title: String,
  pub xml_url: String,
  pub html_url: Option<String>,
  pub health_status: String,
  pub consecutive_failures: i32,
  pub last_fetch_at: Option<String>,
  pub use_readability: bool,
  pub unread_count: i64,
  pub fetch_interval_sec: Option<i64>,
}
#[derive(Serialize)]
pub struct Items<T> {
  pub items: Vec<T>,
}

pub fn validate_interval(value: Option<i64>) -> ApiResult<Option<i64>> {
  if value.is_some_and(|v| !(1800..=604800).contains(&v)) {
    Err(AppError::invalid("抓取间隔须为 30 分钟至 7 天"))
  } else {
    Ok(value)
  }
}
pub fn validate_name(name: &str) -> ApiResult<String> {
  let name = name.trim();
  if name.is_empty() || name.chars().count() > 200 {
    return Err(AppError::invalid("名称须为 1–200 个字符"));
  }
  Ok(name.into())
}
fn public_site(value: Option<String>) -> ApiResult<Option<String>> {
  value
    .filter(|s| !s.trim().is_empty())
    .map(|s| crate::outbound::normalize_url(&s))
    .transpose()
}

const FEED_COLS:&str="f.id,f.folder_id,f.title,COALESCE(f.feed_title,''),f.xml_url,f.html_url,f.health_status,
 f.consecutive_failures,f.last_fetch_at,f.use_readability,
 (SELECT count(*) FROM t_article x WHERE x.feed_id=f.id AND x.deleted=0
 AND NOT EXISTS(SELECT 1 FROM t_article_read r WHERE r.user_id=f.user_id AND r.article_id=x.id)),f.interval_override_sec";
fn map_feed(r: &rusqlite::Row) -> rusqlite::Result<FeedVO> {
  Ok(FeedVO {
    id: r.get(0)?,
    folder_id: r.get(1)?,
    title: r.get(2)?,
    feed_title: r.get(3)?,
    xml_url: r.get(4)?,
    html_url: r.get(5)?,
    health_status: r.get(6)?,
    consecutive_failures: r.get(7)?,
    last_fetch_at: r.get(8)?,
    use_readability: r.get(9)?,
    unread_count: r.get(10)?,
    fetch_interval_sec: r.get(11)?,
  })
}
pub async fn get_feed(db: &Db, user: i64, id: i64) -> ApiResult<FeedVO> {
  with_conn(db, move |c| {
    c.query_row(
      &format!("SELECT {FEED_COLS} FROM t_feed f WHERE f.id=?1 AND f.user_id=?2 AND f.deleted=0"),
      params![id, user],
      map_feed,
    )
    .optional()?
    .ok_or_else(AppError::not_found)
  })
  .await
}
pub async fn feed_list(
  State(s): State<AppState>,
  Extension(a): Extension<AuthUser>,
  Extension(t): Extension<TraceId>,
  Json(req): Json<FeedListRequest>,
) -> Response {
  let r=with_conn(&s.db,move|c|{
    let mut stmt=c.prepare(&format!("SELECT {FEED_COLS} FROM t_feed f WHERE f.user_id=?1 AND f.deleted=0 AND (?2 IS NULL OR f.folder_id=?2) ORDER BY f.id"))?;
    let items=stmt.query_map(params![a.user_id,req.folder_id],map_feed)?.collect::<Result<Vec<_>,_>>()?;
    Ok(Items{items})
  }).await;
  respond(r, t)
}
pub async fn feed_get(
  State(s): State<AppState>,
  Extension(a): Extension<AuthUser>,
  Extension(t): Extension<TraceId>,
  Json(req): Json<IdRequest>,
) -> Response {
  respond(get_feed(&s.db, a.user_id, req.id).await, t)
}
pub async fn feed_create(
  State(s): State<AppState>,
  Extension(a): Extension<AuthUser>,
  Extension(t): Extension<TraceId>,
  Json(req): Json<FeedCreateRequest>,
) -> Response {
  let r=async{
    let xml=crate::outbound::normalize_url(&req.xml_url)?;
    let title=validate_name(req.title.as_deref().filter(|x|!x.trim().is_empty()).unwrap_or(&xml))?;
    let html=public_site(req.html_url)?;
    let interval=validate_interval(req.fetch_interval_sec)?;
    let id=with_conn(&s.db,move|c|{
      super::folders::check_folder(c,a.user_id,req.folder_id)?;
      let now=crate::dto::now_iso();
      c.query_row("INSERT INTO t_feed(user_id,folder_id,title,xml_url,html_url,interval_override_sec,use_readability,next_fetch_at,created_time,updated_time)
        VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?8,?8)
        ON CONFLICT(user_id,xml_url) DO UPDATE SET deleted=0,folder_id=excluded.folder_id,title=excluded.title,
        html_url=excluded.html_url,interval_override_sec=excluded.interval_override_sec,use_readability=excluded.use_readability,
        health_status='healthy',consecutive_failures=0,next_fetch_at=excluded.next_fetch_at,updated_time=excluded.updated_time,
        etag=NULL,last_modified=NULL WHERE t_feed.deleted=1 RETURNING id",
        params![a.user_id,req.folder_id,title,xml,html,interval,req.use_readability.unwrap_or(false),now],|r|r.get::<_,i64>(0))
        .optional()?.ok_or_else(AppError::feed_exists)
    }).await?;
    s.scheduler.trigger(Some(id)).await?;
    get_feed(&s.db,a.user_id,id).await
  }.await;
  respond(r, t)
}
pub async fn feed_update(
  State(s): State<AppState>,
  Extension(a): Extension<AuthUser>,
  Extension(t): Extension<TraceId>,
  Json(req): Json<FeedUpdateRequest>,
) -> Response {
  let r=async{
    let xml=crate::outbound::normalize_url(&req.xml_url)?;
    let title=validate_name(&req.title)?;
    let html=public_site(req.html_url)?;
    let interval=validate_interval(req.fetch_interval_sec)?;
    with_conn(&s.db,move|c|{
      let tx=c.transaction()?;
      super::folders::check_folder(&tx,a.user_id,req.folder_id)?;
      let old:String=tx.query_row("SELECT xml_url FROM t_feed WHERE id=?1 AND user_id=?2 AND deleted=0",
        params![req.id,a.user_id],|r|r.get(0)).optional()?.ok_or_else(AppError::not_found)?;
      if old!=xml {
        let conflict:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM t_feed WHERE user_id=?1 AND xml_url=?2 AND id!=?3)",
          params![a.user_id,xml,req.id],|r|r.get(0))?;
        if conflict{return Err(AppError::feed_exists());}
      }
      tx.execute("UPDATE t_feed SET title=?1,folder_id=?2,html_url=?3,interval_override_sec=?4,use_readability=?5,
        xml_url=?6,updated_time=?7,next_fetch_at=?7,
        etag=CASE WHEN xml_url!=?6 THEN NULL ELSE etag END,last_modified=CASE WHEN xml_url!=?6 THEN NULL ELSE last_modified END,
        health_status=CASE WHEN xml_url!=?6 THEN 'healthy' ELSE health_status END,
        consecutive_failures=CASE WHEN xml_url!=?6 THEN 0 ELSE consecutive_failures END
        WHERE id=?8 AND user_id=?9 AND deleted=0",
        params![title,req.folder_id,html,interval,req.use_readability,xml,crate::dto::now_iso(),req.id,a.user_id])?;
      tx.commit()?;Ok(())
    }).await?;
    get_feed(&s.db,a.user_id,req.id).await
  }.await;
  respond(r, t)
}
pub async fn feed_delete(
  State(s): State<AppState>,
  Extension(a): Extension<AuthUser>,
  Extension(t): Extension<TraceId>,
  Json(req): Json<IdRequest>,
) -> Response {
  let r = with_conn(&s.db, move |c| {
    if c.execute(
      "UPDATE t_feed SET deleted=1,updated_time=?1 WHERE id=?2 AND user_id=?3 AND deleted=0",
      params![crate::dto::now_iso(), req.id, a.user_id],
    )? == 0
    {
      return Err(AppError::not_found());
    }
    Ok(())
  })
  .await;
  respond(r, t)
}
pub async fn feed_test(
  State(s): State<AppState>,
  Extension(a): Extension<AuthUser>,
  Extension(t): Extension<TraceId>,
  Json(req): Json<IdRequest>,
) -> Response {
  let r = async {
    get_feed(&s.db, a.user_id, req.id).await?;
    s.scheduler.fetch_wait(req.id).await
  }
  .await;
  respond(r, t)
}
