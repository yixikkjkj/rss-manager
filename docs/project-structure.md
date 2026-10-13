# 项目目录整理

根目录表示整个 RSS Manager 项目，前后端是两个独立构建的子目录。

| 原位置 | 当前位置 |
| --- | --- |
| `web/` | `frontend/` |
| `src/` | `backend/src/` |
| `migrations/` | `backend/migrations/` |
| `Cargo.toml`、`Cargo.lock`、`.rustfmt.toml` | `backend/` 下的同名文件 |
| `target/` | `backend/target/` |
| `delivery/` | `docs/design/` |

`deploy/` 管理部署；`docs/` 管理使用、维护和设计说明；`scripts/` 保留跨端构建检查、隔离测试服务和维护脚本。`.git/`、`.github/`、`.workbuddy/` 等工具目录保持在项目根目录。

根目录 `package.json` 仅提供任务入口，没有运行时依赖，也没有新增 workspace。前端 npm 锁文件和后端 Cargo 锁文件分别保留在各自目录。

## 构建与运行

从根目录执行 `npm run setup` 安装前端依赖，`npm run build` 构建前端与后端，`npm run build:release` 构建发行二进制。后端通过相对其 Cargo.toml 的 `../frontend/dist/` 获取前端资源，release 构建将这些资源嵌入二进制。

前端开发使用 `npm run dev:frontend`；后端使用 `npm run dev:backend`。这些根目录命令保持后端进程的工作目录为项目根目录，原 `data/` 及环境变量指定的数据路径仍可使用。若自行进入 `backend/` 启动程序，请用绝对路径设置数据库环境变量，避免把相对数据库路径解析到另一个目录。

CI 与本地都通过 `npm run check:e2e` 或 `node scripts/check.cjs --e2e` 检查。隔离数据库位于 `backend/target/e2e/`，浏览器失败记录位于 `frontend/test-results/`。

Docker 仍以项目根目录为构建上下文：`docker build -f deploy/Dockerfile .`。多阶段构建分别读取 frontend 和 backend；`.dockerignore` 排除本地依赖、编译缓存、测试输出和运行数据。编辑器若仍缓存旧的根 Cargo.toml，重新加载项目，并将 Rust 项目定位到 `backend/Cargo.toml`。

## 验证记录

2026-10-14，在项目根目录执行 `npm run check:e2e`，类型、静态检查、格式检查、生产前端构建及全部 51 项测试通过。`deploy/build.sh` 和项目脚本语法检查通过。

Docker 多阶段构建通过；临时容器使用全新数据库验证健康接口、内嵌首页、JS/CSS 资源、登录与文件夹接口，均通过，测试容器已清理。未修改实际业务数据库，也未执行部署或镜像推送。
