mod assets;
mod auth;
mod config;
mod content;
mod db;
mod dto;
mod error;
mod fetch;
mod modules;
mod outbound;
mod scheduler;
mod state;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use uuid::Uuid;

use crate::dto::TraceId;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let args: Vec<String> = std::env::args().collect();
  if args.len() >= 3 && args[1] == "hash-password" {
    let hash = auth::hash_password(&args[2]).map_err(|e| anyhow::anyhow!("{}", e))?;
    println!("{}", hash);
    return Ok(());
  }

  run().await
}

async fn run() -> anyhow::Result<()> {
  let config = config::Config::from_env()?;
  tracing_subscriber::fmt()
    .with_env_filter(tracing_subscriber::EnvFilter::new(&config.log_level))
    .init();

  tracing::info!(
      db_path = %config.db_path,
      backup_dir = %config.backup_dir,
      listen = %config.listen_addr,
      "rss-manager starting"
  );

  let db = db::open(&config.db_path).await?;
  db::init_admin_user(&db, &config).await?;

  let config = Arc::new(config);
  let scheduler = scheduler::Scheduler::new(db.clone(), config.clone());
  scheduler.start();

  let state = state::AppState {
    config: config.clone(),
    db: db.clone(),
    scheduler: scheduler.clone(),
    content_lock: Arc::new(tokio::sync::Mutex::new(())),
  };

  let app = build_router(state);

  let listener = tokio::net::TcpListener::bind(&config.listen_addr).await?;
  tracing::info!("rss-manager listening on {}", config.listen_addr);
  axum::serve(listener, app)
    .with_graceful_shutdown(async {
      let _ = tokio::signal::ctrl_c().await;
    })
    .await?;
  Ok(())
}

fn build_router(state: state::AppState) -> Router {
  let protected = Router::new()
    .route("/feed_list", post(modules::subscriptions::feed_list))
    .route("/feed_create", post(modules::subscriptions::feed_create))
    .route("/feed_get", post(modules::subscriptions::feed_get))
    .route("/feed_update", post(modules::subscriptions::feed_update))
    .route("/feed_delete", post(modules::subscriptions::feed_delete))
    .route("/feed_test", post(modules::subscriptions::feed_test))
    .route("/feed_probe", post(modules::subscriptions::feed_probe))
    .route("/feed_request_config_get", post(modules::subscriptions::feed_request_config_get))
    .route("/fetch_now", post(modules::fetching::fetch_now))
    .route("/fetch_status", post(modules::fetching::fetch_status))
    .route("/fetch_config_get", post(modules::fetching::fetch_config_get))
    .route("/fetch_config_update", post(modules::fetching::fetch_config_update))
    .route("/fetch_log_list", post(modules::fetching::fetch_log_list))
    .route("/health_overview", post(modules::health::health_overview))
    .route("/feed_health", post(modules::health::feed_health_detail))
    .route("/feed_retry", post(modules::health::feed_retry))
    .route("/article_list", post(modules::articles::article_list))
    .route("/article_get", post(modules::articles::article_get))
    .route("/article_mark_read", post(modules::articles::article_mark_read))
    .route("/article_mark_unread", post(modules::articles::article_mark_unread))
    .route("/article_read_all", post(modules::articles::article_read_all))
    .route("/unread_count", post(modules::articles::unread_count))
    .route("/article_fetch_content", post(modules::articles::article_fetch_content))
    .route("/folder_list", post(modules::folders::folder_list))
    .route("/folder_create", post(modules::folders::folder_create))
    .route("/folder_move", post(modules::folders::folder_move))
    .route("/folder_rename", post(modules::folders::folder_rename))
    .route("/folder_delete", post(modules::folders::folder_delete))
    .route("/folder_article_list", post(modules::folders::folder_article_list))
    .route("/article_favorite", post(modules::folders::article_favorite))
    .route("/article_unfavorite", post(modules::folders::article_unfavorite))
    .route("/search_query", post(modules::search::search_query))
    .route("/opml_import", post(modules::opml::opml_import))
    .route("/opml_export", post(modules::opml::opml_export))
    .route("/opml_import_report", post(modules::opml::opml_import_report))
    .route("/auth_logout", post(modules::account::auth_logout))
    .route("/auth_me", post(modules::account::auth_me))
    .route("/auth_change_password", post(modules::account::auth_change_password))
    .layer(middleware::from_fn_with_state(state.clone(), auth::auth_middleware));

  let public = Router::new().route("/auth_login", post(modules::account::auth_login));

  let api = public.merge(protected).with_state(state.clone());

  Router::new()
    .route("/healthz", get(healthz))
    .route("/readyz", get(readyz))
    .nest("/api/rpc", api)
    .fallback(assets::spa_handler)
    .layer(DefaultBodyLimit::max(10 * 1024 * 1024))
    .layer(middleware::from_fn(trace_middleware))
    .with_state(state)
}

async fn healthz() -> impl IntoResponse {
  (StatusCode::OK, "ok")
}

async fn readyz(State(state): State<state::AppState>) -> Response {
  match db::with_conn(&state.db, |c| {
    c.query_row("SELECT 1", [], |r| r.get::<_, i64>(0))?;
    Ok(())
  })
  .await
  {
    Ok(()) => (StatusCode::OK, "ready").into_response(),
    Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "not ready").into_response(),
  }
}

async fn trace_middleware(mut req: Request, next: Next) -> Response {
  let trace_id = Uuid::new_v4().simple().to_string();
  let method = req.method().to_string();
  let path = req.uri().path().to_string();
  req.extensions_mut().insert(TraceId(trace_id.clone()));
  let start = std::time::Instant::now();
  let mut resp = next.run(req).await;
  resp.headers_mut().insert("content-security-policy", axum::http::HeaderValue::from_static("default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' https: http: data:; connect-src 'self'; object-src 'none'; frame-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'"));
  resp
    .headers_mut()
    .insert("x-content-type-options", axum::http::HeaderValue::from_static("nosniff"));
  resp
    .headers_mut()
    .insert("referrer-policy", axum::http::HeaderValue::from_static("no-referrer"));
  tracing::info!(
      trace_id = %trace_id,
      method = %method,
      path = %path,
      status = resp.status().as_u16(),
      elapsed_ms = start.elapsed().as_millis() as u64,
      "request"
  );
  resp
}
