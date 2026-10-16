import { html, nothing } from 'lit';
import { AppElement, valueOf, checkedOf } from '../../core/AppElement';
import { Action } from '../../controllers/Action';
import { errorMessage } from '../Feedback/Feedback';
import { api } from '../../Api';
import type { FolderVO, FeedInput, FeedVO, FeedRequestConfig, FeedProbeResult } from '../../Types';
import { folderPaths } from '../../core/Folders';
import { fetchResultLabel } from '../../core/FetchResult';
import styles from './FeedEditor.module.scss';

export class FeedEditor extends AppElement {
  static properties = {
    feed: { attribute: false },
    parentId: { attribute: false },
    folders: { attribute: false },
    draft: { state: true },
    loaded: { state: true },
    preview: { state: true },
    configNotice: { state: true },
  };
  declare feed?: FeedVO;
  declare parentId?: number;
  declare folders: FolderVO[];
  declare private draft: FeedInput;
  private action = new Action(this);
  private loading = new Action(this);
  private testing = new Action(this);
  declare private loaded: boolean;
  declare private preview?: FeedProbeResult;
  declare private configNotice: string;
  private configOrigin?: string;
  private get config(): FeedRequestConfig {
    return this.draft.request_config ?? { headers: [], cookie: '' };
  }
  constructor() {
    super();
    this.folders = [];
    this.loaded = true;
    this.configNotice = '';
    this.draft = {
      title: '',
      xml_url: '',
      html_url: null,
      folder_id: null,
      fetch_interval_sec: null,
      use_readability: false,
    };
  }
  connectedCallback() {
    super.connectedCallback();
    this.draft = { ...this.draft, folder_id: this.parentId ?? null };
    if (this.feed) {
      const { title, xml_url, html_url, folder_id, fetch_interval_sec, use_readability } = this.feed;
      this.draft = { title, xml_url, html_url, folder_id, fetch_interval_sec, use_readability };
      this.configOrigin = new URL(xml_url).origin;
      this.loaded = false;
      void this.loadConfig();
    }
  }
  private loadConfig = () =>
    this.loading.run(async (current) => {
      const config = await api.feedRequestConfig(this.feed!.id);
      if (current()) {
        // Older saved configurations kept Cookie separately; display it as a normal row.
        this.patch({
          request_config: {
            headers: [
              ...config.headers,
              ...(config.cookie ? [{ name: 'Cookie', value: config.cookie }] : []),
            ],
            cookie: '',
          },
        });
        this.loaded = true;
      }
    });
  private patch(value: Partial<FeedInput>) {
    this.preview = undefined;
    this.draft = { ...this.draft, ...value };
  }
  private patchConfig(value: Partial<FeedRequestConfig>) {
    this.patch({ request_config: { ...this.config, ...value } });
  }
  private changedUrl() {
    try {
      const origin = new URL(this.draft.xml_url).origin;
      if (this.configOrigin && this.configOrigin !== origin) {
        this.patchConfig({ headers: [], cookie: '' });
        this.configNotice = '订阅地址的站点已变化，请求头和 Cookie 已清空，请按需重新填写。';
      }
      this.configOrigin = origin;
    } catch {
      /* The URL input reports invalid addresses. */
    }
  }
  private testConfig = () => {
    if (!this.loaded || this.action.busy || !this.querySelector('form')?.reportValidity()) return;
    this.preview = undefined;
    void this.testing.run(async (current) => {
      const result = await api.feedProbe(this.draft.xml_url, this.config);
      if (current()) this.preview = result;
    });
  };
  render() {
    return html`<form
      class="panel"
      @submit=${(e: Event) => {
        e.preventDefault();
        if (!this.loaded || this.testing.busy) return;
        void this.action.run(async (current) => {
          if (this.feed) await api.feedUpdate({ ...this.draft, id: this.feed.id });
          else await api.feedCreate(this.draft);
          if (current())
            this.dispatchEvent(
              new CustomEvent('saved', {
                bubbles: true,
                detail: { folderId: this.draft.folder_id },
              }),
            );
        });
      }}
    >
      <h2>${this.feed ? '编辑订阅源' : '添加订阅源'}</h2>
      ${errorMessage(this.loading.error)}
      ${this.loading.error ? html`<button type="button" @click=${this.loadConfig}>重新加载请求配置</button>` : nothing}
      ${this.loading.busy ? html`<p role="status">正在加载请求配置…</p>` : nothing}
      <fieldset ?disabled=${this.action.busy || this.testing.busy || !this.loaded}>
        <label
          >订阅地址<input
            required
            type="url"
            .value=${this.draft.xml_url}
            @input=${(e: Event) => this.patch({ xml_url: valueOf(e) })}
            @change=${() => this.changedUrl()}
            placeholder="https://example.com/feed.xml"
        /></label>
        <label
          >显示名称<input
            maxlength="200"
            .value=${this.draft.title}
            @input=${(e: Event) => this.patch({ title: valueOf(e) })}
            placeholder="留空使用订阅地址"
        /></label>
        <label
          >网站地址<input
            type="url"
            .value=${this.draft.html_url ?? ''}
            @input=${(e: Event) => this.patch({ html_url: valueOf(e) || null })}
        /></label>
        <label
          >所属文件夹<select @change=${(e: Event) => this.patch({ folder_id: Number(valueOf(e)) || null })}>
            <option value="" ?selected=${this.draft.folder_id === null}>根目录</option>
            ${folderPaths(this.folders).map((g) => html`<option value=${g.id} ?selected=${this.draft.folder_id === g.id}>${g.path}</option>`)}
          </select></label
        >
        <label
          >抓取间隔（分钟）<input
            type="number"
            min="30"
            max="10080"
            .value=${this.draft.fetch_interval_sec === null ? '' : String(this.draft.fetch_interval_sec / 60)}
            placeholder="留空继承全局设置"
            @input=${(e: Event) => this.patch({ fetch_interval_sec: valueOf(e) ? Number(valueOf(e)) * 60 : null })}
        /></label>
        <label class="checkbox"
          ><input
            type="checkbox"
            .checked=${this.draft.use_readability}
            @change=${(e: Event) => this.patch({ use_readability: checkedOf(e) })}
          />打开文章时自动提取全文</label
        >
        <details class=${styles.config}>
          <summary>请求配置（可选）</summary>
          <p class=${styles.hint}>按需添加 Cookie、User-Agent 等请求头，仅用于此订阅。</p>
          ${this.configNotice ? html`<p role="status">${this.configNotice}</p>` : nothing}
          <div class=${styles.headers}>
            <div class=${styles.columns} aria-hidden="true">
              <span>请求头</span><span>请求值</span><span></span>
            </div>
            ${this.config.headers.map(
              (header, index) =>
                html`<div class=${styles.row}>
                  <input
                    type="text"
                    required
                    aria-label=${`请求头 ${index + 1}`}
                    placeholder="Cookie"
                    autocomplete="off"
                    spellcheck="false"
                    .value=${header.name}
                    @input=${(e: Event) => this.patchConfig({ headers: this.config.headers.map((h, i) => (i === index ? { ...h, name: valueOf(e) } : h)) })}
                  />
                  <input
                    type="text"
                    aria-label=${`请求值 ${index + 1}`}
                    placeholder="输入请求值"
                    autocomplete="off"
                    spellcheck="false"
                    .value=${header.value}
                    @input=${(e: Event) => this.patchConfig({ headers: this.config.headers.map((h, i) => (i === index ? { ...h, value: valueOf(e) } : h)) })}
                  />
                  <button
                    type="button"
                    class=${styles.remove}
                    aria-label=${`删除请求头 ${index + 1}`}
                    title="删除"
                    @click=${() => this.patchConfig({ headers: this.config.headers.filter((_, i) => i !== index) })}
                  >
                    ×
                  </button>
                </div>`,
            )}
            ${this.config.headers.length === 0 ? html`<p class=${styles.empty}>未添加请求头，使用默认请求配置</p>` : nothing}
            <div class=${styles.configActions}>
              <button
                type="button"
                class=${styles.add}
                ?disabled=${this.config.headers.length >= 32}
                @click=${() => this.patchConfig({ headers: [...this.config.headers, { name: '', value: '' }] })}
              >
                添加请求头
              </button>
              ${
                this.config.headers.length
                  ? html`<button
                      type="button"
                      class=${styles.clear}
                      @click=${() => this.patchConfig({ headers: [], cookie: '' })}
                    >
                      清空请求配置
                    </button>`
                  : nothing
              }
            </div>
          </div>
        </details>
        ${errorMessage(this.action.error || this.testing.error)}
        <button type="button" class="secondary" @click=${this.testConfig}>
          ${this.testing.busy ? '测试中…' : '测试当前配置'}
        </button>
        <p class=${styles.hint}>测试当前填写的配置，不保存或导入文章，也不改变健康状态。</p>
        ${
          this.preview
            ? html`<div class=${styles.result} role="status">
                <strong>${fetchResultLabel(this.preview.result)}</strong>
                <p>
                  ${this.preview.http_status === null ? '未收到 HTTP 响应' : `HTTP ${this.preview.http_status}`}
                  · ${this.preview.duration_ms} ms
                </p>
                ${this.preview.result === 'success' ? html`<p>${this.preview.feed_title || '未命名 Feed'} · 解析到 ${this.preview.item_count} 篇文章</p>` : nothing}
                ${this.preview.error_message ? html`<p>${this.preview.error_message}</p>` : nothing}
              </div>`
            : nothing
        }
      </fieldset>
      <div class="actions">
        <button type="submit" ?disabled=${!this.loaded || this.action.busy || this.testing.busy}>
          ${this.action.busy ? '保存中…' : '保存订阅'}</button
        ><button
          type="button"
          class="secondary"
          @click=${() => this.dispatchEvent(new CustomEvent('cancel', { bubbles: true }))}
        >
          取消
        </button>
      </div>
    </form>`;
  }
}
customElements.define('rss-feed-editor', FeedEditor);
