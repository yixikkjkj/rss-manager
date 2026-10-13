use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use axum::extract::Request;
use axum::extract::State;
use axum::http::header;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use hmac::{Hmac, Mac};
use rand::RngCore;
use rusqlite::OptionalExtension;
use sha2::{Digest, Sha256};

use crate::dto::TraceId;
use crate::error::AppError;
use crate::state::AppState;

const COOKIE_NAME: &str = "rss_session";
const SESSION_TTL_SEC: i64 = 7 * 24 * 3600; // 7 天

type HmacSha256 = Hmac<Sha256>;

/// 鉴权后注入到 request extensions 的用户身份
#[derive(Debug, Clone)]
pub struct AuthUser {
  pub user_id: i64,
}

// ============ 密码（argon2id） ============

pub fn hash_password(plaintext: &str) -> Result<String, argon2::password_hash::Error> {
  let salt = SaltString::generate(&mut OsRng);
  let argon2 = Argon2::default();
  Ok(argon2.hash_password(plaintext.as_bytes(), &salt)?.to_string())
}

pub fn verify_password(plaintext: &str, hash: &str) -> bool {
  match PasswordHash::new(hash) {
    Ok(parsed) => Argon2::default().verify_password(plaintext.as_bytes(), &parsed).is_ok(),
    Err(_) => false,
  }
}

pub async fn verify_password_async(plaintext: String, hash: String) -> Result<bool, AppError> {
  tokio::task::spawn_blocking(move || verify_password(&plaintext, &hash))
    .await
    .map_err(|_| AppError::internal())
}

pub async fn hash_password_async(plaintext: String) -> Result<String, AppError> {
  tokio::task::spawn_blocking(move || hash_password(&plaintext).map_err(|_| AppError::internal()))
    .await
    .map_err(|_| AppError::internal())?
}

// ============ 会话令牌 ============

pub fn generate_token() -> String {
  let mut bytes = [0u8; 32];
  rand::thread_rng().fill_bytes(&mut bytes);
  hex::encode(bytes)
}

pub fn hash_token(token: &str) -> String {
  let mut h = Sha256::new();
  h.update(token.as_bytes());
  hex::encode(h.finalize())
}

/// 用会话密钥对令牌做 HMAC 签名（cookie 值 = token.signature）
fn sign_token(token: &str, secret: &str) -> String {
  let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac key");
  mac.update(token.as_bytes());
  hex::encode(mac.finalize().into_bytes())
}

fn ct_eq(a: &[u8], b: &[u8]) -> bool {
  if a.len() != b.len() {
    return false;
  }
  let mut diff = 0u8;
  for (x, y) in a.iter().zip(b.iter()) {
    diff |= x ^ y;
  }
  diff == 0
}

/// 从 Cookie 解析出原始 token（校验 HMAC 签名，防篡改）
pub fn parse_cookie_token(cookie_value: &str, secret: &str) -> Option<String> {
  let (token, sig) = cookie_value.split_once('.')?;
  let expected = sign_token(token, secret);
  if ct_eq(expected.as_bytes(), sig.as_bytes()) {
    Some(token.to_string())
  } else {
    None
  }
}

pub fn session_cookie_value(token: &str, secret: &str) -> String {
  format!("{}.{}", token, sign_token(token, secret))
}

fn extract_cookie(req: &Request) -> Option<String> {
  let cookies = req.headers().get(header::COOKIE)?.to_str().ok()?;
  for part in cookies.split(';') {
    let part = part.trim();
    if let Some((k, v)) = part.split_once('=') {
      if k.trim() == COOKIE_NAME {
        return Some(v.trim().to_string());
      }
    }
  }
  None
}

// ============ 会话存储 ============

