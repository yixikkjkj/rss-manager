#!/usr/bin/env bash
set -euo pipefail

# rss-manager SQLite 在线备份脚本（交付物 D-08）
# 依赖：sqlite3 CLI（apt install sqlite3）
# 配合 cron / systemd timer 使用，例如每日 03:00：
#   0 3 * * * /opt/rss-manager/deploy/backup.sh

DB_PATH="${RSS_MANAGER_DB_PATH:-/data/rss_manager.db}"
BACKUP_DIR="${RSS_MANAGER_BACKUP_DIR:-/data/backups}"

mkdir -p "$BACKUP_DIR"
STAMP="$(date +%Y%m%d_%H%M%S)"
DEST="$BACKUP_DIR/rss_manager_$STAMP.db"

# SQLite `.backup` 在线快照（写时复制，不断服务）
sqlite3 "$DB_PATH" ".backup '$DEST'"

# 保留最近 30 天
find "$BACKUP_DIR" -name 'rss_manager_*.db' -mtime +30 -delete

echo "backup -> $DEST"
