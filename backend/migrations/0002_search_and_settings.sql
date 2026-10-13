-- Repair indexes created by the original rowid-only triggers.
DROP TRIGGER IF EXISTS trg_article_ai;
DROP TRIGGER IF EXISTS trg_article_ad;
DROP TRIGGER IF EXISTS trg_article_au;
CREATE TRIGGER trg_article_ai AFTER INSERT ON t_article BEGIN
  INSERT INTO t_article_fts(rowid, title, content, full_content)
  VALUES (new.id, new.title, new.content, new.full_content);
END;
CREATE TRIGGER trg_article_ad AFTER DELETE ON t_article BEGIN
  INSERT INTO t_article_fts(t_article_fts, rowid, title, content, full_content)
  VALUES ('delete', old.id, old.title, old.content, old.full_content);
END;
CREATE TRIGGER trg_article_au AFTER UPDATE ON t_article BEGIN
  INSERT INTO t_article_fts(t_article_fts, rowid, title, content, full_content)
  VALUES ('delete', old.id, old.title, old.content, old.full_content);
  INSERT INTO t_article_fts(rowid, title, content, full_content)
  VALUES (new.id, new.title, new.content, new.full_content);
END;
INSERT INTO t_article_fts(t_article_fts) VALUES ('rebuild');

CREATE TABLE t_setting (key TEXT PRIMARY KEY, value INTEGER NOT NULL);
-- NULL inherits the global interval; legacy values remain explicit overrides.
ALTER TABLE t_feed ADD COLUMN interval_override_sec INTEGER;
UPDATE t_feed SET interval_override_sec = fetch_interval_sec;
ALTER TABLE t_article ADD COLUMN full_content_status TEXT NOT NULL DEFAULT 'not_attempted';
UPDATE t_article SET full_content_status = 'success' WHERE full_content IS NOT NULL;

