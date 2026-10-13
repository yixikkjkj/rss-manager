# Lit 前端约定

前端从 React 迁移到 Lit，运行时依赖只有 `lit` 和 `dompurify`。Rust RPC、SQLite 和 `frontend/dist` 嵌入方式保持兼容。使用 Node.js 24+，以 `frontend/package-lock.json` 和 `npm ci` 为构建基准。

## 文件命名与目录

源码和样式的基础文件名使用 PascalCase，保留 `.module.scss`、`.test.ts`、`.d.ts` 等语义后缀。每个组件或页面的代码、局部样式和测试放进同名目录，例如：

```text
src/
  Main.ts
  Api.ts
  Types.ts
  Styles.css
  components/
    App/App.ts
    App/App.module.scss
    SafeHtml/SafeHtml.ts
    SafeHtml/SafeHtml.module.scss
    SafeHtml/SafeHtml.test.ts
  pages/
    Reader/Reader.ts
    Reader/Reader.module.scss
  core/Router.ts
  controllers/Resource.ts
```

页面内部独立注册的组件也单独归档，例如 `components/IntervalForm/IntervalForm.ts`。没有局部样式或测试的组件不创建空文件。工具配置仍使用约定名称，例如 `rspack.config.js`、`tsconfig.json`。

## 格式化与静态检查

使用 Oxfmt 0.73.0 和 Oxlint 1.88.0，移除直接依赖的 ESLint、typescript-eslint 和 Prettier，以及旧配置文件。

- `npm run format` / `npm run format:check`：Oxfmt 格式化或检查整个 `frontend` 目录，包括 TS、Lit 模板、SCSS/CSS、HTML、工具配置和浏览器测试。沿用单引号、110 列和尾逗号；忽略构建产物、测试输出和锁文件。
- `npm run lint` / `npm run lint:fix`：Oxlint 检查或自动修复；警告同样导致失败。
- `.oxlintrc.json` 保留禁止显式 any、未使用变量、受限导入等规则。原有 HTML 注入语法约束迁至本地插件 `tools/HtmlSafety/HtmlSafety.mjs`；只有实际的 SafeHtml 模块可导入 unsafe 指令。
- `npm run test:lint` 通过实际调用 Oxlint 检查直接 HTML 赋值、插入、Lit 属性绑定、unsafe 指令导入及允许的正常写法。版本已固定，更新工具时也运行这组测试。
- `npm run check` 与现有 CI 的 `scripts/check.cjs` 使用同一工具链。TypeScript 类型检查仍通过 `tsc --noEmit` 执行。

