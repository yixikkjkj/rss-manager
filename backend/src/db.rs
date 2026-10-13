use rusqlite::Connection;
use std::sync::{Arc, Mutex};

use crate::config::Config;
use crate::error::AppError;

pub type Db = Arc<Mutex<Connection>>;

pub async fn open(path: &str) -> anyhow::Result<Db> {
  let path = path.to_string();
  tokio::task::spawn_blocking(move || {
    let mut conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "busy_timeout", 5000)?;
    migrate(&mut conn)?;
    Ok(Arc::new(Mutex::new(conn)))
  })
  .await?
}

fn migrate(conn: &mut Connection) -> anyhow::Result<()> {
  let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
  anyhow::ensure!(version <= 3, "数据库版本高于当前程序，请勿降级运行");
  let tx = conn.transaction()?;
  if version < 1 {
    tx.execute_batch(include_str!("../migrations/0001_init.sql"))?;
  }
  if version < 2 {
    tx.execute_batch(include_str!("../migrations/0002_search_and_settings.sql"))?;
  }
  if version < 3 {
    tx.execute_batch(include_str!("../migrations/0003_folder_tree.sql"))?;
    let groups = {
      let mut stmt = tx.prepare("SELECT id,user_id,name,sort_order FROM t_feed_group WHERE deleted=0 ORDER BY id")?;
      let rows = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?)))?
        .collect::<Result<Vec<_>, _>>()?;
      rows
    };
    for (group_id, user_id, name, sort_order) in groups {
      let mut candidate = name.clone();
      let mut suffix = 0;
      while tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM t_folder WHERE user_id=?1 AND parent_id IS NULL AND name=?2 AND deleted=0)",
        rusqlite::params![user_id, candidate],
        |r| r.get::<_, bool>(0),
      )? {
        suffix += 1;
        let ending = format!("（订阅分组 {group_id}-{suffix}）");
        let prefix: String = name.chars().take(200 - ending.chars().count()).collect();
        candidate = format!("{prefix}{ending}");
      }
      tx.execute(
        "INSERT INTO t_folder(user_id,name,sort_order,created_time,updated_time) VALUES(?1,?2,?3,?4,?4)",
        rusqlite::params![user_id, candidate, sort_order, crate::dto::now_iso()],
      )?;
      let folder_id = tx.last_insert_rowid();
      tx.execute(
        "INSERT INTO t_legacy_group_folder(group_id,folder_id) VALUES(?1,?2)",
        rusqlite::params![group_id, folder_id],
      )?;
      tx.execute(
        "UPDATE t_feed SET folder_id=?1 WHERE group_id=?2 AND user_id=?3",
        rusqlite::params![folder_id, group_id, user_id],
      )?;
    }
  }
  tx.pragma_update(None, "user_version", 3)?;
  tx.commit()?;
  Ok(())
}

/// Keep SQLite's blocking work off Tokio's async executor. The single connection
/// serializes short transactions; no network work belongs inside this closure.
pub async fn with_conn<F, T>(db: &Db, f: F) -> Result<T, AppError>
where
  F: FnOnce(&mut Connection) -> Result<T, AppError> + Send + 'static,
  T: Send + 'static,
{
  let db = Arc::clone(db);
  tokio::task::spawn_blocking(move || {
    let mut conn = db.lock().map_err(|_| AppError::internal())?;
    f(&mut conn)
  })
  .await
  .map_err(|_| AppError::internal())?
}

