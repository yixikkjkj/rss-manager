# rss-manager

单用户、自托管的 RSS 阅读器。后端使用 Rust / Axum，数据保存在 SQLite，Lit 页面随二进制分发，默认使用 Light DOM，Rspack 编译 TypeScript 与独立 SCSS/CSS Modules。

支持嵌套文件夹、三栏阅读、OPML 导入导出、订阅管理、定时抓取与失败退避、阅读状态、全文提取、文章收藏、全文搜索、健康状态和抓取日志。

## 项目目录

```text
rss-manager/
├── frontend/          # Lit 页面、组件、样式、前端测试与 npm 配置
├── backend/           # Rust 服务、迁移、后端测试与 Cargo 配置
│   ├── src/
│   ├── migrations/
│   ├── Cargo.toml
│   └── Cargo.lock
├── deploy/            # Docker、Caddy、systemd 和部署脚本
├── docs/              # 使用与维护文档
│   └── design/        # 原始调研与设计资料（原 delivery）
├── scripts/           # 跨端质量检查、隔离测试服务和维护脚本
├── .github/           # CI
├── .workbuddy/        # 协作工具资料
├── package.json       # 项目级任务入口，无运行时依赖
└── README.md
```

`.git/`、`.codex/` 等工具目录保留在项目根目录（若已存在）。前端产物位于 `frontend/dist/`，后端产物位于 `backend/target/`，均不提交。运行数据仍放在根目录 `data/` 或通过环境变量指定的位置。

在项目根目录执行：

```sh
npm run setup          # 按前端锁文件安装依赖
npm run build          # 先前端，再后端 debug 构建
npm run build:release  # 构建内嵌前端的 release 二进制
npm run check          # 前后端质量检查
npm run check:e2e      # 加上隔离浏览器测试
```

前后端仍可单独进入各自目录使用 npm 和 Cargo。根目录命令通过明确路径调用它们，不依赖 Cargo workspace 或 npm workspaces。目录迁移及开发注意事项见 [项目结构说明](docs/project-structure.md)。

## 本地构建

需要 Rust 1.95+、Node.js 24+ 和 npm。Windows 需安装 Rust 所需的 C++ 编译工具。

```sh
cd frontend
npm ci
npm run build
cd ..
cargo build --manifest-path backend/Cargo.toml --locked
```

必须先构建前端；后端的 rust-embed 依赖 `frontend/dist`。发行构建使用 `cargo build --manifest-path backend/Cargo.toml --locked --release`。

如果登录页持续闪烁，日志反复出现 `POST /api/rpc/auth_me 401` 和 `GET /login 200`，请确认运行的不是旧版二进制。未登录时返回 401 是正常行为；旧版前端会因此反复刷新登录页，当前源码已修复。

Windows 下先在运行程序的终端按 Ctrl+C 停止服务，再从项目根目录依次执行（每步成功后再执行下一步）：

```powershell
cd frontend
npm run build
cd ..
cargo build --manifest-path backend/Cargo.toml --locked --release
.\backend\target\release\rss-manager.exe
```

请在已设置运行环境变量的终端启动程序，然后在浏览器按 Ctrl+F5 刷新。release 程序内嵌前端资源，仅更新 `frontend/dist` 或运行不带 `--release` 的 `cargo build --manifest-path backend/Cargo.toml` 都不会更新 `backend/target/release/rss-manager.exe`。无需删除数据库或重新初始化账号。

## 首次运行

默认账号是 `admin`。先生成密码哈希和随机会话密钥，数据库父目录需要已存在。

Windows PowerShell 示例：

```powershell
New-Item -ItemType Directory -Force data | Out-Null
$env:RSS_MANAGER_DB_PATH = "$PWD/data/rss_manager.db"
$env:RSS_MANAGER_LISTEN_ADDR = "127.0.0.1:8080"
$env:RSS_MANAGER_SESSION_SECRET = node -e "process.stdout.write(require('crypto').randomBytes(32).toString('hex'))"
$env:RSS_MANAGER_INITIAL_PASSWORD_HASH = .\backend\target\debug\rss-manager.exe hash-password '替换为你的初始强密码'
.\backend\target\debug\rss-manager.exe
```

Linux / macOS 示例：

```sh
mkdir -p data
export RSS_MANAGER_DB_PATH="$PWD/data/rss_manager.db"
export RSS_MANAGER_LISTEN_ADDR=127.0.0.1:8080
export RSS_MANAGER_SESSION_SECRET="$(openssl rand -hex 32)"
export RSS_MANAGER_INITIAL_PASSWORD_HASH="$(./backend/target/debug/rss-manager hash-password '替换为你的初始强密码')"
./backend/target/debug/rss-manager
```

