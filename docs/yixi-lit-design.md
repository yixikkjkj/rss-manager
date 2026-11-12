# yixi-lit-design 接入评估

当前版本：`yixi-lit-design@0.3.0`，2026-11-11 升级；初次评估版本为 0.1.0。结论：现有接入范围兼容，图标按需 SVG 明显减小构建体积，适合继续渐进使用，暂不做全量控件替换。

## 兼容性与已接入范围

项目和组件库均基于 Lit 3、Light DOM 和全局 CSS，无需 React/Vue 包装层。库要求 `lit ^3.3.3`，当前依赖树满足要求。库以 MIT 协议发布，提供 ESM、类型声明、自定义元素注册和独立组件入口。Context7 未收录该库，本次具体 API 以安装包的 README、类型声明及实现为准。

- 登录：使用 `ld-button` 和 `ld-input`，保留原生 form 提交、回车提交及必填校验；内部 input 透传 `name` 与 `autocomplete`。
- 设置和默认抓取间隔：使用 `ld-button`，异步操作同时绑定 `disabled` 和 `loading`。旧密码和抓取间隔使用 `ld-input`，新密码暂保留原生 input，原因见下文。
- 公共分页：将前后页按钮替换为 `ld-button`，保留原有 URL 状态、页码逻辑和文案。
- 公共错误提示：使用 `ld-alert theme="danger"`，通过 `.title` 更新错误文字，保留内部 `role="alert"`。

各使用模块自行导入 `yixi-lit-design/components/button` 、`components/input` 或 `components/alert`。按需入口自动注册组件并加载依赖样式，不导入 `all` 或全量 `styles.css`。

## 接入约定

1. 表单提交按钮必须显式写 `type="submit"`，库的默认值是 `button`。操作按钮使用 `@ld-click`，表单请求统一由 form 的 `@submit` 处理，避免重复发起。
2. 0.3.0 的 `loading` 已会禁用内部原生按钮；应用保留 `disabled` 和 `loading` 同时绑定，明确业务禁用状态。业务层继续使用 Action 防止重复请求。
3. 库通过移动 Light DOM 子节点模拟插槽，不能把它当成原生 Shadow DOM 插槽。登录按钮的动态文字放在稳定的 `<span>` 内，使外层 Lit 的动态节点保持在同一个父元素里。更复杂的条件内容、列表和反复挂载需要额外验证。
4. `Styles.css` 用 `html:root` 映射应用主题变量，优先于库的 `:root` 默认值；原生按钮样式排除 `.ld-btn`，原生输入框样式排除 `.ld-input__inner`，避免覆盖组件内部样式。输入框的布局间距放在 `ld-input` 宿主上。今后引入其他控件时也需检查全局 input、label、select 等选择器。
5. Rspack 保留字体 `asset/resource` 规则，供未来其他字体资源使用；0.3.0 当前按需入口已不输出字体。开发和生产继续沿用现有 CSS 加载链。
6. Vitest 的 `server.deps.inline` 包含 `yixi-lit-design`，让 Vite 处理其 CSS 导入，避免 Node 直接加载 CSS 报错。
7. 保留用户新增的 pnpm 配置，同时同步 `package-lock.json`；现有 setup 和 CI 仍使用 npm ci。

## 0.2.0 输入框接入与校验边界

新版已透传 `name`、`form`、`required`、`minlength`、`min/max/step`、`pattern`、`autocomplete`、`inputmode`、`enterkeyhint` 和 `list`。本项目使用 `.value` 传值，通过 `@ld-input` 的 `detail.value` 更新业务状态；外层 `<label>` 包含 Light DOM 中的原生 input，保留可访问名称与点击标签聚焦。

登录用户名、登录密码、旧密码、默认抓取间隔已替换为 `ld-input`。密码字段使用 `type="password"`，本轮不引入密码可见性切换控件。已验证 `required`、数值上下限、整数步长、FormData 字段名、回车提交、事件值和 `autocomplete` 属性透传；未自动化验证真实浏览器密码管理器的自动填充流程。

