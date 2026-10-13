use axum::Json;
use chrono::Utc;
use serde::Serialize;

/// ISO 8601 UTC + 毫秒（对齐 §4.1 时间字段约定）
pub fn now_iso() -> String {
  Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// 统一返回结构（§3.5.1）：code/msg/data/traceId/timestamp
#[derive(Debug, Serialize)]
pub struct Resp<T> {
  pub code: String,
  pub msg: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub data: Option<T>,
  #[serde(rename = "traceId")]
  pub trace_id: String,
  pub timestamp: String,
}

impl<T: Serialize> Resp<T> {
  pub fn ok(data: T, trace_id: String) -> Json<Self> {
    Json(Self {
      code: "0".to_string(),
      msg: "ok".to_string(),
      data: Some(data),
      trace_id,
      timestamp: now_iso(),
    })
  }
}

impl<T> Resp<T> {
  pub fn err(code: &str, msg: &str, trace_id: String) -> Self {
    Self {
      code: code.to_string(),
      msg: msg.to_string(),
      data: None,
      trace_id,
      timestamp: now_iso(),
    }
  }
}

/// 分页请求（§3.2.1 Common，通用契约结构，当前各模块用自身分页字段）
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PageRequest {
  pub page: Option<i32>,
  pub page_size: Option<i32>,
}

impl PageRequest {
  /// (offset, limit)
  pub fn bounds(&self) -> (i64, i64) {
    let page = self.page.unwrap_or(1).max(1) as i64;
    let size = self.page_size.unwrap_or(50).clamp(1, 500) as i64;
    ((page - 1) * size, size)
  }
}

/// 请求 traceId（由 trace 中间件注入 request extensions）
#[derive(Debug, Clone)]
pub struct TraceId(pub String);

/// Use the same response envelope for every domain RPC.
pub fn respond<T: Serialize>(result: crate::error::ApiResult<T>, tid: TraceId) -> axum::response::Response {
  use axum::response::IntoResponse;
  match result {
    Ok(v) => Resp::ok(v, tid.0).into_response(),
    Err(e) => e.with_trace(&tid.0).into_response(),
  }
}
