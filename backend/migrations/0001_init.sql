-- rss-manager 初始 schema（对齐《系统设计》§4.2 单表设计）
-- SQLite 语法规范化：MySQL 的 `KEY idx (col)` 索引声明改写为 `CREATE INDEX`；
-- FTS5 external-content 模式同步触发器仅写 rowid（外部内容由 content='t_article' 实时读取）。
-- 全部幂等（IF NOT EXISTS），可安全重复执行。

-- ============ M7 认证与会话 ============
CREATE TABLE IF NOT EXISTS t_user (
  id            INTEGER PRIMARY KEY,
  username      TEXT NOT NULL,
  password_hash TEXT NOT NULL,
  created_time  TEXT,
  updated_time  TEXT,
  deleted       INTEGER NOT NULL DEFAULT 0,
  UNIQUE (username)
);

CREATE TABLE IF NOT EXISTS t_session (
  id          INTEGER PRIMARY KEY,
  user_id     INTEGER NOT NULL,
  token_hash  TEXT NOT NULL,
  expires_at  TEXT NOT NULL,
  created_time TEXT,
  UNIQUE (token_hash)
);
CREATE INDEX IF NOT EXISTS idx_session_user ON t_session (user_id);

-- ============ M1 订阅管理 ============
CREATE TABLE IF NOT EXISTS t_feed_group (
  id           INTEGER PRIMARY KEY,
  user_id      INTEGER NOT NULL,
  name         TEXT NOT NULL,
  sort_order   INTEGER NOT NULL DEFAULT 0,
  created_time TEXT,
  updated_time TEXT,
  deleted      INTEGER NOT NULL DEFAULT 0,
  UNIQUE (user_id, name)
);

CREATE TABLE IF NOT EXISTS t_feed (
  id                    INTEGER PRIMARY KEY,
  user_id               INTEGER NOT NULL,
  group_id              INTEGER,
  title                 TEXT NOT NULL,
  feed_title            TEXT,
  xml_url               TEXT NOT NULL,
  html_url              TEXT,
  fetch_interval_sec    INTEGER NOT NULL DEFAULT 3600,
  use_readability       INTEGER NOT NULL DEFAULT 0,
  health_status         TEXT NOT NULL DEFAULT 'healthy',
  consecutive_failures  INTEGER NOT NULL DEFAULT 0,
  etag                  TEXT,
  last_modified         TEXT,
  last_fetch_at         TEXT,
  last_success_at       TEXT,
  next_fetch_at         TEXT,
  created_time          TEXT,
  updated_time          TEXT,
  deleted               INTEGER NOT NULL DEFAULT 0,
  UNIQUE (user_id, xml_url)
);
CREATE INDEX IF NOT EXISTS idx_feed_user_group ON t_feed (user_id, group_id);
CREATE INDEX IF NOT EXISTS idx_feed_next_fetch ON t_feed (next_fetch_at, health_status);

-- ============ M4 阅读中心 ============
CREATE TABLE IF NOT EXISTS t_article (
  id            INTEGER PRIMARY KEY,
  feed_id       INTEGER NOT NULL,
  guid          TEXT NOT NULL,
  title         TEXT NOT NULL,
  author        TEXT,
  link          TEXT,
  published_at  TEXT,
  content       TEXT,
  full_content  TEXT,
  content_hash  TEXT,
  created_time  TEXT,
  updated_time  TEXT,
  deleted       INTEGER NOT NULL DEFAULT 0,
  UNIQUE (feed_id, guid)
);
CREATE INDEX IF NOT EXISTS idx_article_feed_published ON t_article (feed_id, published_at DESC);
CREATE INDEX IF NOT EXISTS idx_article_published ON t_article (published_at);

CREATE TABLE IF NOT EXISTS t_article_read (
  id           INTEGER PRIMARY KEY,
  user_id      INTEGER NOT NULL,
  article_id   INTEGER NOT NULL,
  read_at      TEXT,
  created_time TEXT,
  UNIQUE (user_id, article_id)
);
CREATE INDEX IF NOT EXISTS idx_article_read_article ON t_article_read (article_id);

CREATE TABLE IF NOT EXISTS t_folder (
  id           INTEGER PRIMARY KEY,
  user_id      INTEGER NOT NULL,
  name         TEXT NOT NULL,
  created_time TEXT,
  updated_time TEXT,
  deleted      INTEGER NOT NULL DEFAULT 0,
  UNIQUE (user_id, name)
);

CREATE TABLE IF NOT EXISTS t_article_folder (
  id           INTEGER PRIMARY KEY,
  user_id      INTEGER NOT NULL,
  article_id   INTEGER NOT NULL,
  folder_id    INTEGER NOT NULL,
  created_time TEXT,
  UNIQUE (user_id, article_id, folder_id)
);
CREATE INDEX IF NOT EXISTS idx_article_folder_folder ON t_article_folder (folder_id, created_time DESC);
CREATE INDEX IF NOT EXISTS idx_article_folder_article ON t_article_folder (article_id);

-- ============ M2 抓取调度 ============
CREATE TABLE IF NOT EXISTS t_fetch_log (
  id                INTEGER PRIMARY KEY,
  feed_id           INTEGER NOT NULL,
  fetch_at          TEXT,
  http_status       INTEGER,
  result            TEXT NOT NULL,
  error_message     TEXT,
  new_article_count INTEGER NOT NULL DEFAULT 0,
  duration_ms       INTEGER,
  created_time      TEXT
);
CREATE INDEX IF NOT EXISTS idx_fetch_log_feed ON t_fetch_log (feed_id, fetch_at DESC);
CREATE INDEX IF NOT EXISTS idx_fetch_log_at ON t_fetch_log (fetch_at);

-- ============ M5 全文搜索（FTS5 external-content）============
CREATE VIRTUAL TABLE IF NOT EXISTS t_article_fts USING fts5(
  title, content, full_content,
  content='t_article', content_rowid='id'
);

CREATE TRIGGER IF NOT EXISTS trg_article_ai AFTER INSERT ON t_article BEGIN
  INSERT INTO t_article_fts(rowid) VALUES (new.id);
END;
CREATE TRIGGER IF NOT EXISTS trg_article_ad AFTER DELETE ON t_article BEGIN
  INSERT INTO t_article_fts(t_article_fts, rowid) VALUES ('delete', old.id);
END;
CREATE TRIGGER IF NOT EXISTS trg_article_au AFTER UPDATE ON t_article BEGIN
  INSERT INTO t_article_fts(t_article_fts, rowid) VALUES ('delete', old.id);
  INSERT INTO t_article_fts(rowid) VALUES (new.id);
END;
