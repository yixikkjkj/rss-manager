use std::sync::Arc;

use crate::config::Config;
use crate::db::Db;
use crate::scheduler::Scheduler;

/// 全局应用状态（所有 handler / 中间件共享）
#[derive(Clone)]
pub struct AppState {
  pub config: Arc<Config>,
  pub db: Db,
  pub scheduler: Arc<Scheduler>,
  pub content_lock: Arc<tokio::sync::Mutex<()>>,
}
