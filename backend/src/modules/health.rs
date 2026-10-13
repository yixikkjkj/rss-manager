use axum::extract::{Extension, Json, State};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use crate::auth::AuthUser;
use crate::db::{with_conn, Db};
use crate::dto::{Resp, TraceId};
use crate::error::ApiResult;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct FeedHealthRequest {
  pub id: i64,
}
#[derive(Debug, Deserialize)]
pub struct FeedRetryRequest {
  pub id: i64,
}

#[derive(Debug, Serialize)]
pub struct FeedHealthVO {
  pub feed_id: i64,
  pub title: String,
  pub health_status: String,
  pub consecutive_failures: i32,
  pub last_fetch_at: Option<String>,
  pub last_error: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct HealthOverviewVO {
  pub total: i32,
  pub healthy: i32,
  pub failing: i32,
  pub dead: i32,
  pub feeds: Vec<FeedHealthVO>,
}

async fn feed_health(db: &Db, user_id: i64, feed_id: i64) -> ApiResult<FeedHealthVO> {
  with_conn(db, move |c| {
    Ok(c.query_row(
      "SELECT f.id, f.title, f.health_status, f.consecutive_failures, f.last_fetch_at, \
             (SELECT error_message FROM t_fetch_log l WHERE l.feed_id = f.id AND l.result NOT IN ('success','not_modified') \
              ORDER BY l.id DESC LIMIT 1) \
             FROM t_feed f WHERE f.id = ?1 AND f.user_id = ?2 AND f.deleted = 0",
      rusqlite::params![feed_id, user_id],
      |r| {
        Ok(FeedHealthVO {
          feed_id: r.get(0)?,
          title: r.get(1)?,
          health_status: r.get(2)?,
          consecutive_failures: r.get(3)?,
          last_fetch_at: r.get(4)?,
          last_error: r.get(5)?,
        })
      },
    )?)
  })
  .await
}

pub async fn health_overview(
  State(state): State<AppState>,
  Extension(auth): Extension<AuthUser>,
  Extension(tid): Extension<TraceId>,
) -> Response {
  match with_conn(&state.db, move |c| {
    let mut stmt = c.prepare("SELECT id, health_status FROM t_feed WHERE user_id = ?1 AND deleted = 0")?;
    let ids: Vec<(i64, String)> = stmt
      .query_map(rusqlite::params![auth.user_id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
      .collect::<Result<Vec<_>, _>>()?;
    let total = ids.len() as i32;
    let healthy = ids.iter().filter(|(_, s)| s == "healthy").count() as i32;
    let failing = ids.iter().filter(|(_, s)| s == "failing").count() as i32;
    let dead = ids.iter().filter(|(_, s)| s == "dead").count() as i32;
    Ok((ids, total, healthy, failing, dead))
  })
  .await
  {
    Ok((ids, total, healthy, failing, dead)) => {
      let mut feeds = Vec::new();
      for (feed_id, _) in ids {
        if let Ok(vo) = feed_health(&state.db, auth.user_id, feed_id).await {
          feeds.push(vo);
        }
      }
      let vo = HealthOverviewVO {
        total,
        healthy,
        failing,
        dead,
        feeds,
      };
      Resp::ok(vo, tid.0).into_response()
    },
    Err(e) => e.with_trace(&tid.0).into_response(),
  }
}

pub async fn feed_health_detail(
  State(state): State<AppState>,
  Extension(auth): Extension<AuthUser>,
  Extension(tid): Extension<TraceId>,
  Json(req): Json<FeedHealthRequest>,
) -> Response {
  match feed_health(&state.db, auth.user_id, req.id).await {
    Ok(vo) => Resp::ok(vo, tid.0).into_response(),
    Err(e) => e.with_trace(&tid.0).into_response(),
  }
}

pub async fn feed_retry(
  State(state): State<AppState>,
  Extension(auth): Extension<AuthUser>,
  Extension(tid): Extension<TraceId>,
  Json(req): Json<FeedRetryRequest>,
) -> Response {
  // 重置失败计数 + 重新排期（立即抓取）
  let n = with_conn(&state.db, move |c| {
    let now = crate::dto::now_iso();
    Ok(c.execute(
      "UPDATE t_feed SET health_status = 'healthy', consecutive_failures = 0, next_fetch_at = ?1, updated_time = ?1 \
             WHERE id = ?2 AND user_id = ?3 AND deleted = 0",
      rusqlite::params![now, req.id, auth.user_id],
    )?)
  })
  .await;

  match n {
    Ok(0) => crate::error::AppError::group_not_found()
      .with_trace(&tid.0)
      .into_response(),
    Ok(_) => {
      if let Err(e) = state.scheduler.trigger(Some(req.id)).await {
        return e.with_trace(&tid.0).into_response();
      }
      Resp::ok((), tid.0).into_response()
    },
    Err(e) => e.with_trace(&tid.0).into_response(),
  }
}
