# AICoding 架构设计 · 资料摘要

> 本文档做一件事：**精读主理人转交的全部原始资料，逐份、逐章节做出摘要**——后面任何人拿到这份摘要，都能通过章节号快速定位回原始文件的对应位置。

> 上游输入：主理人转交的全部原始资料（OPML / Markdown）；
> 产出者：`knowledge-ingest-engineer`（知识摄入工程师 - 闻资料），经 G1 校验与人工审核通过后交付。

---

## 0. 元信息

```yaml
标题: RSS 管理器（自托管 Web 应用）- 资料摘要 v0.2
版本: v0.2
状态: Draft
创建日期: 2026-08-25
整理人: knowledge-ingest-engineer（闻资料）
审核人:
  - team-lead（主理人）

原始资料清单:
  - C:\Users\yixi\projects\rss-manager\feedbro-subscriptions-20260810-170604.opml: 用户从 Feedbro 导出的 OPML 1.0 订阅清单，4 个分组共 41 个订阅源
  - C:\Users\yixi\projects\rss-manager\introduce.md: 项目介绍文件，当前为空文件
```

| 版本 | 日期 | 作者 | 变更内容 |
| --- | --- | --- | --- |
| v0.1 | 2026-08-25 | knowledge-ingest-engineer | 初稿 |
| v0.2 | 2026-08-25 | knowledge-ingest-engineer | 按 G1 自动校验意见，将 §2 章节定位与出处标注改为「§数字 + 名称」格式，事实内容不变 |

---

## 1. 资料清单

> 列出全部原始资料，每份标注解析状态。解析失败或跳过的必须注明原因。

| 编号 | 文件名 | 类型 | 来源 | 解析状态 | 说明 |
| --- | --- | --- | --- | --- | --- |
| D1 | feedbro-subscriptions-20260810-170604.opml | opml（XML） | 用户（Feedbro 浏览器插件导出） | 已解析 | 全文 99 行完整解析，4 分组 41 源逐条记录 |
| D2 | introduce.md | markdown | 用户（项目目录自带） | 已解析（空文件） | 文件存在但业务内容为 0 行；项目介绍缺失，属事实缺口，待用户后续补充 |

**类型枚举说明**：本项目实际资料类型为 `opml` 与 `markdown`，不属于模板枚举的 docx / pdf / pptx / xlsx，已按纯文本方式直接解析，无需调用专用解析 Skill。

---

## 2. 资料内容摘要

> 逐份文档按自身章节结构做摘要。每条摘要标注章节号（`D编号，§章节`），后面任何人想核实某个点，直接定位回原文对应位置即可。

### D1：feedbro-subscriptions-20260810-170604.opml

> 用户从浏览器插件 Feedbro 导出的 OPML 1.0 订阅清单，含 4 个分组共 41 个 `type="rss"` 订阅源 — 来源：用户（Feedbro 导出）

引用方式说明：本文件为结构化 XML，以下均为**数据提取**，无推断内容。

