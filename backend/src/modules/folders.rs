use super::{
  articles::{list_articles, ArticleListRequest, ArticleListVO},
  subscriptions::{validate_name, IdRequest, Items},
};
use crate::{
  auth::AuthUser,
  db::with_conn,
  dto::{respond, TraceId},
  error::{ApiResult, AppError},
  state::AppState,
};
use axum::extract::{Extension, Json, State};
use axum::response::Response;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct CreateRequest {
  pub name: String,
  pub parent_id: Option<i64>,
}
#[derive(Deserialize)]
pub struct RenameRequest {
  pub id: i64,
  pub name: String,
}
#[derive(Deserialize)]
pub struct MoveRequest {
  pub id: i64,
  pub parent_id: Option<i64>,
}
#[derive(Deserialize)]
pub struct ListRequest {
  pub folder_id: i64,
  pub page: Option<i32>,
  pub page_size: Option<i32>,
}
#[derive(Deserialize)]
pub struct FavoriteRequest {
  pub article_id: i64,
  pub folder_id: i64,
}
#[derive(Serialize)]
pub struct FolderVO {
  pub id: i64,
  pub parent_id: Option<i64>,
  pub name: String,
  pub sort_order: i64,
  pub article_count: i64,
  pub unread_count: i64,
  pub feed_count: i64,
  pub legacy_group_id: Option<i64>,
}