访问 http://127.0.0.1:8080。请持久保存随机会话密钥；每次更换密钥都会使已有登录失效。初始密码哈希只在空数据库创建管理员时使用。公开部署时通过 HTTPS 反向代理访问。

`hash-password` 的参数可能进入终端历史，请使用专门生成的初始密码，登录后在设置中修改。

### 管理员密码无法登录

已有账号的密码保存在数据库中，修改初始密码环境变量不会覆盖它。若旧版程序写入了无效哈希，或需要重置管理员密码，先停止服务，再在项目根目录执行（需要 Node.js 24+）：

```powershell
$env:RSS_MANAGER_INITIAL_PASSWORD_HASH = .\backend\target\release\rss-manager.exe hash-password '替换为你要使用的密码'
node scripts/reset-admin-password.cjs .\data\rss_manager.db
.\backend\target\release\rss-manager.exe
```

请将数据库路径替换为启动日志中的实际路径，并确保哈希生成成功后再运行修复脚本。密码参数使用单引号，避免 PowerShell 展开其中的 `$`。不要手工用双引号粘贴 Argon2 哈希，因为哈希本身也包含 `$`。脚本只更新现有 `admin` 的密码并清除其旧会话，保留订阅、文章和收藏；登录时填写生成哈希所用的原始密码。

## 推荐体验路径

1. 登录后在“导入导出”选择 OPML 或粘贴内容，点击“开始导入”，查看自动更新的首抓进度。
2. 在“文章”左侧文件树中新建文件夹、添加订阅。节点的“⋯”菜单支持重命名、移动、删除和订阅设置；源的所属文件夹可在编辑表单中修改。抓取间隔留空表示继承全局设置。
3. 左侧选择文件夹或源，中间显示文章列表，右侧显示正文。文件夹列表合并整个子树中的订阅文章和收藏文章，按文章 ID 去重；支持仅未读和当前范围全部已读。
4. 打开文章后自动已读；j/k 在当前列表页的文章中切换，m 切换阅读状态。返回列表保留筛选和页码。
5. 正文中的“收藏到文件夹”可同时选择多个文件夹。文件夹既能容纳订阅源，也能容纳收藏文章和子文件夹。窄屏通过“文件树 / 文章列表 / 正文”切换面板。
6. 搜索支持分页、源筛选、日期范围及相关度/时间排序。
7. 在“健康与日志”查看错误与最近 30 天抓取记录；“设置”可修改全局间隔和密码。

订阅编辑中的「请求配置（可选）」支持单源 Cookie 和自定义请求头，可先「测试当前配置」再保存；测试显示访问诊断及条目数，不导入文章。使用方式、凭据存储和限制见 [单源请求配置与访问诊断](docs/feed-request-config.md)。

抓取间隔范围为 30 分钟至 7 天。单源覆盖值可以小于全局值。连续三次失败会停止该源的自动抓取，手动重试后恢复调度。

## 开发和检查

开发页面：

```sh
cd frontend
npm run dev
```

也可从根目录执行 `npm run dev:frontend`；另一个终端运行 `npm run dev:backend` 启动后端。开发服务器监听 3000，代理 API 到 8080；首次启动后端前需完成前端构建并设置“首次运行”中的环境变量。自定义元素代码变更使用整页自动刷新，避免重复注册元素；生产样式提取为带内容哈希的独立 CSS。前端组件和样式约定见 [Lit 迁移说明](docs/lit-migration.md)。

完整质量检查（Windows / Linux / macOS 通用）：

```sh
node scripts/check.cjs
node scripts/check.cjs --e2e
```

第一条执行 TypeScript、Oxlint、Oxfmt、前端与规则回归测试、生产构建、Rust 格式、Clippy 和后端测试。第二条额外启动隔离后端并运行 Playwright 浏览器测试。Windows 自动使用已安装的 Edge；其他环境先在 `frontend` 中执行 `npx playwright install chromium`。

浏览器测试只使用自动创建的 `backend/target/e2e/<随机编号>/fixture.db`，不会读写业务数据库，也不会抓取真实订阅。测试后服务器关闭，失败跟踪文件保存在 `frontend/test-results`。

验证已构建的 release 程序时，可在 PowerShell 设置 `$env:RSS_MANAGER_E2E_RELEASE = '1'`，再在 `frontend` 目录运行 `npm run test:e2e`；测试会使用 `backend/target/release/rss-manager.exe`。完成后执行 `Remove-Item Env:RSS_MANAGER_E2E_RELEASE` 恢复默认的 debug 测试模式。