| 章节 | 内容摘要 |
| --- | --- |
| §1 文件头 | OPML `version="1.0"`，head/title 为 "Feed Subscriptions"；文件名含导出时间戳 20260810-170604（即 2026-08-10 导出）；导出工具为浏览器插件 Feedbro（来源：主理人任务说明） |
| §2 分组结构 | 共 4 个一级分组 outline：Blogs（18 源）、杂志资讯（14 源）、计算机发展资讯（8 源）、已失效（1 源），合计 41 个订阅源，全部 `type="rss"`，无嵌套子分组 |
| §3 Blogs | 18 个个人博客源：李璐慧blog（blog.liluhui.cn/rss2.xml）、杜老师Blog（duyixian.cn/atom.xml）、李兵Blog（hustyichi.github.io//feed）、阿猫的博客（ameow.xyz/feed.xml）、Josh Comeau's blog（joshwcomeau.com/rss.xml）、Ghost_chu's Blog（ghostchu.com/feed/）、从不说安全词（jt26wzz.com/rss.xml）、Nekonull's Garden（nekonull.me/index.xml）、Nekonull's Garden TIL（nekonull.me/til.xml）、Long Luo's Life Notes（longluo.me/rss2.xml）、卡瓦邦噶（kawabangga.com/feed）、Taxodium（taxodium.ink/rss.xml）、mozillazg's Blog（mozillazg.com/feeds/all.atom.xml）、Qiangdom Blog（theodoruszq.wordpress.com/feed/）、⋄ Videre Invisibilia ⋄（caelica.github.io/atom.xml）、yixi blog（wweb.yixijun.cn/feeds/rss.xml）、Matrix67: The Aha Moments（matrix67.com/blog/feed）、老冯云数（vonng.com/index.xml） |
| §4 杂志资讯 | 14 个资讯/社区源：阮一峰的网络日志（feeds.feedburner.com/ruanyifeng）、GitHub All Languages Weekly Trending（mshibanami.github.io/GitHubTrendingRSS/weekly/all.xml）、Hacker News 每日播报（hacker-news.agi.li/rss.xml）、蓝点网（landiannews.com/feed）、36氪（36kr.com/feed）、36氪文章（36kr.com/feed-article）、路由器交流（src.acwifi.net/feed）、SegmentFault 最新的文章（segmentfault.com/feeds/blogs）、Visual Studio Code（code.visualstudio.com/feed.xml）、云原生实验室（icloudnative.io/index.xml）、清华大学 TUNA 协会（tuna.moe/feed.xml）、bing blogs（blogs.bing.com/webmaster/feed）、PReact（preactjs.com/feed.xml）、Zed Industries - Blog（zed.dev/blog.rss） |
| §5 计算机发展资讯 | 8 个技术周刊/官方源：This Week in Rust（this-week-in-rust.org/rss.xml）、Golang Weekly（cprss.s3.amazonaws.com/golangweekly.com.xml）、Node.js Blog（nodejs.org/en/feed/blog.xml）、FSF blogs（static.fsf.org/fsforg/rss/blogs.xml）、Python Insider（feeds.feedburner.com/PythonInsider）、OpenAI Status - Incident History（status.openai.com/history.rss）、Rust.cc（rustcc.cn/rss）、All Mac Rumors Headlines（feeds.macrumors.com/MacRumors-All） |
| §6 已失效 | 1 个源：Linux 中国◆开源社区（linux.cn/rss.xml）。分组名"已失效"为用户在 Feedbro 中自建的归类，表明该源已被用户判定失效，但资料中无失效原因与失效时间 |
| §7 Feed 格式线索 | 从 xmlUrl 路径形态可见多种 Feed 格式惯例（未实际请求验证）：atom.xml 类 2 个（duyixian.cn、caelica.github.io），all.atom.xml 1 个（mozillazg.com），rss2.xml 类 2 个（liluhui.cn、longluo.me），rss.xml 类 6 个（joshwcomeau、jt26wzz、taxodium、wweb.yixijun、hacker-news.agi.li、linux.cn），feed.xml 类 4 个（ameow、code.visualstudio、tuna.moe、preactjs），index.xml 类 3 个（nekonull.me、icloudnative.io、vonng.com），/feed 或 /feed/ 路径类 8 个（ghostchu、kawabangga、wordpress、matrix67、landiannews、36kr、acwifi、blogs.bing），其余为平台特有路径（Feedburner ×2、status.openai.com/history.rss、zed.dev/blog.rss、nodejs blog.xml、fsf blogs.xml、rustcc.cn/rss、macrumors、segmentfault feeds/blogs、36kr feed-article、GitHubTrendingRSS all.xml、S3 托管 golangweekly、nekonull til.xml）。真实 Feed 格式（RSS 2.0 / Atom）需运行时抓取确认 |
| §8 数据质量：缺失 htmlUrl | 4 个源无 htmlUrl 属性：⋄ Videre Invisibilia ⋄（D1，§3 Blogs）、路由器交流（D1，§4 杂志资讯）、Python Insider（D1，§5 计算机发展资讯）、Linux 中国◆开源社区（D1，§6 已失效） |
| §9 数据质量：异常 htmlUrl | 李兵Blog 的 htmlUrl="/"（D1，§3 Blogs），为无效站点地址；其 xmlUrl 为 `https://hustyichi.github.io//feed`，路径含双斜杠"//"，可能存在规范化问题 |
| §10 数据质量：http 明文 | 阮一峰 htmlUrl 为 http（D1，§4 杂志资讯）；36氪与36氪文章 htmlUrl 均为 http://36kr.com（D1，§4 杂志资讯）；FSF blogs htmlUrl 为 http（D1，§5 计算机发展资讯）；Python Insider 的 xmlUrl 本身为 http://feeds.feedburner.com/PythonInsider（D1，§5 计算机发展资讯），是唯一 xmlUrl 使用 http 的源 |
| §11 数据质量：xmlUrl 与 htmlUrl 域名不一致 | 蓝点网：xmlUrl 为 landiannews.com，htmlUrl 为 landian.news（D1，§4 杂志资讯）；Hacker News 每日播报：xmlUrl 为 hacker-news.agi.li，htmlUrl 为 hacker-podcast.agi.li（D1，§4 杂志资讯）；Rust.cc：xmlUrl 为 rustcc.cn/rss，htmlUrl 为 rust.cc（D1，§5 计算机发展资讯）；bing blogs：htmlUrl 路径大小写与 xmlUrl 不一致（webmaster/Feed vs webmaster/feed）（D1，§4 杂志资讯） |
| §12 数据质量：同站多源 | Nekonull's Garden 与 Nekonull's Garden TIL 共享同一 htmlUrl（nekonull.me），xmlUrl 不同（D1，§3 Blogs）；36氪与36氪文章共享同一 htmlUrl（36kr.com），xmlUrl 仅路径不同（feed vs feed-article）（D1，§4 杂志资讯）——导入时需支持同站多源去重/并存策略 |
| §13 数据质量：text 与 title 不一致 | 部分源 text（用户自定义显示名）与 title（Feed 原始标题）不同，如 text="李璐慧blog" 而 title="Luhui's Personal Website"，text="杜老师Blog" 而 title="那时难决"，text="卡瓦邦噶" 而 title="卡瓦邦噶！ » Feed"（D1，§3 Blogs）——导入时需决策以哪个字段作为源名称 |
| §14 缺失信息 | 全文不含任何订阅源的更新频率、未读数、文章存量、抓取历史等运行时数据（纯静态订阅清单）；不含 Feedbro 本身的配置（刷新策略、过滤规则等） |

