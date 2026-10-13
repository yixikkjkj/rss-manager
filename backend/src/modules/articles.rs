use crate::dto::respond;
use crate::{
  auth::AuthUser,
  db::{with_conn, Db},
  dto::TraceId,
  error::{ApiResult, AppError},
  state::AppState,
};
use axum::extract::{Extension, Json, State};
use axum::response::Response;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct ArticleListRequest {
  pub folder_id: Option<i64>,
  pub feed_id: Option<i64>,
  pub unread_only: Option<bool>,
  pub page: Option<i32>,
  pub page_size: Option<i32>,
}
#[derive(Deserialize)]
pub struct IdRequest {
  pub id: i64,
}
#[derive(Deserialize)]
pub struct ReadAllRequest {
  pub folder_id: Option<i64>,
  pub feed_id: Option<i64>,
}
#[derive(Deserialize)]
pub struct ContentRequest {
  pub id: i64,
  #[serde(default)]
  pub retry: bool,
}
#[derive(Serialize)]
pub struct ArticleListItemVO {
  pub id: i64,
  pub feed_id: i64,
  pub feed_title: String,
  pub title: String,
  pub author: Option<String>,
  pub published_at: Option<String>,
  pub is_read: bool,
  pub summary: String,
}
#[derive(Serialize)]
pub struct ArticleListVO {
  pub items: Vec<ArticleListItemVO>,
  pub total: i64,
  pub has_more: bool,
}
#[derive(Serialize)]
pub struct ArticleDetailVO {
  pub id: i64,
  pub feed_id: i64,
  pub feed_title: String,
  pub title: String,
  pub author: Option<String>,
  pub link: String,
  pub published_at: Option<String>,
  pub content: String,
  pub full_content: Option<String>,
  pub full_content_status: String,
  pub use_readability: bool,
  pub is_read: bool,
  pub folder_ids: Vec<i64>,
}
#[derive(Serialize)]
pub struct FolderUnreadVO {
  pub folder_id: i64,
  pub count: i64,
}
#[derive(Serialize)]
pub struct UnreadCountVO {
  pub total: i64,
  pub folders: Vec<FolderUnreadVO>,
}

pub(crate) const LIST_COLS: &str =
  "a.id,a.feed_id,f.title,a.title,a.author,a.published_at,a.content,(r.article_id IS NOT NULL)";
pub(crate) fn map_list(r: &rusqlite::Row) -> rusqlite::Result<ArticleListItemVO> {
  let html: Option<String> = r.get(6)?;
  Ok(ArticleListItemVO {
    id: r.get(0)?,
    feed_id: r.get(1)?,
    feed_title: r.get(2)?,
    title: r.get(3)?,
    author: r.get(4)?,
    published_at: r.get(5)?,
    summary: crate::content::plain_text(html.as_deref().unwrap_or(""), 180),
    is_read: r.get(7)?,
  })
}
pub(crate) const SCOPE_CTE: &str = "WITH RECURSIVE subtree(id) AS (
 SELECT id FROM t_folder WHERE id=?2 AND user_id=?1 AND deleted=0
 UNION ALL SELECT f.id FROM t_folder f JOIN subtree s ON f.parent_id=s.id WHERE f.user_id=?1 AND f.deleted=0)";
pub(crate) const SCOPE_FROM: &str = "FROM t_article a JOIN t_feed f ON f.id=a.feed_id
 LEFT JOIN t_article_read r ON r.article_id=a.id AND r.user_id=?1
 WHERE a.deleted=0 AND f.user_id=?1
 AND ((?2 IS NULL AND f.deleted=0) OR (?2 IS NOT NULL AND (
   (f.deleted=0 AND f.folder_id IN (SELECT id FROM subtree)) OR EXISTS(
   SELECT 1 FROM t_article_folder af WHERE af.article_id=a.id AND af.user_id=?1 AND af.folder_id IN (SELECT id FROM subtree)))))
 AND (?3 IS NULL OR a.feed_id=?3) AND (?4=0 OR r.article_id IS NULL)";
