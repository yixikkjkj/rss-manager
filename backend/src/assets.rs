use axum::body::Body;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

/// 编译期内嵌前端构建产物（build.sh 先跑 rspack 产出 frontend/dist）
#[derive(RustEmbed)]
#[folder = "../frontend/dist/"]
pub struct Assets;

/// SPA 静态资源 / History 路由回退：有文件就发文件，否则回退 index.html
pub async fn spa_handler(uri: axum::http::Uri) -> Response {
  let path = uri.path().trim_start_matches('/');
  let path = if path.is_empty() { "index.html" } else { path };

  if let Some(content) = Assets::get(path) {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let mut resp = Response::new(Body::from(content.data.into_owned()));
    resp
      .headers_mut()
      .insert(header::CONTENT_TYPE, mime.as_ref().parse().unwrap());
    if path.starts_with("assets/") {
      resp
        .headers_mut()
        .insert(header::CACHE_CONTROL, "public, max-age=31536000, immutable".parse().unwrap());
    }
    return resp;
  }

  match Assets::get("index.html") {
    Some(content) => {
      let mut resp = Response::new(Body::from(content.data.into_owned()));
      resp
        .headers_mut()
        .insert(header::CONTENT_TYPE, "text/html".parse().unwrap());
      resp
    },
    None => (StatusCode::NOT_FOUND, "frontend not built").into_response(),
  }
}