### D2：introduce.md

> 项目介绍文件，当前为空文件（0 行业务内容） — 来源：用户（项目目录自带）

| 章节 | 内容摘要 |
| --- | --- |
| §1 全文 | （D2，§1）文件存在但无任何业务内容。项目背景、目标用户、核心功能诉求、非功能约束等介绍性信息全部缺失，属事实缺口。当前唯一可用的需求侧信息是主理人转述的用户原始诉求："开发一个 RSS 管理器，从零开始搭建，先探讨是什么/为什么/怎么做"，以及 Phase 0 已确认的产品形态倾向「自托管 Web 应用」（对标 FreshRSS/Miniflux）（来源：主理人任务说明，非 D2 内容） |

---

## 3. 冲突记录

> 不同资料对同一事实描述矛盾时，**并列保留两个版本**，不做裁决。

| 编号 | 冲突主题 | 版本 A | 出处 A | 版本 B | 出处 B | 差异说明 |
| --- | --- | --- | --- | --- | --- | --- |
| — | 无跨资料冲突 | — | — | — | — | 本批资料仅 D1 有实质内容，D2 为空文件，不存在可比对的事实矛盾。D1 内部的数据质量异常（如 text 与 title 不一致、xmlUrl 与 htmlUrl 域名不一致）已在 §2 逐条记录，属于单一来源内的客观现象而非多源冲突 |

---

## 4. 硬指标清单

| 章节 | 硬指标 | 状态 |
| --- | --- | --- |
| §1 | 每份资料有解析状态，失败/跳过注明原因 | ✅（D1 已解析；D2 已解析但为空文件，已注明"项目介绍缺失"事实缺口） |
| §2 | 每份文档按章节逐条摘要，每条标注了 `D编号，§章节` | ✅（D1 按 OPML 分组结构 14 条摘要，章节列采用「§数字 + 名称」连续编号（§1~§14），数据质量项逐条标注 D1+数字章节定位；D2 按全文 1 条摘要并标注 D2，§1） |
| §3 | 冲突信息并列保留，不做裁决 | ✅（无跨资料冲突，已如实说明；未做任何裁决） |

---

## 附录 A：生成流程

### 流程总览

| 步骤 | 动作 | 落入章节 |
| --- | --- | --- |
| Step0 | 读取模板 + 全部原始资料 | — |
| Step1 | 盘点资料清单，标注解析状态 | §1 |
| Step2 | 逐份打开资料，按自身章节结构逐条摘要 | §2 |
| Step3 | 交叉比对不同资料，发现并记录矛盾 | §3 |
| Step4 | 逐项核验硬指标 | §4 |

```mermaid
flowchart LR
    S0[读取模板与资料] --> S1[盘点资料清单]
    S1 --> S2[逐份精读逐章节摘要]
    S2 --> S3[交叉比对记录冲突]
    S3 --> S4[硬指标自检]
```

### 整理原则

1. **逐份精读，不跨文档归并**：摘要按文档自身章节结构组织，不做跨文档的主题重组（那是下游的事）
2. **出处即章节号**：每条摘要标注 `D编号，§章节`，直接映射回原文位置
3. **冲突保留**：矛盾信息并列保留两个版本，不擅自裁决
4. **事实驱动**：以原始资料中的事实为准，不添加主观推断

---

## 附录 B：解析 Skill

- `docx`：Word 类产品/业务文档
- `pdf`：PDF 类规范、手册、报告
- `pptx`：PPT 类方案/汇报
- `xlsx`：Excel 类数据清单、指标表
- 本项目实际资料为 `opml`（XML）与 `markdown` 纯文本，直接读取解析，未调用上述专用 Skill