pub(crate) fn list_articles(c: &rusqlite::Connection, user: i64, req: ArticleListRequest) -> ApiResult<ArticleListVO> {
  super::folders::check_folder(c, user, req.folder_id)?;
  let (offset, size) = crate::dto::PageRequest {
    page: req.page,
    page_size: req.page_size,
  }
  .bounds();
  let unread = req.unread_only.unwrap_or(false);
  let total = c.query_row(
    &format!("{SCOPE_CTE} SELECT count(*) {SCOPE_FROM}"),
    params![user, req.folder_id, req.feed_id, unread],
    |r| r.get::<_, i64>(0),
  )?;
  let mut stmt=c.prepare(&format!("{SCOPE_CTE} SELECT {LIST_COLS} {SCOPE_FROM} ORDER BY COALESCE(a.published_at,a.created_time) DESC,a.id DESC LIMIT ?5 OFFSET ?6"))?;
  let items = stmt
    .query_map(params![user, req.folder_id, req.feed_id, unread, size, offset], map_list)?
    .collect::<Result<Vec<_>, _>>()?;
  Ok(ArticleListVO {
    items,
    total,
    has_more: offset + size < total,
  })
}
pub async fn article_list(
  State(s): State<AppState>,
  Extension(a): Extension<AuthUser>,
  Extension(t): Extension<TraceId>,
  Json(req): Json<ArticleListRequest>,
) -> Response {
  respond(with_conn(&s.db, move |c| list_articles(c, a.user_id, req)).await, t)
}

