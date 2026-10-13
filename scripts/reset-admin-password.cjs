// Run locally after setting RSS_MANAGER_INITIAL_PASSWORD_HASH with hash-password.
const fs = require("node:fs");
const path = require("node:path");
const { DatabaseSync } = require("node:sqlite");

function main() {
  const hash = process.env.RSS_MANAGER_INITIAL_PASSWORD_HASH;
  if (
    !hash ||
    !/^\$argon2id\$v=19\$m=\d+,t=\d+,p=\d+\$[A-Za-z0-9+/]+\$[A-Za-z0-9+/]+$/.test(hash)
  ) {
    throw new Error(
      "RSS_MANAGER_INITIAL_PASSWORD_HASH 无效。请先用 rss-manager.exe hash-password 生成哈希，并将命令输出直接赋给环境变量。",
    );
  }
  const dbPath = path.resolve(
    process.argv[2] ||
      process.env.RSS_MANAGER_DB_PATH ||
      path.join(__dirname, "../data/rss_manager.db"),
  );
  if (!fs.existsSync(dbPath)) throw new Error(`数据库不存在：${dbPath}`);
  const db = new DatabaseSync(dbPath);
  try {
    db.exec("PRAGMA busy_timeout=5000; BEGIN IMMEDIATE");
    const user = db
      .prepare("SELECT id FROM t_user WHERE username = ? AND deleted = 0")
      .get("admin");
    if (!user) throw new Error("数据库中没有有效的 admin 账号，未作修改。");
    db.prepare(
      "UPDATE t_user SET password_hash = ?, updated_time = ? WHERE id = ?",
    ).run(hash, new Date().toISOString(), user.id);
    db.prepare("DELETE FROM t_session WHERE user_id = ?").run(user.id);
    db.exec("COMMIT");
    console.log(`已更新 admin 密码并清除其旧会话：${dbPath}`);
    console.log("请启动程序，使用 admin 和生成哈希时的原始密码登录。");
  } catch (error) {
    if (db.isTransaction) db.exec("ROLLBACK");
    throw error;
  } finally {
    db.close();
  }
}

try {
  main();
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