pub async fn create_session(db: &crate::db::Db, user_id: i64) -> Result<String, AppError> {
  let token = generate_token();
  let hash = hash_token(&token);
  let now = crate::dto::now_iso();
  let expires = chrono::Utc::now().timestamp() + SESSION_TTL_SEC;
  let expires_at = chrono::DateTime::<chrono::Utc>::from_timestamp(expires, 0)
    .map(|t| t.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
    .unwrap_or(now.clone());
  crate::db::with_conn(db, move |c| {
    c.execute(
      "INSERT INTO t_session (user_id, token_hash, expires_at, created_time) VALUES (?1, ?2, ?3, ?4)",
      rusqlite::params![user_id, hash, expires_at, now],
    )?;
    Ok(())
  })
  .await?;
  Ok(token)
}

/// 校验会话令牌，返回 user_id（未登录/过期/不存在返回 None）
pub async fn validate_session(db: &crate::db::Db, token: &str) -> Result<Option<i64>, AppError> {
  let hash = hash_token(token);
  let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
  crate::db::with_conn(db, move |c| {
    let uid: Option<i64> = c
      .query_row(
        "SELECT user_id FROM t_session WHERE token_hash = ?1 AND expires_at > ?2",
        rusqlite::params![hash, now],
        |r| r.get(0),
      )
      .optional()?;
    Ok(uid)
  })
  .await
}

pub async fn delete_session_by_token(db: &crate::db::Db, token: &str) -> Result<(), AppError> {
  let hash = hash_token(token);
  crate::db::with_conn(db, move |c| {
    c.execute("DELETE FROM t_session WHERE token_hash = ?1", rusqlite::params![hash])?;
    Ok(())
  })
  .await
}

// ============ 鉴权中间件 ============

// ============ 全局登录限流器 ============

static LOGIN_LIMITER: std::sync::OnceLock<LoginLimiter> = std::sync::OnceLock::new();

pub fn login_limiter() -> &'static LoginLimiter {
  LOGIN_LIMITER.get_or_init(LoginLimiter::new)
}

// ============ Cookie 头构建 ============

pub fn set_cookie_header(token: &str, secret: &str, secure: bool) -> axum::http::HeaderValue {
  let value = session_cookie_value(token, secret);
  let mut c = format!("{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=604800", COOKIE_NAME, value);
  if secure {
    c.push_str("; Secure");
  }
  axum::http::HeaderValue::from_str(&c).expect("valid cookie header")
}

pub fn clear_cookie_header() -> axum::http::HeaderValue {
  axum::http::HeaderValue::from_static("rss_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0")
}

pub async fn auth_middleware(State(state): State<AppState>, mut req: Request, next: Next) -> Response {
  let tid = req
    .extensions()
    .get::<TraceId>()
    .cloned()
    .unwrap_or_else(|| TraceId(String::new()))
    .0;

  let cookie = match extract_cookie(&req) {
    Some(c) => c,
    None => {
      return AppError::unauthorized().with_trace(&tid).into_response();
    },
  };

  let token = match parse_cookie_token(&cookie, &state.config.session_secret) {
    Some(t) => t,
    None => {
      return AppError::unauthorized().with_trace(&tid).into_response();
    },
  };

  match validate_session(&state.db, &token).await {
    Ok(Some(user_id)) => {
      req.extensions_mut().insert(AuthUser { user_id });
      next.run(req).await
    },
    Ok(None) => AppError::session_expired().with_trace(&tid).into_response(),
    Err(e) => e.with_trace(&tid).into_response(),
  }
}

/// 登录失败锁定：内存态计数（单进程），key = username
pub struct LoginLimiter {
  // (username -> (failure_count, last_failure_epoch_sec, locked_until_epoch_sec))
  inner: std::sync::Mutex<std::collections::HashMap<String, (u32, i64, i64)>>,
}

impl LoginLimiter {
  pub fn new() -> Self {
    Self {
      inner: std::sync::Mutex::new(std::collections::HashMap::new()),
    }
  }

  pub fn check(&self, username: &str) -> bool {
    let now = chrono::Utc::now().timestamp();
    let mut map = self.inner.lock().unwrap();
    map.retain(|_, (_, last, locked)| *locked > now || now - *last < 900);
    !matches!(map.get(username), Some(&(_, _, locked_until)) if locked_until > now)
  }

  pub fn record_failure(&self, username: &str) {
    let now = chrono::Utc::now().timestamp();
    let mut map = self.inner.lock().unwrap();
    if map.len() >= 1000 && !map.contains_key(username) {
      return;
    }
    let (count, _, _) = map.get(username).copied().unwrap_or((0, now, 0));
    let count = count + 1;
    let locked_until = if count >= 5 { now + 15 * 60 } else { 0 };
    map.insert(username.to_string(), (count, now, locked_until));
  }

  pub fn clear(&self, username: &str) {
    let mut map = self.inner.lock().unwrap();
    map.remove(username);
  }
}