pub async fn get_article(db: &Db, user: i64, id: i64) -> ApiResult<ArticleDetailVO> {
  with_conn(db,move|c|{
  let mut article=c.query_row("SELECT a.id,a.feed_id,f.title,a.title,a.author,a.link,a.published_at,
   a.content,a.full_content,a.full_content_status,f.use_readability,
   EXISTS(SELECT 1 FROM t_article_read WHERE article_id=a.id AND user_id=?1)
   FROM t_article a JOIN t_feed f ON a.feed_id=f.id WHERE a.id=?2 AND f.user_id=?1 AND a.deleted=0",
   params![user,id],|r|{
    let link:Option<String>=r.get(5)?;
    let content:Option<String>=r.get(7)?;
    let full:Option<String>=r.get(8)?;
    Ok(ArticleDetailVO{id:r.get(0)?,feed_id:r.get(1)?,feed_title:r.get(2)?,title:r.get(3)?,author:r.get(4)?,
     content:crate::content::sanitize(content.as_deref().unwrap_or(""),link.as_deref()),
     full_content:full.map(|v|crate::content::sanitize(&v,link.as_deref())),
     link:link.unwrap_or_default(),published_at:r.get(6)?,full_content_status:r.get(9)?,use_readability:r.get(10)?,
     is_read:r.get(11)?,folder_ids:vec![]})
   }).optional()?.ok_or_else(AppError::article_not_found)?;
  let mut stmt=c.prepare("SELECT af.folder_id FROM t_article_folder af JOIN t_folder f ON f.id=af.folder_id WHERE af.user_id=?1 AND af.article_id=?2 AND f.deleted=0")?;
  article.folder_ids=stmt.query_map(params![user,id],|r|r.get(0))?.collect::<Result<Vec<_>,_>>()?;
  Ok(article)
 }).await
}
pub async fn article_get(
  State(s): State<AppState>,
  Extension(a): Extension<AuthUser>,
  Extension(t): Extension<TraceId>,
  Json(req): Json<IdRequest>,
) -> Response {
  respond(get_article(&s.db, a.user_id, req.id).await, t)
}
async fn set_read(db: &Db, user: i64, id: i64, read: bool) -> ApiResult<()> {
  with_conn(db,move|c|{
  let exists:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM t_article a JOIN t_feed f ON f.id=a.feed_id WHERE a.id=?1 AND f.user_id=?2 AND a.deleted=0)",params![id,user],|r|r.get(0))?;
  if !exists{return Err(AppError::article_not_found());}
  if read{
   c.execute("INSERT OR IGNORE INTO t_article_read(user_id,article_id,read_at,created_time) VALUES(?1,?2,?3,?3)",params![user,id,crate::dto::now_iso()])?;
  }else{c.execute("DELETE FROM t_article_read WHERE user_id=?1 AND article_id=?2",params![user,id])?;}
  Ok(())
 }).await
}
pub async fn article_mark_read(
  State(s): State<AppState>,
  Extension(a): Extension<AuthUser>,
  Extension(t): Extension<TraceId>,
  Json(req): Json<IdRequest>,
) -> Response {
  respond(set_read(&s.db, a.user_id, req.id, true).await, t)
}
pub async fn article_mark_unread(
  State(s): State<AppState>,
  Extension(a): Extension<AuthUser>,
  Extension(t): Extension<TraceId>,
  Json(req): Json<IdRequest>,
) -> Response {
  respond(set_read(&s.db, a.user_id, req.id, false).await, t)
}
pub async fn article_read_all(
  State(s): State<AppState>,
  Extension(a): Extension<AuthUser>,
  Extension(t): Extension<TraceId>,
  Json(req): Json<ReadAllRequest>,
) -> Response {
  let r = with_conn(&s.db, move |c| {
    super::folders::check_folder(c, a.user_id, req.folder_id)?;
    c.execute(
      &format!(
        "{SCOPE_CTE} INSERT OR IGNORE INTO t_article_read(user_id,article_id,read_at,created_time)
      SELECT ?1,a.id,?5,?5 {SCOPE_FROM}"
      ),
      params![a.user_id, req.folder_id, req.feed_id, false, crate::dto::now_iso()],
    )?;
    Ok(())
  })
  .await;
  respond(r, t)
}
pub async fn unread_count(
  State(s): State<AppState>,
  Extension(a): Extension<AuthUser>,
  Extension(t): Extension<TraceId>,
) -> Response {
  let r = with_conn(&s.db, move |c| {
    let total: i64 = c.query_row("SELECT count(*) FROM t_article x JOIN t_feed f ON f.id=x.feed_id
      WHERE f.user_id=?1 AND f.deleted=0 AND x.deleted=0
      AND NOT EXISTS(SELECT 1 FROM t_article_read r WHERE r.user_id=?1 AND r.article_id=x.id)", [a.user_id], |r| r.get(0))?;
    let mut stmt = c.prepare("WITH RECURSIVE descendants(root,id) AS (
      SELECT id,id FROM t_folder WHERE user_id=?1 AND deleted=0
      UNION ALL SELECT d.root,f.id FROM descendants d JOIN t_folder f ON f.parent_id=d.id WHERE f.user_id=?1 AND f.deleted=0
    ), contents(folder_id,article_id) AS (
      SELECT d.root,x.id FROM descendants d JOIN t_feed f ON f.folder_id=d.id JOIN t_article x ON x.feed_id=f.id WHERE f.user_id=?1 AND f.deleted=0 AND x.deleted=0
      UNION SELECT d.root,x.id FROM descendants d JOIN t_article_folder af ON af.folder_id=d.id JOIN t_article x ON x.id=af.article_id JOIN t_feed f ON f.id=x.feed_id WHERE af.user_id=?1 AND f.user_id=?1 AND x.deleted=0
    ) SELECT folder_id,count(*) FROM contents x
      WHERE NOT EXISTS(SELECT 1 FROM t_article_read r WHERE r.user_id=?1 AND r.article_id=x.article_id) GROUP BY folder_id")?;
    let folders = stmt.query_map([a.user_id], |r| Ok(FolderUnreadVO { folder_id: r.get(0)?, count: r.get(1)? }))?.collect::<Result<Vec<_>, _>>()?;
    Ok(UnreadCountVO { total, folders })
  })
  .await;
  respond(r, t)
}
pub async fn article_fetch_content(
  State(s): State<AppState>,
  Extension(a): Extension<AuthUser>,
  Extension(t): Extension<TraceId>,
  Json(req): Json<ContentRequest>,
) -> Response {
  let r = async {
    // Serialize extraction for this single-user app. Re-check the cache after locking:
    // duplicate requests (including overlapping client requests) must not fetch the same page twice.
    let _guard = s.content_lock.lock().await;
    let article = get_article(&s.db, a.user_id, req.id).await?;
    if !article.use_readability
      || article.full_content.is_some()
      || (!req.retry && article.full_content_status == "failed")
    {
      return Ok(article);
    }
    let extracted = crate::fetch::fetch_full_content(&s.scheduler.client(), &article.link).await;
    let (full, status) = match extracted {
      Ok(html) => (Some(html), "success"),
      Err(e) => {
        tracing::info!(article_id = req.id, code = e.code, "full content unavailable");
        (None, "failed")
      },
    };
    with_conn(&s.db, move |c| {
      c.execute(
        "UPDATE t_article SET full_content=?1,full_content_status=?2,updated_time=?3 WHERE id=?4",
        params![full, status, crate::dto::now_iso(), req.id],
      )?;
      Ok(())
    })
    .await?;
    get_article(&s.db, a.user_id, req.id).await
  }
  .await;
  respond(r, t)
}