格式化：`cargo fmt --manifest-path backend/Cargo.toml` 和 `cd frontend && npm run format`（Oxfmt）。`npm run lint` 使用 Oxlint，`npm run lint:fix` 应用可自动修复的规则。前端配置分别在 `frontend/.oxfmtrc.json` 与 `frontend/.oxlintrc.json`；CI 通过同一个 `scripts/check.cjs` 使用这套检查。

## 结构与维护约定

- `backend/src/modules/`：按业务命名的 RPC 入口，文章查询与统一文件夹树分别维护。
- `backend/src/outbound.rs`：唯一出站网络入口；地址校验、DNS 解析固定、重定向检查、并发及响应体上限。
- `backend/src/scheduler.rs`：待处理和执行中的源统一去重；定时、手动测试和 OPML 首抓共用队列。
- `backend/src/fetch/`：Feed 入库、条件请求、退避、抓取日志、Readability 正文提取。
- `backend/src/content.rs`：服务端 HTML 清洗和纯文本摘要。
- `backend/src/db.rs`、`backend/migrations/`：SQLite 阻塞任务与版本化事务迁移。
- `frontend/src/Api.ts`、`Types.ts`：前端接口和明确的数据类型。
- `frontend/src/components/<组件名>/`、`frontend/src/pages/<页面名>/`：PascalCase 命名的组件源码、同名 `.module.scss` 和 `.test.ts` 同目录存放；入口为 `frontend/src/Main.ts`，全局样式为 `Styles.css`。
- `frontend/src/controllers/`：统一的数据加载、过期响应忽略、操作状态与轮询清理。
- `frontend/src/core/`：Light DOM 基类和 History 路由；筛选分页保存在 URL，阅读上下文保存在 history.state。
- `frontend/src/components/SafeHtml/SafeHtml.ts`：唯一允许渲染外部 HTML 的入口；先经 DOMPurify 清洗，Oxlint 限制其他位置使用 unsafeHTML 或直接注入 HTML。
- `backend/src/tests.rs`、`frontend/src/**/*.test.*`、`frontend/e2e/`：领域接口、前端回归和浏览器流程测试。

修改接口时同时更新 DTO、前端类型及相关回归测试。数据库查询放在 `with_conn` 内；不要在数据库锁内进行网络请求。禁止绕过 `FetchClient` 创建新抓取路径。新增数据库变更必须增加迁移版本，不依赖修改旧建表脚本升级现有数据库。

## 升级、备份和运行边界

- 启动会在事务内将旧数据库升级至 schema v5；原收藏夹 ID 与收藏关联保留，旧订阅分组迁为根目录文件夹，同名时为迁入的分组添加后缀，不合并原数据。迁移保留订阅、文章和阅读状态，支持从 v1/v2 升级。旧源原有抓取间隔保留为显式覆盖，可在 UI 清空后继承全局。
- 文件夹最多 64 层，同级名称唯一；移动会拒绝自身或后代作为目标。删除包含子文件夹或订阅源的目录前需先移出它们；删除空目录仅移除其收藏关联，文章保留。
- OPML 导入导出保留文件夹嵌套和空文件夹，文章内容、收藏关联和阅读状态仍需通过数据库备份保存。
- 升级前使用 SQLite 在线备份或停止服务后备份数据库。升级后不要用旧二进制直接运行已迁移的数据库。
- `deploy/backup.sh` 使用 SQLite 的 `.backup`，保留 30 天。运行时有 WAL 写入，不要直接复制正在使用的单个主数据库文件。
- SSRF 防护仅允许公网 HTTP/HTTPS、80/443 端口，拒绝内网、回环、链路本地和保留地址，并忽略进程的代理环境变量。当前版本不支持私有 RSSHub、非标准端口或代理抓取。
- 每个 HTTP 请求及其重定向链最长 30 秒、解压后的响应体最多 5 MB；统一网络并发上限 5。全文提取失败会保留 Feed 正文并提供原文链接。
- 中文关键词使用字面子串匹配，拉丁文字使用 FTS5；大规模中文搜索的性能还需单独评估。
- 导入报告在内存保留最近 100 次，重启后清空；订阅和文章持久化。历史文章在删除订阅后仍可通过搜索或收藏访问。
- 原有 `deploy/Caddyfile` 默认是二进制部署的回环地址。使用 Docker Compose 时，需按文件注释把 upstream 改为 `rss-manager:8080`，并替换示例域名。真实公网订阅需按实际部署环境验收。

`docs/design/` 保留最初调研和设计；本轮实现范围、验证与后续事项见 [修复记录](docs/repair-notes.md)和[统一文件夹树说明](docs/folder-tree.md)。