pub async fn init_admin_user(db: &Db, config: &Config) -> anyhow::Result<()> {
  let hash = config.initial_password_hash.clone();
  with_conn(db, move |conn| {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM t_user WHERE deleted = 0", [], |r| r.get(0))?;
    if count == 0 {
      let hash = hash.ok_or_else(|| AppError::invalid("首次启动需设置 RSS_MANAGER_INITIAL_PASSWORD_HASH"))?;
      argon2::PasswordHash::new(&hash).map_err(|_| AppError::invalid("初始密码哈希无效"))?;
      let now = crate::dto::now_iso();
      conn.execute(
        "INSERT INTO t_user (username,password_hash,created_time,updated_time,deleted) VALUES ('admin',?1,?2,?2,0)",
        rusqlite::params![hash, now],
      )?;
    }
    Ok(())
  })
  .await
  .map_err(|e| anyhow::anyhow!("{}", e.msg))
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn repairs_legacy_index_and_tracks_insert_update_delete() {
    let mut c = Connection::open_in_memory().unwrap();
    c.execute_batch(include_str!("../migrations/0001_init.sql")).unwrap();
    c.execute("INSERT INTO t_article(feed_id,guid,title,content) VALUES(1,'old','legacyneedle','body')", [])
      .unwrap();
    migrate(&mut c).unwrap();
    let count = |c: &Connection, q: &str| {
      c.query_row("SELECT count(*) FROM t_article_fts WHERE t_article_fts MATCH ?1", [q], |r| r.get::<_, i64>(0))
        .unwrap()
    };
    assert_eq!(count(&c, "legacyneedle"), 1);
    c.execute("UPDATE t_article SET title='updatedneedle', full_content='fullneedle' WHERE id=1", [])
      .unwrap();
    assert_eq!(count(&c, "legacyneedle"), 0);
    assert_eq!(count(&c, "updatedneedle"), 1);
    assert_eq!(count(&c, "fullneedle"), 1);
    c.execute("INSERT INTO t_article(feed_id,guid,title) VALUES(1,'new','insertneedle')", [])
      .unwrap();
    assert_eq!(count(&c, "insertneedle"), 1);
    c.execute("DELETE FROM t_article WHERE id=1", []).unwrap();
    assert_eq!(count(&c, "updatedneedle"), 0);
    migrate(&mut c).unwrap();
    assert_eq!(count(&c, "insertneedle"), 1);
  }
}

#[cfg(test)]
mod folder_migration_tests {
  use super::*;
  #[test]
  fn migration_preserves_favorites_group_feeds_and_conflicting_names_and_is_idempotent() {
    let mut c = Connection::open_in_memory().unwrap();
    c.execute_batch(include_str!("../migrations/0001_init.sql")).unwrap();
    c.execute_batch(include_str!("../migrations/0002_search_and_settings.sql"))
      .unwrap();
    c.execute_batch(
      "PRAGMA user_version=2;
      INSERT INTO t_folder(id,user_id,name) VALUES(7,1,'Same');
      INSERT INTO t_feed_group(id,user_id,name) VALUES(3,1,'Same');
      INSERT INTO t_feed(id,user_id,group_id,title,xml_url) VALUES(9,1,3,'Feed','https://fixture.invalid');
      INSERT INTO t_article(id,feed_id,guid,title) VALUES(11,9,'guid','Article');
      INSERT INTO t_article_folder(user_id,article_id,folder_id) VALUES(1,11,7);
      INSERT INTO t_article_read(user_id,article_id) VALUES(1,11);",
    )
    .unwrap();
    migrate(&mut c).unwrap();
    let folder: i64 = c
      .query_row("SELECT folder_id FROM t_feed WHERE id=9", [], |r| r.get(0))
      .unwrap();
    assert_ne!(folder, 7);
    assert_eq!(
      c.query_row("SELECT name FROM t_folder WHERE id=7", [], |r| r.get::<_, String>(0))
        .unwrap(),
      "Same"
    );
    assert!(c
      .query_row("SELECT name FROM t_folder WHERE id=?1", [folder], |r| r.get::<_, String>(0))
      .unwrap()
      .starts_with("Same（订阅分组"));
    assert_eq!(
      c.query_row("SELECT folder_id FROM t_article_folder WHERE article_id=11", [], |r| r.get::<_, i64>(0))
        .unwrap(),
      7
    );
    assert_eq!(
      c.query_row("SELECT folder_id FROM t_legacy_group_folder WHERE group_id=3", [], |r| r.get::<_, i64>(0))
        .unwrap(),
      folder
    );
    migrate(&mut c).unwrap();
    assert_eq!(
      c.query_row("SELECT count(*) FROM t_folder", [], |r| r.get::<_, i64>(0))
        .unwrap(),
      2
    );
    assert_eq!(
      c.query_row("SELECT count(*) FROM t_article_read", [], |r| r.get::<_, i64>(0))
        .unwrap(),
      1
    );
    assert_eq!(
      c.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
        .unwrap(),
      3
    );
  }
}
