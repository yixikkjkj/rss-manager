use std::env;

/// 运行时配置（对齐《部署设计》§4.4 环境变量清单）
/// L3 配置有默认值；L4 Secret 无默认，启动 fail-fast。
#[derive(Debug, Clone)]
pub struct Config {
  pub listen_addr: String,
  pub db_path: String,
  pub backup_dir: String,
  pub log_level: String,
  pub fetch_interval: i64,
  pub session_secret: String,
  pub initial_password_hash: Option<String>,
}

impl Config {
  pub fn from_env() -> anyhow::Result<Self> {
    let listen_addr = env::var("RSS_MANAGER_LISTEN_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let db_path = env::var("RSS_MANAGER_DB_PATH").unwrap_or_else(|_| "/data/rss_manager.db".into());
    let backup_dir = env::var("RSS_MANAGER_BACKUP_DIR").unwrap_or_else(|_| "/data/backups".into());
    let log_level = env::var("RSS_MANAGER_LOG_LEVEL").unwrap_or_else(|_| "INFO".into());
    let fetch_interval = env::var("RSS_MANAGER_FETCH_INTERVAL")
      .unwrap_or_else(|_| "3600".into())
      .parse::<i64>()?;
    anyhow::ensure!((1800..=604800).contains(&fetch_interval), "RSS_MANAGER_FETCH_INTERVAL 须为 1800–604800 秒");

    // L4：会话签名密钥，必须显式提供（256bit 随机，禁止跑默认值）
    let session_secret = env::var("RSS_MANAGER_SESSION_SECRET").map_err(|_| {
      anyhow::anyhow!(
        "缺少环境变量 RSS_MANAGER_SESSION_SECRET（L4 Secret，必须显式提供）。\
                 生成方式：`openssl rand -hex 32`"
      )
    })?;
    if session_secret.len() < 32 {
      anyhow::bail!("RSS_MANAGER_SESSION_SECRET 长度不足（需 ≥32 字节的随机密钥）");
    }

    // L4：初始密码哈希，仅在首次初始化（t_user 为空）时必需；此处不强校验，由 db 初始化时判空。
    let initial_password_hash = env::var("RSS_MANAGER_INITIAL_PASSWORD_HASH").ok();

    Ok(Self {
      listen_addr,
      db_path,
      backup_dir,
      log_level,
      fetch_interval,
      session_secret,
      initial_password_hash,
    })
  }
}
