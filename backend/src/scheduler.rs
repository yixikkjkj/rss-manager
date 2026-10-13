//! One bounded queue shared by periodic, manual and OPML-triggered fetches.
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::config::Config;
use crate::db::{with_conn, Db};
use crate::error::{ApiResult, AppError};
use crate::fetch::FetchOutcome;
use crate::outbound::FetchClient;
use tokio::sync::{oneshot, Mutex, Semaphore};

const MAX_CONCURRENCY: usize = 5;
type Waiter = oneshot::Sender<ApiResult<FetchOutcome>>;

#[derive(Default)]
struct Queue {
  waiting: VecDeque<i64>,
  pending: HashSet<i64>,
  waiters: HashMap<i64, Vec<Waiter>>,
}
impl Queue {
  fn enqueue(&mut self, id: i64, waiter: Option<Waiter>) {
    if let Some(waiter) = waiter {
      self.waiters.entry(id).or_default().push(waiter);
    }
    if self.pending.insert(id) {
      self.waiting.push_back(id);
    }
  }
  fn finish(&mut self, id: i64, outcome: ApiResult<FetchOutcome>) {
    self.pending.remove(&id);
    if let Some(waiters) = self.waiters.remove(&id) {
      for waiter in waiters {
        let _ = waiter.send(outcome.clone());
      }
    }
  }
}

pub struct Scheduler {
  db: Db,
  config: Arc<Config>,
  client: FetchClient,
  sem: Arc<Semaphore>,
  queue: Mutex<Queue>,
  active: AtomicI32,
}
impl Scheduler {
  pub fn new(db: Db, config: Arc<Config>) -> Arc<Self> {
    Arc::new(Self {
      db,
      config,
      client: FetchClient::new(),
      sem: Arc::new(Semaphore::new(MAX_CONCURRENCY)),
      queue: Mutex::new(Queue::default()),
      active: AtomicI32::new(0),
    })
  }
  pub fn start(self: &Arc<Self>) {
    let me = Arc::clone(self);
    tokio::spawn(async move {
      let mut ticker = tokio::time::interval(Duration::from_secs(1));
      loop {
        ticker.tick().await;
        if let Err(error) = me.enqueue_due().await {
          tracing::error!(code = error.code, "scan due feeds failed");
        }
        me.drain().await;
      }
    });
  }
  async fn enqueue_due(&self) -> ApiResult<()> {
    // Hold queue state across the DB scan so completion cannot make a stale due
    // snapshot eligible again between SELECT and enqueue.
    let mut queue = self.queue.lock().await;
    let now = crate::dto::now_iso();
    let due = with_conn(&self.db, move |c| {
      let mut stmt =
        c.prepare("SELECT id FROM t_feed WHERE deleted=0 AND health_status!='dead' AND next_fetch_at<=?1")?;
      let rows = stmt
        .query_map([now], |r| r.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
      Ok(rows)
    })
    .await?;
    for id in due {
      queue.enqueue(id, None);
    }
    Ok(())
  }
  async fn drain(self: &Arc<Self>) {
    while let Ok(permit) = self.sem.clone().try_acquire_owned() {
      let id = self.queue.lock().await.waiting.pop_front();
      let Some(id) = id else { break };
      self.active.fetch_add(1, Ordering::SeqCst);
      let me = self.clone();
      tokio::spawn(async move {
        let result = crate::fetch::fetch_single_feed(&me.db, &me.client, id, &me.config).await;
        if let Err(ref e) = result {
          tracing::warn!(feed_id = id, code = e.code, "feed task failed");
        }
        me.queue.lock().await.finish(id, result);
        me.active.fetch_sub(1, Ordering::SeqCst);
        drop(permit);
      });
    }
  }
  pub async fn trigger(&self, feed_id: Option<i64>) -> ApiResult<()> {
    let ids = with_conn(&self.db, move |c| {
      let mut stmt = c.prepare("SELECT id FROM t_feed WHERE deleted=0 AND (?1 IS NULL OR id=?1)")?;
      let ids = stmt
        .query_map([feed_id], |r| r.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
      if feed_id.is_some() && ids.is_empty() {
        return Err(AppError::not_found());
      }
      Ok(ids)
    })
    .await?;
    let mut queue = self.queue.lock().await;
    for id in ids {
      queue.enqueue(id, None);
    }
    Ok(())
  }
  pub async fn fetch_wait(&self, id: i64) -> ApiResult<FetchOutcome> {
    let (tx, rx) = oneshot::channel();
    self.queue.lock().await.enqueue(id, Some(tx));
    rx.await.map_err(|_| AppError::internal())?
  }
  pub fn client(&self) -> FetchClient {
    self.client.clone()
  }
  pub async fn status(&self) -> SchedulerStatus {
    let active = self.active.load(Ordering::SeqCst);
    SchedulerStatus {
      running: active > 0,
      active_fetches: active,
      max_concurrency: MAX_CONCURRENCY as i32,
      queue_len: self.queue.lock().await.waiting.len() as i32,
    }
  }
}

#[derive(Debug, serde::Serialize)]
pub struct SchedulerStatus {
  pub running: bool,
  pub active_fetches: i32,
  pub max_concurrency: i32,
  pub queue_len: i32,
}

#[cfg(test)]
mod tests {
  use super::*;
  #[tokio::test]
  async fn a_running_feed_cannot_be_queued_twice_and_waiters_share_result() {
    let mut q = Queue::default();
    q.enqueue(1, None);
    assert_eq!(q.waiting.pop_front(), Some(1));
    let (tx, rx) = oneshot::channel();
    for _ in 0..30 {
      q.enqueue(1, None);
    }
    q.enqueue(1, Some(tx));
    assert!(q.waiting.is_empty());
    q.enqueue(2, None);
    assert_eq!(q.waiting.len(), 1);
    q.finish(
      1,
      Ok(FetchOutcome {
        result: "success".into(),
        http_status: Some(200),
        new_articles: 1,
        error_message: None,
      }),
    );
    assert_eq!(rx.await.unwrap().unwrap().new_articles, 1);
    q.enqueue(1, None);
    assert_eq!(q.waiting.len(), 2);
  }
}