配置参照 [Oxlint 官方文档](https://oxc.rs/docs/guide/usage/linter/config.html)和 [Oxfmt 迁移文档](https://oxc.rs/docs/guide/usage/formatter/migrate-from-prettier.html)。

## 组件与状态

- 应用内部组件继承 `core/AppElement.ts`，默认渲染到 Light DOM。需要 Shadow DOM 隔离的独立组件可直接继承 `LitElement`，并单独处理影子树内的样式投递。
- 新的块级自定义元素需要显式设置 `display: block`。Light DOM 不使用 `:host`、`::slotted` 或 slot 投影，也不依赖 `static styles` 自动投递样式。
- 文章行、分页器、错误提示、搜索高亮使用模板函数；拥有独立编辑状态或生命周期的页面、表单使用自定义元素。
- 响应式字段使用 `static properties`、TypeScript `declare` 和构造函数初始化。不要用原生实例字段遮蔽 Lit 生成的访问器。对象和数组替换引用后更新。
- 普通私有字段可保存非响应式数据；异步控制器会调用 `requestUpdate()`，手动异步流程需显式请求更新。
- 订阅编辑器和文章正文通过 `keyed()` 按业务对象重建，避免切换对象后保留旧草稿。

## 路由与异步

`core/Router.ts` 负责 History API、同源普通链接跳转和查询参数。页面按需导入。阅读器在根路由的查询参数变化时保留实例，树与列表持续驻留；正文按文章 ID 通过 keyed 重建。其他页面在 URL 变化时创建新实例。筛选、选中文章与页码以 URL 为准。

文章导航使用 `history.state` 中的 `{ ids, returnTo }`，刷新仍可恢复当前历史条目的阅读上下文。直接打开文章链接时进入三栏阅读器，在当前列表数据加载后恢复前后篇导航；不在该页列表中的文章没有相邻导航。仅接管显式绑定 `followLink` 的链接，保留 Ctrl/Meta 点击、新窗口和下载行为。

- `Resource`：显式声明请求依赖，保留 loading/error/refresh，忽略旧请求与断开连接后的结果。
- `Action`：同步阻止重复提交，提供 `current()` 判断；异步操作完成后更新页面、导航或触发其他请求前应检查它。
- `Poll`：等待请求结束再安排下一轮；离开页面停止调度。轮询函数自行处理业务错误，返回 `false` 停止轮询。
- 这些控制器使旧响应失效，不会撤销已发送给后端的操作。
- 详情页单独管理加载序号、自动已读、全文提取和键盘监听；离开时移除监听并使旧结果失效。
- API 收到非登录请求的 401 时清除会话；应用壳负责跳转，HTTP 层不刷新页面，避免登录循环。

## 独立样式

- `Styles.css`：设计变量、reset、表单和共享布局样式。
- `*.module.scss`：组件专属样式，支持 Sass 嵌套；模板使用 `class=${styles.name}`。
- `rspack.config.js` 的 `css-loader` 只对 `.module.css` / `.module.scss` 启用 Modules。类名为 `组件文件名-六位哈希-局部类名`，哈希包含规范化的相对路径和局部类名。
- 构建链为 Sass → css-loader → 开发时 style-loader / 生产时 CssExtractRspackPlugin。生产输出独立、带内容哈希的 CSS，HTML 与异步 chunk 加载器负责引用。
- 不要为同一个文件叠加原生 `css/auto` 和 CSS 提取插件。CSS Modules 防止类名碰撞，不能阻止全局标签规则进入组件。
- 当前 `.module.scss` 类型声明提供字符串映射，不保证每个类名拼写都经过静态校验。
- 正文 HTML 无法附加本地类名；在正文局部根类下使用 `img`、`pre` 等后代选择器。

开发服务器使用整页自动刷新，避免 CustomElementRegistry 重复定义。`npm run dev` 仍监听 3000、代理 `/api` 到 8080。

## 外部 HTML 与测试

`components/SafeHtml/SafeHtml.ts` 是唯一外部 HTML 入口，先 DOMPurify 清洗，再调用 `unsafeHTML`。其他文本（包括搜索高亮）通过普通模板绑定渲染。Shadow DOM 不替代 HTML 清洗。

单元测试使用 Lit 和 Testing Library DOM，清理时移除节点以执行 disconnectedCallback。浏览器测试使用标签、角色和 `data-testid="article-content"`，不依赖编译后的类名；仅样式构建验收测试检查类名格式。

运行完整检查：

```sh
node scripts/check.cjs --e2e
```

测试启动随机隔离数据库，不读写业务数据库。生产发布仍需先构建前端，再重新构建 Rust release 二进制。

## Lit 迁移阶段验证记录

以下为统一文件夹树改造前的迁移阶段记录；后续验证见 [文件夹树说明](folder-tree.md)。

在 Node 24.18.0 下执行 `node scripts/check.cjs --e2e`，类型检查、ESLint、Prettier、生产构建、Rust 格式与 Clippy 全部通过；14 项前端测试、17 项后端测试、10 项 Playwright 流程测试全部通过。另验证 ESLint 会拒绝外部 unsafeHTML 导入、直接 innerHTML 赋值和 Lit 的 .innerHTML 属性绑定。

同一工作区生产构建的产物总量（包含所有懒加载页面，不是首屏请求量）：

| 指标 | React 基线 | Lit |
| --- | ---: | ---: |
| 全部 JS + CSS 原始字节 | 296,373 | 120,112 |
| 每文件 gzip 后加总字节 | 100,086 | 46,688 |

原实现把 CSS 放在 JS 中；新实现单独提取 CSS，上表已经合并计算。原始体积减少约 59.5%，gzip 加总减少约 53.4%。未进行 CPU、内存或真实网络性能基准，不能把体积减少等同于同幅度的运行速度提升。

## PascalCase 与 Oxc 调整验证

2026-10-14，执行 `node scripts/check.cjs --e2e` 全部通过：TypeScript、Oxlint、Oxfmt、前端生产构建、Rust 格式及 Clippy，另有 14 项前端测试、3 项 Oxlint 规则回归、22 项后端测试和 12 条 Chromium 流程。浏览器检查包含移动后的懒加载页面、PascalCase CSS Modules 类名、设置子组件、三栏阅读及窄屏布局。
