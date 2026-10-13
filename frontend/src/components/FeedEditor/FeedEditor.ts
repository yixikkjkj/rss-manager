import { html } from 'lit';
import { AppElement, valueOf, checkedOf } from '../../core/AppElement';
import { Action } from '../../controllers/Action';
import { errorMessage } from '../Feedback/Feedback';
import { api } from '../../Api';
import type { FolderVO, FeedInput, FeedVO } from '../../Types';
import { folderPaths } from '../../core/Folders';

export class FeedEditor extends AppElement {
  static properties = {
    feed: { attribute: false },
    parentId: { attribute: false },
    folders: { attribute: false },
    draft: { state: true },
  };
  declare feed?: FeedVO;
  declare parentId?: number;
  declare folders: FolderVO[];
  declare private draft: FeedInput;
  private action = new Action(this);
  constructor() {
    super();
    this.folders = [];
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
    }
  }
  private patch(value: Partial<FeedInput>) {
    this.draft = { ...this.draft, ...value };
  }
  render() {
    return html`<form
      class="panel"
      @submit=${(e: Event) => {
        e.preventDefault();
        void this.action.run(async (current) => {
          if (this.feed) await api.feedUpdate({ ...this.draft, id: this.feed.id });
          else await api.feedCreate(this.draft);
          if (current())
            this.dispatchEvent(
              new CustomEvent('saved', { bubbles: true, detail: { folderId: this.draft.folder_id } }),
            );
        });
      }}
    >
      <h2>${this.feed ? '编辑订阅源' : '添加订阅源'}</h2>
      <fieldset ?disabled=${this.action.busy}>
        <label
          >订阅地址<input
            required
            type="url"
            .value=${this.draft.xml_url}
            @input=${(e: Event) => this.patch({ xml_url: valueOf(e) })}
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
        ${errorMessage(this.action.error)}
        <div class="actions">
          <button type="submit">${this.action.busy ? '保存中…' : '保存订阅'}</button
          ><button
            type="button"
            class="secondary"
            @click=${() => this.dispatchEvent(new CustomEvent('cancel', { bubbles: true }))}
          >
            取消
          </button>
        </div>
      </fieldset>
    </form>`;
  }
}
customElements.define('rss-feed-editor', FeedEditor);