**发现的问题：`minlength` 属性存在，但原生最小长度校验失效。** `LdInput.onInput` 更新组件的 value，随后模板 `.value=${this.value}` 又写回内部 input。浏览器将它视为脚本赋值，清除基于用户编辑的 `validity.tooShort`。真实键盘输入 7 个字符后，新版组件的 12 位校验没有阻止请求。

最小 Chromium 对照实验（`type="password" minlength="12" required`，键盘输入 `Short1!`）：

| 实现 | validity.tooShort | validity.valid |
| --- | --- | --- |
| 原生 input，正常输入 | true | false |
| input 事件里执行 `input.value = input.value` | false | true |

因此“新密码”保持原生 input。建议组件库在内部 value 绑定使用 `live(this.value)`（`lit/directives/live.js`），使当前 DOM 值相同的情况下不再次写入；需要在库中增加真实键盘输入、长度不足被拦截、足够长度可提交的浏览器测试后再恢复迁移。本项目不修改 node_modules，也不以弱化校验来通过测试。

## 暂缓迁移的部分

- **依赖 minlength 的字段**：0.2.0 已补齐相关属性，0.3.0 仍有上述原生校验问题。新密码输入框待库修复后迁移。
- **Dialog / Drawer / Select**：本轮未接入。文件夹树、订阅编辑器已有较复杂交互，替换前应验证焦点管理、键盘操作、动态插槽及窄屏布局。
- **文件夹树和阅读器布局**：属于业务组件，继续保留现有实现；组件库提供基础控件，无需替换业务状态和路由管理。

## 0.1.0 首次接入体积记录

同一工作区、同一依赖安装与生产配置，统计所有懒加载页面的 JS/CSS，不等于首屏请求量。gzip 为各文件单独压缩后加总。

| 指标 | 接入前 | 接入后 |
| --- | ---: | ---: |
| JS + CSS 原始字节 | 135,951 | 289,360 |
| JS + CSS gzip 字节 | 47,942 | 73,169 |
| WOFF2 字体字节 | 0 | 176,812 |
| TTF 回退字体字节 | 0 | 569,976 |

JS/CSS gzip 增加 25,227 字节；字体单独列出，未计入上面的 JS/CSS。按需加载按钮和提示仍会引入完整 Remix Icon CSS 与字体。现代浏览器通常使用 WOFF2，不会同时下载 TTF；TTF 仍是部署产物，并触发 Rspack 单资源大小警告。

0.2.0 已补齐表单属性透传，仍需修复上述长度校验；体积方面建议优化图标按需加载：考虑独立图标入口、SVG 图标或字体子集，避免仅使用按钮也携带整套图标资源。当前保留库官方入口，便于升级和维护。

## 0.1.0 首次接入验证记录

在 Node 24.18.0 下完成：

- `npm run check`：TypeScript、Oxlint、Oxfmt、20 项前端单元测试、3 项 HTML 安全规则测试通过。
- `npm run build`：生产构建成功；完整 TTF 字体触发资源大小警告，见上方实测。
- `npm run test:e2e`：14 条 Chromium 流程全部通过，使用隔离测试数据库。包含新增的组件库登录测试（必填校验、回车提交、加载中禁用、文字恢复、重复错误更新、主题色、图标字体），以及现有设置保存、分页、路由和移动端阅读流程。

本机默认 Node 为 20，验证时通过 PATH 切换至已安装的 Node 24.18.0；当前 Playwright 所需浏览器下载至 `/tmp/rss-manager-playwright`，测试时设置 `PLAYWRIGHT_BROWSERS_PATH` 指向该目录。未更改项目的浏览器路径配置。

## 0.2.0 升级验证结果（2026-10-28）

