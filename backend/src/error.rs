use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

use crate::dto::Resp;

/// 统一应用错误（映射《系统设计》§3.5.1 全局错误码体系）
#[derive(Debug, Clone)]
pub struct AppError {
  pub code: &'static str,
  pub status: StatusCode,
  pub msg: &'static str,
  pub trace_id: String,
}

#[allow(dead_code)] // 错误码注册表为完整契约面，部分码暂未被业务路径触发
impl AppError {
  pub fn invalid(msg: &'static str) -> Self {
    Self::new("C400001", StatusCode::BAD_REQUEST, msg)
  }
  pub fn unsafe_url() -> Self {
    Self::invalid("地址必须是公网 HTTP/HTTPS 地址，端口仅允许 80/443，禁止内网地址及用户凭据")
  }
  pub fn fetch_failed(msg: &'static str) -> Self {
    Self::new("A200002", StatusCode::BAD_GATEWAY, msg)
  }
  pub fn new(code: &'static str, status: StatusCode, msg: &'static str) -> Self {
    Self {
      code,
      status,
      msg,
      trace_id: String::new(),
    }
  }

  pub fn with_trace(mut self, trace_id: &str) -> Self {
    self.trace_id = trace_id.to_string();
    self
  }

  // ---- 全局（C 类，4xx）----
  pub fn bad_request() -> Self {
    Self::new("C400001", StatusCode::BAD_REQUEST, "参数错误")
  }
  pub fn unauthorized() -> Self {
    Self::new("C401001", StatusCode::UNAUTHORIZED, "请重新登录")
  }
  pub fn forbidden() -> Self {
    Self::new("C403001", StatusCode::FORBIDDEN, "无权访问")
  }
  pub fn not_found() -> Self {
    Self::new("C404001", StatusCode::NOT_FOUND, "资源不存在")
  }
  pub fn rate_limited() -> Self {
    Self::new("C429001", StatusCode::TOO_MANY_REQUESTS, "操作太频繁，请稍后再试")
  }

  // ---- M1 订阅管理 ----
  pub fn group_not_found() -> Self {
    Self::new("A100001", StatusCode::OK, "分组不存在")
  }
  pub fn feed_exists() -> Self {
    Self::new("A100002", StatusCode::OK, "该订阅源已存在")
  }
  pub fn group_not_empty() -> Self {
    Self::new("A100003", StatusCode::OK, "请先移出分组内订阅源")
  }

  // ---- M2 抓取调度 ----
  pub fn fetch_conflict() -> Self {
    Self::new("A200001", StatusCode::OK, "抓取正在进行，请稍后再试")
  }

  // ---- M3 源健康 ----
  pub fn feed_dead() -> Self {
    Self::new("A300001", StatusCode::OK, "该源已失效，可重试或修复 URL")
  }

  // ---- M4 阅读中心 ----
  pub fn article_not_found() -> Self {
    Self::new("A400001", StatusCode::OK, "文章不存在")
  }
  pub fn folder_not_found() -> Self {
    Self::new("A400002", StatusCode::OK, "文件夹不存在")
  }
  pub fn folder_exists() -> Self {
    Self::new("A400003", StatusCode::OK, "同级文件夹名已存在")
  }

  // ---- M5 全文搜索 ----
  pub fn search_syntax() -> Self {
    Self::new("A500001", StatusCode::OK, "搜索关键词格式有误")
  }

  // ---- M6 导入导出 ----
  pub fn opml_parse_failed() -> Self {
    Self::new("A600001", StatusCode::OK, "OPML 文件格式无法解析")
  }

  // ---- M7 认证 ----
  pub fn bad_credentials() -> Self {
    Self::new("A700001", StatusCode::UNAUTHORIZED, "用户名或密码错误")
  }
  pub fn session_expired() -> Self {
    Self::new("A700002", StatusCode::UNAUTHORIZED, "登录已过期，请重新登录")
  }

  // ---- 系统错误（B 类，5xx）----
  pub fn internal() -> Self {
    Self::new("B999001", StatusCode::INTERNAL_SERVER_ERROR, "系统繁忙，请稍后再试")
  }
  pub fn db_error() -> Self {
    Self::new("B999002", StatusCode::INTERNAL_SERVER_ERROR, "系统繁忙，请稍后再试")
  }
}

impl IntoResponse for AppError {
  fn into_response(self) -> Response {
    let body = Resp::<()>::err(self.code, self.msg, self.trace_id);
    (self.status, Json(body)).into_response()
  }
}

impl From<rusqlite::Error> for AppError {
  fn from(e: rusqlite::Error) -> Self {
    if let rusqlite::Error::SqliteFailure(ref failure, _) = e {
      if failure.extended_code == 2067 {
        return AppError::invalid("名称或地址已存在，请使用其他值");
      }
    }
    tracing::error!("database error: {:?}", e);
    AppError::db_error()
  }
}

/// 快速构造 AppError 的 Result
pub type ApiResult<T> = Result<T, AppError>;
