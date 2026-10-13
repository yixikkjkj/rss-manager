use axum::extract::{Extension, Json, State};
use axum::http::{header, HeaderMap};
use axum::response::{IntoResponse, Response};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::auth::{self, AuthUser};
use crate::db::with_conn;
use crate::dto::{Resp, TraceId};
use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
  pub username: String,
  pub password: String,
}
#[derive(Debug, Serialize)]
pub struct LoginResponse {
  pub user_id: i64,
  pub username: String,
}
#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
  pub old_password: String,
  pub new_password: String,
}
#[derive(Debug, Serialize)]
pub struct MeVO {
  pub user_id: i64,
  pub username: String,
}

fn validate_password(pw: &str) -> Result<(), AppError> {
  if pw.chars().count() < 12 || pw.len() > 1024 {
    return Err(AppError::bad_request());
  }
  let has_lower = pw.chars().any(|c| c.is_ascii_lowercase());
  let has_upper = pw.chars().any(|c| c.is_ascii_uppercase());
  let has_digit = pw.chars().any(|c| c.is_ascii_digit());
  let has_sym = pw.chars().any(|c| !c.is_alphanumeric());
  if !(has_lower && has_upper && has_digit && has_sym) {
    return Err(AppError::bad_request());
  }
  Ok(())
}

fn is_https(headers: &HeaderMap) -> bool {
  headers
    .get("x-forwarded-proto")
    .and_then(|v| v.to_str().ok())
    .map(|v| v.eq_ignore_ascii_case("https"))
    .unwrap_or(false)
}

pub async fn auth_login(
  State(state): State<AppState>,
  Extension(tid): Extension<TraceId>,
  headers: HeaderMap,
  Json(req): Json<LoginRequest>,
) -> Response {
  let limiter = auth::login_limiter();
  if req.username.len() > 200 || req.password.len() > 1024 {
    return AppError::bad_request().with_trace(&tid.0).into_response();
  }
  if !limiter.check(&req.username) {
    return AppError::rate_limited().with_trace(&tid.0).into_response();
  }

  let username = req.username.clone();
  let user: Option<(i64, String)> = match with_conn(&state.db, move |c| {
    Ok(
      c.query_row(
        "SELECT id, password_hash FROM t_user WHERE username = ?1 AND deleted = 0",
        rusqlite::params![username],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
      )
      .optional()?,
    )
  })
  .await
  {
    Ok(u) => u,
    Err(e) => return e.with_trace(&tid.0).into_response(),
  };

  let (user_id, password_hash) = match user {
    Some(v) => v,
    None => {
      limiter.record_failure(&req.username);
      return AppError::bad_credentials().with_trace(&tid.0).into_response();
    },
  };

  let verified = match auth::verify_password_async(req.password, password_hash).await {
    Ok(value) => value,
    Err(e) => return e.with_trace(&tid.0).into_response(),
  };
  if !verified {
    limiter.record_failure(&req.username);
    return AppError::bad_credentials().with_trace(&tid.0).into_response();
  }

  limiter.clear(&req.username);

  match auth::create_session(&state.db, user_id).await {
    Ok(token) => {
      let mut resp = Resp::ok(
        LoginResponse {
          user_id,
          username: req.username,
        },
        tid.0,
      )
      .into_response();
      resp.headers_mut().insert(
        header::SET_COOKIE,
        auth::set_cookie_header(&token, &state.config.session_secret, is_https(&headers)),
      );
      resp
    },
    Err(e) => e.with_trace(&tid.0).into_response(),
  }
}

pub async fn auth_logout(
  State(state): State<AppState>,
  Extension(_auth): Extension<AuthUser>,
  Extension(tid): Extension<TraceId>,
  headers: HeaderMap,
) -> Response {
  // 从 Cookie 取 token 并删除会话
  if let Some(cookie) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) {
    for part in cookie.split(';') {
      if let Some((k, v)) = part.trim().split_once('=') {
        if k == "rss_session" {
          if let Some(token) = auth::parse_cookie_token(v, &state.config.session_secret) {
            if let Err(e) = auth::delete_session_by_token(&state.db, &token).await {
              return e.with_trace(&tid.0).into_response();
            }
          }
        }
      }
    }
  }
  let mut resp = Resp::ok((), tid.0).into_response();
  resp
    .headers_mut()
    .insert(header::SET_COOKIE, auth::clear_cookie_header());
  resp
}

pub async fn auth_me(
  State(state): State<AppState>,
  Extension(auth): Extension<AuthUser>,
  Extension(tid): Extension<TraceId>,
) -> Response {
  let username: Option<String> = match with_conn(&state.db, move |c| {
    Ok(
      c.query_row("SELECT username FROM t_user WHERE id = ?1 AND deleted = 0", rusqlite::params![auth.user_id], |r| {
        r.get::<_, String>(0)
      })
      .optional()?,
    )
  })
  .await
  {
    Ok(u) => u,
    Err(e) => return e.with_trace(&tid.0).into_response(),
  };

  match username {
    Some(username) => Resp::ok(
      MeVO {
        user_id: auth.user_id,
        username,
      },
      tid.0,
    )
    .into_response(),
    None => AppError::unauthorized().with_trace(&tid.0).into_response(),
  }
}

pub async fn auth_change_password(
  State(state): State<AppState>,
  Extension(auth): Extension<AuthUser>,
  Extension(tid): Extension<TraceId>,
  Json(req): Json<ChangePasswordRequest>,
) -> Response {
  if let Err(e) = validate_password(&req.new_password) {
    return e.with_trace(&tid.0).into_response();
  }

  let password_hash: Option<String> = match with_conn(&state.db, move |c| {
    Ok(
      c.query_row(
        "SELECT password_hash FROM t_user WHERE id = ?1 AND deleted = 0",
        rusqlite::params![auth.user_id],
        |r| r.get::<_, String>(0),
      )
      .optional()?,
    )
  })
  .await
  {
    Ok(h) => h,
    Err(e) => return e.with_trace(&tid.0).into_response(),
  };

  let Some(current_hash) = password_hash else {
    return AppError::unauthorized().with_trace(&tid.0).into_response();
  };

  let verified = match auth::verify_password_async(req.old_password, current_hash).await {
    Ok(value) => value,
    Err(e) => return e.with_trace(&tid.0).into_response(),
  };
  if !verified {
    return AppError::bad_credentials().with_trace(&tid.0).into_response();
  }

  let new_hash = match auth::hash_password_async(req.new_password).await {
    Ok(h) => h,
    Err(e) => {
      tracing::error!(code = e.code, "hash_password failed");
      return AppError::internal().with_trace(&tid.0).into_response();
    },
  };

  let result = with_conn(&state.db, move |c| {
    let tx = c.transaction()?;
    tx.execute(
      "UPDATE t_user SET password_hash = ?1, updated_time = ?2 WHERE id = ?3",
      rusqlite::params![new_hash, crate::dto::now_iso(), auth.user_id],
    )?;
    tx.execute("DELETE FROM t_session WHERE user_id = ?1", rusqlite::params![auth.user_id])?;
    tx.commit()?;
    Ok(())
  })
  .await;

  match result {
    Ok(()) => Resp::ok((), tid.0).into_response(),
    Err(e) => e.with_trace(&tid.0).into_response(),
  }
}
