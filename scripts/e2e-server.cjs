// Isolated fixture server for Playwright. Never points at an existing database.
const fs = require("node:fs");
const path = require("node:path");
const { spawn, execFileSync } = require("node:child_process");
const { DatabaseSync } = require("node:sqlite");
const { randomUUID } = require("node:crypto");

const root = path.resolve(__dirname, "..");
const backend = path.join(root, "backend");
const runDir = path.join(backend, "target", "e2e", randomUUID());
fs.mkdirSync(runDir, { recursive: true });
const binary = path.join(
  backend,
  "target",
  process.env.RSS_MANAGER_E2E_RELEASE === "1" ? "release" : "debug",
  process.platform === "win32" ? "rss-manager.exe" : "rss-manager",
);
const password = "E2e-Test-Password-123!";
const hash = execFileSync(binary, ["hash-password", password], {
  encoding: "utf8",
  windowsHide: true,
}).trim();
const dbPath = path.join(runDir, "fixture.db");
const db = new DatabaseSync(dbPath);
db.exec(fs.readFileSync(path.join(backend, "migrations/0001_init.sql"), "utf8"));
db.exec(
  fs.readFileSync(
    path.join(backend, "migrations/0002_search_and_settings.sql"),
    "utf8",
  ),
);
db.exec("PRAGMA user_version=2");
db.prepare("INSERT INTO t_user(id,username,password_hash) VALUES(1,?,?)").run(
  "admin",
  hash,
);
db.prepare("INSERT INTO t_feed_group(id,user_id,name) VALUES(1,1,?)").run(
  "Fixture group",
);
db.prepare(
  "INSERT INTO t_feed(id,user_id,group_id,title,xml_url,use_readability,next_fetch_at) VALUES(1,1,1,?,?,1,?)",
).run(
  "Fixture source",
  "https://fixture.invalid/feed",
  "2099-01-01T00:00:00.000Z",
);
const insert = db.prepare(
  "INSERT INTO t_article(id,feed_id,guid,title,content,link,published_at,created_time) VALUES(?,1,?,?,?,?,?,?)",
);
for (let id = 1; id <= 26; id++) {
  insert.run(
    id,
    "guid-" + id,
    "Searchneedle article " + id,
    '<p>Readable body needle.</p><img src="x" onerror="window.__rssInjected=true"><script>window.__rssInjected=true</script>',
    "http://127.0.0.1/private",
    "2026-09-" + String(id).padStart(2, "0") + "T12:00:00.000Z",
    "2026-09-01T00:00:00.000Z",
  );
}
db.close();
const child = spawn(binary, [], {
  cwd: root,
  windowsHide: true,
  stdio: "inherit",
  env: {
    ...process.env,
    RSS_MANAGER_DB_PATH: dbPath,
    RSS_MANAGER_LISTEN_ADDR: "127.0.0.1:39183",
    RSS_MANAGER_SESSION_SECRET: "e2e-only-secret-00000000000000000000000000",
    RSS_MANAGER_LOG_LEVEL: "warn",
  },
});
child.on("error", (error) => {
  console.error(error);
  process.exitCode = 1;
});
child.on("exit", (code) => process.exit(code ?? 0));
for (const signal of ["SIGINT", "SIGTERM"])
  process.on(signal, () => child.kill());
