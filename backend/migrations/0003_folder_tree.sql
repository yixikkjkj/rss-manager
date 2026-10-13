-- Unified folder hierarchy. Preserve existing folder IDs and article associations.
CREATE TABLE t_folder_v3 (
  id INTEGER PRIMARY KEY,
  user_id INTEGER NOT NULL,
  parent_id INTEGER,
  name TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_time TEXT,
  updated_time TEXT,
  deleted INTEGER NOT NULL DEFAULT 0
);
INSERT INTO t_folder_v3(id,user_id,name,created_time,updated_time,deleted)
SELECT id,user_id,name,created_time,updated_time,deleted FROM t_folder;
DROP TABLE t_folder;
ALTER TABLE t_folder_v3 RENAME TO t_folder;
CREATE UNIQUE INDEX idx_folder_sibling_name ON t_folder(user_id,COALESCE(parent_id,0),name) WHERE deleted=0;
CREATE INDEX idx_folder_parent ON t_folder(user_id,parent_id,deleted);
ALTER TABLE t_feed ADD COLUMN folder_id INTEGER;
CREATE INDEX idx_feed_folder ON t_feed(user_id,folder_id,deleted);
-- Mapping is also used for old bookmarked ?group= URLs. Legacy group data stays intact.
CREATE TABLE t_legacy_group_folder (group_id INTEGER PRIMARY KEY,folder_id INTEGER NOT NULL);
