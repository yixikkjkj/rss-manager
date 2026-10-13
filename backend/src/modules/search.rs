use crate::dto::respond;
use crate::{auth::AuthUser, db::with_conn, dto::TraceId, error::AppError, state::AppState};
use axum::extract::{Extension, Json, State};
use axum::response::Response;
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchSort {
  Relevance,
  Newest,
}
#[derive(Deserialize)]
pub struct SearchRequest {
  pub q: String,
  pub feed_id: Option<i64>,
  pub folder_id: Option<i64>,
  pub page: Option<i32>,
  pub page_size: Option<i32>,
  pub sort: Option<SearchSort>,
  pub date_from: Option<String>,
  pub date_to: Option<String>,
}
#[derive(Serialize)]
pub struct SearchItemVO {
  pub article_id: i64,
  pub feed_id: i64,
  pub is_read: bool,
  pub title: String,
  pub snippet: String,
  pub feed_title: String,
  pub published_at: Option<String>,
}
#[derive(Serialize)]
pub struct SearchResultVO {
  pub total: i64,
  pub items: Vec<SearchItemVO>,
  pub took_ms: u128,
}
fn match_query(q: &str) -> String {
  q.split_whitespace()
    .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
    .collect::<Vec<_>>()
    .join(" AND ")
}
pub async fn search_query(
  State(s): State<AppState>,
  Extension(a): Extension<AuthUser>,
  Extension(t): Extension<TraceId>,
  Json(req): Json<SearchRequest>,
) -> Response {
  let started = std::time::Instant::now();
  let r=async{
  if req.q.trim().is_empty() || req.q.chars().count()>200{return Err(AppError::invalid("关键词须为 1–200 个字符"));}
  for date in [&req.date_from,&req.date_to].into_iter().flatten(){
   if chrono::NaiveDate::parse_from_str(date,"%Y-%m-%d").is_err(){return Err(AppError::invalid("日期格式应为 YYYY-MM-DD"));}
  }
  if matches!((&req.date_from,&req.date_to),(Some(from),Some(to)) if from>to){return Err(AppError::invalid("开始日期不能晚于结束日期"));}
  let (total,items)=with_conn(&s.db,move|c|{
   super::folders::check_folder(c,a.user_id,req.folder_id)?;
   let (offset,size)=crate::dto::PageRequest{page:req.page,page_size:req.page_size}.bounds();
   // Unicode61 does not segment Chinese phrases. Use a literal substring search
   // for CJK queries; Latin queries retain FTS ranking and indexes.
   let cjk=req.q.chars().any(|ch|('\u{3400}'..='\u{9fff}').contains(&ch));
   let query=if cjk{req.q.trim().to_owned()}else{match_query(&req.q)};
   let source=if cjk{
    "FROM t_article a JOIN t_feed f ON f.id=a.feed_id WHERE
     (instr(a.title,?1)>0 OR instr(a.content,?1)>0 OR instr(COALESCE(a.full_content,''),?1)>0)"
   }else{
    "FROM t_article_fts JOIN t_article a ON a.id=t_article_fts.rowid JOIN t_feed f ON f.id=a.feed_id WHERE t_article_fts MATCH ?1"
   };
   let filters="AND a.deleted=0 AND f.user_id=?2 AND (?3 IS NULL OR (f.deleted=0 AND f.folder_id IN (WITH RECURSIVE tree(id) AS (SELECT id FROM t_folder WHERE id=?3 AND user_id=?2 AND deleted=0 UNION ALL SELECT x.id FROM t_folder x JOIN tree ON x.parent_id=tree.id WHERE x.user_id=?2 AND x.deleted=0) SELECT id FROM tree)) OR EXISTS(SELECT 1 FROM t_article_folder af WHERE af.article_id=a.id AND af.user_id=?2 AND af.folder_id IN (WITH RECURSIVE tree(id) AS (SELECT id FROM t_folder WHERE id=?3 AND user_id=?2 AND deleted=0 UNION ALL SELECT x.id FROM t_folder x JOIN tree ON x.parent_id=tree.id WHERE x.user_id=?2 AND x.deleted=0) SELECT id FROM tree))) AND (?4 IS NULL OR a.feed_id=?4)
    AND (?5 IS NULL OR substr(COALESCE(a.published_at,a.created_time),1,10)>=?5)
    AND (?6 IS NULL OR substr(COALESCE(a.published_at,a.created_time),1,10)<=?6)";
   let total:i64=c.query_row(&format!("SELECT count(*) {source} {filters}"),params![query,a.user_id,req.folder_id,req.feed_id,req.date_from,req.date_to],|r|r.get(0))?;
   let order=if cjk || matches!(req.sort,Some(SearchSort::Newest)){"COALESCE(a.published_at,a.created_time) DESC,a.id DESC"}else{"bm25(t_article_fts),a.id DESC"};
   let snippet=if cjk{"COALESCE(a.full_content,a.content,'')"}else{"snippet(t_article_fts,-1,'','','…',32)"};
   let mut stmt=c.prepare(&format!("SELECT a.id,a.title,{snippet},f.title,a.published_at,a.feed_id,EXISTS(SELECT 1 FROM t_article_read r WHERE r.article_id=a.id AND r.user_id=?2) {source} {filters} ORDER BY {order} LIMIT ?7 OFFSET ?8"))?;
   let items=stmt.query_map(params![query,a.user_id,req.folder_id,req.feed_id,req.date_from,req.date_to,size,offset],|r|{
    let raw:String=r.get(2)?;
    Ok(SearchItemVO{article_id:r.get(0)?,title:r.get(1)?,snippet:crate::content::plain_text(&raw,400),feed_title:r.get(3)?,published_at:r.get(4)?,feed_id:r.get(5)?,is_read:r.get(6)?})
   })?.collect::<Result<Vec<_>,_>>()?;
   Ok((total,items))
  }).await?;
  Ok(SearchResultVO{total,items,took_ms:started.elapsed().as_millis()})
 }.await;
  respond(r, t)
}
