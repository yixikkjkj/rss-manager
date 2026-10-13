import { html, nothing } from 'lit';
import { AppElement, valueOf } from '../../core/AppElement';
import { Action } from '../../controllers/Action';
import { Poll } from '../../controllers/Poll';
import { messageOf } from '../../controllers/Resource';
import { errorMessage } from '../../components/Feedback/Feedback';
import { api } from '../../Api';
import type { ImportReportVO, OpmlImportResultVO } from '../../Types';

export class ImportExport extends AppElement {
  static properties = { content: { state: true } };
  declare private content: string;
  private result?: OpmlImportResultVO;
  private report?: ImportReportVO;
  private pollError = '';
  private revision = 0;
  private action = new Action(this);
  private polling = new Poll(this, 1500, () => this.poll());
  constructor() {
    super();
    this.content = '';
  }
  disconnectedCallback() {
    super.disconnectedCallback();
    this.revision++;
  }
  private poll = async () => {
    const id = this.result?.import_id;
    if (!id) return false;
    const revision = this.revision;
    try {
      const report = await api.opmlImportReport(id);
      if (!this.isConnected || revision !== this.revision) return false;
      this.report = report;
      this.pollError = '';
      this.requestUpdate();
      return report.completed < report.total_first_fetch;
    } catch (error) {
      if (this.isConnected && revision === this.revision) {
        this.pollError = messageOf(error);
        this.requestUpdate();
      }
      return false;
    }
  };
  private restartPoll = () => {
    this.revision++;
    this.polling.start();
  };
  private submit = (event: Event) => {
    event.preventDefault();
    void this.action.run(async (current) => {
      const result = await api.opmlImport(this.content);
      if (!current()) return;
      this.result = result;
      this.report = undefined;
      this.pollError = '';
      this.restartPoll();
    });
  };
  private chooseFile = (event: Event) => {
    const file = (event.currentTarget as HTMLInputElement).files?.[0];
    if (file)
      void this.action.run(async (current) => {
        if (file.size > 8 * 1024 * 1024) throw new Error('请选择不超过 8 MB 的 OPML 文件');
        const content = await file.text();
        if (current()) this.content = content;
      });
  };
  private exportContent = () =>
    void this.action.run(async (current) => {
      const value = await api.opmlExport();
      if (!current()) return;
      const url = URL.createObjectURL(new Blob([value.content], { type: 'text/x-opml' }));
      const link = document.createElement('a');
      link.href = url;
      link.download = 'subscriptions.opml';
      link.click();
      setTimeout(() => URL.revokeObjectURL(url), 1000);
    });
  render() {
    return html`<section class="page">
      <h1>导入 / 导出</h1>
      ${errorMessage(this.action.error || this.pollError)}
      <form class="panel" @submit=${this.submit}>
        <h2>导入订阅</h2>
        <p class="meta">选择 OPML 文件或粘贴内容，确认后开始导入。重复订阅会跳过。</p>
        <label
          >OPML 文件<input
            ?disabled=${this.action.busy}
            type="file"
            accept=".opml,.xml"
            @change=${this.chooseFile}
        /></label>
        <label
          >OPML 内容<textarea
            .value=${this.content}
            @input=${(e: Event) => {
              this.content = valueOf(e);
            }}
            rows="8"
            required
          ></textarea>
        </label>
        <button ?disabled=${this.action.busy || !this.content.trim()}>
          ${this.action.busy ? '处理中…' : '开始导入'}
        </button>
      </form>
      ${
        this.result
          ? html`<section class="panel">
              <h2>导入报告</h2>
              <p>解析成功 ${this.result.parsed} · 失败 ${this.result.failed}</p>
              ${this.report ? html`<p role="status">首抓已完成 ${this.report.completed} / ${this.report.total_first_fetch}，成功 ${this.report.first_fetch_ok}</p>` : html`<p>正在读取首抓进度…</p>`}
              <button class="secondary" @click=${this.restartPoll}>刷新报告</button>
              ${
                this.report?.normalized.length
                  ? html`<h3>规范化记录</h3>
                      <ul>
                        ${this.report.normalized.map(
                          (n) =>
                            html`<li>
                              ${n.feed_title}：${n.action}
                              <div class="url">${n.before} → ${n.after}</div>
                            </li>`,
                        )}
                      </ul>`
                  : nothing
              }
              ${
                this.report?.failures.length
                  ? html`<h3>失败项</h3>
                      <ul>
                        ${this.report.failures.map((f) => html`<li>${f.feed_title}：${f.reason}</li>`)}
                      </ul>`
                  : nothing
              }
              <p class="meta">
                抓取失败的具体原因可在“健康与日志”查看。导入报告保留最近 100 次，服务重启后清空。
              </p>
            </section>`
          : nothing
      }
      <section class="panel">
        <h2>导出订阅</h2>
        <p>导出嵌套文件夹（含空文件夹）和订阅地址。文章、阅读状态和收藏请通过数据库备份保存。</p>
        <button ?disabled=${this.action.busy} @click=${this.exportContent}>导出 OPML</button>
      </section>
    </section>`;
  }
}
customElements.define('rss-import-export', ImportExport);