- npm 依赖范围更新为 `^0.2.0`，npm 和 pnpm 锁文件同步锁定 0.2.0。
- Node 24.18.0 下，类型、静态检查、格式检查、20 项单元测试和 3 项安全规则测试通过。
- 最终生产构建成功，原有 TTF 资源大小警告仍存在。所有页面 JS/CSS 合计 296,958 字节，逐文件 gzip 合计 74,366 字节；字体资源未变化。此数值为当前安装环境的结果，不与上次依赖安装作严格性能对比。
- 最终 15 条 Chromium 流程全部通过，包括登录属性透传和 FormData、新增的数值约束、短密码拦截、有效请求参数及窄屏设置页面。使用已有 Playwright 浏览器缓存和隔离测试数据库。
- 测试中确实复现了组件 minlength 失效，因此最终通过结果包含新密码字段的原生 input 兼容处理，不表示库已修复该问题。

## 0.3.0 升级验证结果（2026-11-11）

**结论：可以直接升级，现有 Button / Alert / Input 用法兼容，体积优化明显。** npm 依赖范围更新为 `^0.3.0`，npm 和 pnpm 锁文件同步锁定 0.3.0，其他依赖版本保持不变。

### 同环境生产构建对照

在 Node 24.18.0 下先构建工作区原有 0.2.0，再只升级组件库到 0.3.0；业务源码、其他 npm 依赖和 Rspack 配置均相同。统计 `frontend/dist` 中所有页面的 `.js`、`.css` 和字体，排除 source map 与 HTML；gzip 使用 Node `gzipSync` 默认设置逐文件压缩后加总。这里是全部构建资源，不代表首屏请求量或加载耗时。

| 指标 | 0.2.0（字节） | 0.3.0（字节） | 变化 |
| --- | ---: | ---: | ---: |
| JavaScript | 135,609 | 139,176 | 增加 2.63% |
| CSS | 161,373 | 37,308 | 减少 76.88% |
| JS + CSS | 296,982 | 176,484 | 减少 40.57% |
| JS + CSS gzip | 74,891 | 56,942 | 减少 23.97% |
| 字体（WOFF2 + TTF） | 746,788 | 0 | 全部移除 |
| JS + CSS + 字体 | 1,043,770 | 176,484 | 减少 83.09% |

优化来自图标由整套 Remix Icon CSS / 字体改成 SVG。系统图标内联在 JS 中，解释了少量 JS 增长；当前按需入口没有引入 `styles/icons.css`，因此不再输出字体，原先 TTF 超过资源大小阈值的构建警告也消失。旧版 WOFF2 为 176,812 字节、TTF 为 569,976 字节，浏览器通常只下载前者，不能把部署资源总减少量当成实际网络节省量。

### 兼容性与验证

- `npm --prefix frontend run build`：成功，无资源大小警告。
- `npm --prefix frontend run check`：类型、Oxlint、Oxfmt、20 项单元测试和 3 项 HTML 安全规则测试全部通过。
- 15 条浏览器流程全部通过，覆盖登录、必填与数值校验、回车提交、加载禁用、重复错误更新、设置、分页、路由及移动端布局。使用本机 Google Chrome 157.0.8103.38、临时 Playwright 配置和隔离测试数据库，项目浏览器配置保持不变。
- 更新登录回归检查：验证 loading 指示器可见、错误提示包含实际 SVG path，并确认该流程没有字体或 Iconify CDN 请求；替换旧的字体可用性断言。
- 单独浏览器诊断确认：0.3.0 中仅设置 Button 的 `loading` 也会禁用原生按钮。
- 单独使用键盘向 `ld-input type="password" minlength="12" required` 输入 `Short1!`，结果仍为 `tooShort=false`、`valid=true`。因此新密码继续保留原生 input；本次升级没有解决这个已知问题。

当前页面使用的系统图标无需额外网络请求。以后使用非系统图标时，库默认向 Iconify CDN 按需请求单个 SVG；需要离线运行时，可按发布包 README 使用 `registerIconLibrary` 配置自托管来源。