pub fn check_folder(c: &Connection, user: i64, folder: Option<i64>) -> ApiResult<()> {
  if let Some(id) = folder {
    let found: bool = c.query_row(
      "SELECT EXISTS(SELECT 1 FROM t_folder WHERE id=?1 AND user_id=?2 AND deleted=0)",
      params![id, user],
      |r| r.get(0),
    )?;
    if !found {
      return Err(AppError::folder_not_found());
    }
  }
  Ok(())
}
fn validate_parent(c: &Connection, user: i64, parent: Option<i64>, moving: Option<i64>) -> ApiResult<()> {
  check_folder(c, user, parent)?;
  let depth: i64 = c.query_row("WITH RECURSIVE ancestors(id,parent_id) AS (
    SELECT id,parent_id FROM t_folder WHERE id=?1 AND user_id=?2 AND deleted=0
    UNION ALL SELECT f.id,f.parent_id FROM t_folder f JOIN ancestors a ON a.parent_id=f.id WHERE f.user_id=?2 AND f.deleted=0)
    SELECT count(*) FROM ancestors",params![parent,user],|r|r.get(0))?;
  let (cycle,height): (bool,i64) = c.query_row("WITH RECURSIVE subtree(id,depth) AS (
    SELECT id,1 FROM t_folder WHERE id=?1 AND user_id=?2 AND deleted=0
    UNION ALL SELECT f.id,s.depth+1 FROM t_folder f JOIN subtree s ON f.parent_id=s.id WHERE f.user_id=?2 AND f.deleted=0)
    SELECT EXISTS(SELECT 1 FROM subtree WHERE id=?3),COALESCE(max(depth),1) FROM subtree",params![moving,user,parent],|r|Ok((r.get(0)?,r.get(1)?)))?;
  if cycle {
    return Err(AppError::invalid("不能将文件夹移动到自身或其子文件夹中"));
  }
  if depth + height > 64 {
    return Err(AppError::invalid("文件夹最多支持 64 层"));
  }
  Ok(())
}
pub fn ensure_folder(c: &Connection, user: i64, parent: Option<i64>, name: &str) -> ApiResult<i64> {
  let name = validate_name(name)?;
  validate_parent(c, user, parent, None)?;
  if let Some(id) = c
    .query_row(
      "SELECT id FROM t_folder WHERE user_id=?1 AND parent_id IS ?2 AND name=?3 AND deleted=0",
      params![user, parent, name],
      |r| r.get(0),
    )
    .optional()?
  {
    return Ok(id);
  }
  create_folder(c, user, parent, &name)
}
fn create_folder(c: &Connection, user: i64, parent: Option<i64>, name: &str) -> ApiResult<i64> {
  validate_parent(c, user, parent, None)?;
  let exists: bool = c.query_row(
    "SELECT EXISTS(SELECT 1 FROM t_folder WHERE user_id=?1 AND parent_id IS ?2 AND name=?3 AND deleted=0)",
    params![user, parent, name],
    |r| r.get(0),
  )?;
  if exists {
    return Err(AppError::folder_exists());
  }
  let deleted: Option<i64> = c
    .query_row(
      "SELECT id FROM t_folder WHERE user_id=?1 AND parent_id IS ?2 AND name=?3 AND deleted=1 ORDER BY id DESC LIMIT 1",
      params![user, parent, name],
      |r| r.get(0),
    )
    .optional()?;
  if let Some(id) = deleted {
    c.execute("UPDATE t_folder SET deleted=0,updated_time=?1 WHERE id=?2", params![crate::dto::now_iso(), id])?;
    Ok(id)
  } else {
    c.execute(
      "INSERT INTO t_folder(user_id,parent_id,name,created_time,updated_time) VALUES(?1,?2,?3,?4,?4)",
      params![user, parent, name, crate::dto::now_iso()],
    )?;
    Ok(c.last_insert_rowid())
  }
}
pub async fn folder_list(
  State(s): State<AppState>,
  Extension(a): Extension<AuthUser>,
  Extension(t): Extension<TraceId>,
) -> Response {
  let r=with_conn(&s.db,move|c|{
    let mut stmt=c.prepare("WITH RECURSIVE descendants(root,id) AS (
      SELECT id,id FROM t_folder WHERE user_id=?1 AND deleted=0
      UNION ALL SELECT d.root,f.id FROM descendants d JOIN t_folder f ON f.parent_id=d.id WHERE f.user_id=?1 AND f.deleted=0
    ), contents(folder_id,article_id) AS (
      SELECT d.root,x.id FROM descendants d JOIN t_feed f ON f.folder_id=d.id JOIN t_article x ON x.feed_id=f.id WHERE f.user_id=?1 AND f.deleted=0 AND x.deleted=0
      UNION SELECT d.root,x.id FROM descendants d JOIN t_article_folder af ON af.folder_id=d.id JOIN t_article x ON x.id=af.article_id JOIN t_feed f ON f.id=x.feed_id WHERE af.user_id=?1 AND f.user_id=?1 AND x.deleted=0
    ) SELECT f.id,f.parent_id,f.name,f.sort_order,
      (SELECT count(*) FROM contents WHERE folder_id=f.id),
      (SELECT count(*) FROM contents x WHERE x.folder_id=f.id AND NOT EXISTS(SELECT 1 FROM t_article_read r WHERE r.article_id=x.article_id AND r.user_id=?1)),
      (SELECT count(*) FROM t_feed x WHERE x.folder_id=f.id AND x.user_id=?1 AND x.deleted=0),
      (SELECT group_id FROM t_legacy_group_folder WHERE folder_id=f.id)
      FROM t_folder f WHERE f.user_id=?1 AND f.deleted=0 ORDER BY f.sort_order,f.id")?;
    let items=stmt.query_map([a.user_id],|r|Ok(FolderVO{id:r.get(0)?,parent_id:r.get(1)?,name:r.get(2)?,sort_order:r.get(3)?,article_count:r.get(4)?,unread_count:r.get(5)?,feed_count:r.get(6)?,legacy_group_id:r.get(7)?}))?.collect::<Result<Vec<_>,_>>()?;
    Ok(Items{items})
  }).await;
  respond(r, t)
}
pub async fn folder_create(
  State(s): State<AppState>,
  Extension(a): Extension<AuthUser>,
  Extension(t): Extension<TraceId>,
  Json(req): Json<CreateRequest>,
) -> Response {
  let r = async {
    let name = validate_name(&req.name)?;
    with_conn(&s.db, move |c| {
      let id = create_folder(c, a.user_id, req.parent_id, &name)?;
      Ok(FolderVO {
        id,
        parent_id: req.parent_id,
        name,
        sort_order: 0,
        article_count: 0,
        unread_count: 0,
        feed_count: 0,
        legacy_group_id: None,
      })
    })
    .await
  }
  .await;
  respond(r, t)
}
pub async fn folder_rename(
  State(s): State<AppState>,
  Extension(a): Extension<AuthUser>,
  Extension(t): Extension<TraceId>,
  Json(req): Json<RenameRequest>,
) -> Response {
  let r = async {
    let name = validate_name(&req.name)?;
    with_conn(&s.db, move |c| {
      check_folder(c, a.user_id, Some(req.id))?;
      c.execute(
        "UPDATE t_folder SET name=?1,updated_time=?2 WHERE id=?3 AND user_id=?4",
        params![name, crate::dto::now_iso(), req.id, a.user_id],
      )?;
      Ok(())
    })
    .await
  }
  .await;
  respond(r, t)
}
pub async fn folder_move(
  State(s): State<AppState>,
  Extension(a): Extension<AuthUser>,
  Extension(t): Extension<TraceId>,
  Json(req): Json<MoveRequest>,
) -> Response {
  let r = with_conn(&s.db, move |c| {
    let tx = c.transaction()?;
    check_folder(&tx, a.user_id, Some(req.id))?;
    validate_parent(&tx, a.user_id, req.parent_id, Some(req.id))?;
    tx.execute(
      "UPDATE t_folder SET parent_id=?1,updated_time=?2 WHERE id=?3 AND user_id=?4",
      params![req.parent_id, crate::dto::now_iso(), req.id, a.user_id],
    )?;
    tx.commit()?;
    Ok(())
  })
  .await;
  respond(r, t)
}
pub async fn folder_delete(
  State(s): State<AppState>,
  Extension(a): Extension<AuthUser>,
  Extension(t): Extension<TraceId>,
  Json(req): Json<IdRequest>,
) -> Response {
  let r=with_conn(&s.db,move|c| {
    let tx=c.transaction()?;
    check_folder(&tx,a.user_id,Some(req.id))?;
    let occupied:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM t_folder WHERE parent_id=?1 AND user_id=?2 AND deleted=0) OR EXISTS(SELECT 1 FROM t_feed WHERE folder_id=?1 AND user_id=?2 AND deleted=0)",params![req.id,a.user_id],|r|r.get(0))?;
    if occupied { return Err(AppError::invalid("请先移出子文件夹和订阅源，再删除文件夹")); }
    tx.execute("UPDATE t_folder SET deleted=1,updated_time=?1 WHERE id=?2",params![crate::dto::now_iso(),req.id])?;
    tx.execute("DELETE FROM t_article_folder WHERE folder_id=?1 AND user_id=?2",params![req.id,a.user_id])?;
    tx.commit()?; Ok(())
  }).await;
  respond(r, t)
}
pub async fn folder_article_list(
  State(s): State<AppState>,
  Extension(a): Extension<AuthUser>,
  Extension(t): Extension<TraceId>,
  Json(req): Json<ListRequest>,
) -> Response {
  let r: ApiResult<ArticleListVO> = with_conn(&s.db, move |c| {
    list_articles(
      c,
      a.user_id,
      ArticleListRequest {
        folder_id: Some(req.folder_id),
        feed_id: None,
        unread_only: None,
        page: req.page,
        page_size: req.page_size,
      },
    )
  })
  .await;
  respond(r, t)
}
pub async fn article_favorite(
  State(s): State<AppState>,
  Extension(a): Extension<AuthUser>,
  Extension(t): Extension<TraceId>,
  Json(req): Json<FavoriteRequest>,
) -> Response {
  let r=with_conn(&s.db,move|c| {
    check_folder(c,a.user_id,Some(req.folder_id))?;
    let exists:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM t_article x JOIN t_feed f ON f.id=x.feed_id WHERE x.id=?1 AND f.user_id=?2 AND x.deleted=0)",params![req.article_id,a.user_id],|r|r.get(0))?;
    if !exists { return Err(AppError::article_not_found()); }
    c.execute("INSERT OR IGNORE INTO t_article_folder(user_id,article_id,folder_id,created_time) VALUES(?1,?2,?3,?4)",params![a.user_id,req.article_id,req.folder_id,crate::dto::now_iso()])?; Ok(())
  }).await;
  respond(r, t)
}
pub async fn article_unfavorite(
  State(s): State<AppState>,
  Extension(a): Extension<AuthUser>,
  Extension(t): Extension<TraceId>,
  Json(req): Json<FavoriteRequest>,
) -> Response {
  respond(
    with_conn(&s.db, move |c| {
      c.execute(
        "DELETE FROM t_article_folder WHERE user_id=?1 AND article_id=?2 AND folder_id=?3",
        params![a.user_id, req.article_id, req.folder_id],
      )?;
      Ok(())
    })
    .await,
    t,
  )
}
