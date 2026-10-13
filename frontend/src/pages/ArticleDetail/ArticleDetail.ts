import { html, nothing } from 'lit';
import { AppElement, checkedOf } from '../../core/AppElement';
import { followLink, navigate, readingContext } from '../../core/Router';
import { messageOf } from '../../controllers/Resource';
import { Action } from '../../controllers/Action';
import { errorMessage } from '../../components/Feedback/Feedback';
import { safeHtml, safeHref } from '../../components/SafeHtml/SafeHtml';
import { api } from '../../Api';
import type { ArticleDetailVO, FolderVO, ReadingContext } from '../../Types';
import { folderPaths } from '../../core/Folders';
import styles from './ArticleDetail.module.scss';

export class ArticleDetail extends AppElement {
  static properties = {
    articleId: { attribute: false },
    context: { attribute: false },
    folderOptions: { attribute: false },
  };
  declare articleId: number;
  declare context: ReadingContext | null;
  declare folderOptions: FolderVO[];
  constructor() {
    super();
    this.articleId = 0;
    this.context = readingContext();
    this.folderOptions = [];
  }
  private article?: ArticleDetailVO;
  private error = '';
  private extracting = false;
  private marking = false;
  private revision = 0;
  private action = new Action(this);
  private get index() {
    return this.context?.ids.indexOf(this.articleId) ?? -1;
  }
  private get previous() {
    return this.index > 0 ? this.context?.ids[this.index - 1] : undefined;
  }
  private get next() {
    return this.index >= 0 ? this.context?.ids[this.index + 1] : undefined;
  }
  connectedCallback() {
    super.connectedCallback();
    document.addEventListener('keydown', this.keydown);
    void this.load();
  }
  disconnectedCallback() {
    super.disconnectedCallback();
    this.revision++;
    document.removeEventListener('keydown', this.keydown);
  }
  private load = async () => {
    const revision = ++this.revision;
    const current = () => this.isConnected && revision === this.revision;
    this.article = undefined;
    this.error = '';
    this.extracting = false;
    this.marking = false;
    this.requestUpdate();
    try {
      const value = await api.articleGet(this.articleId);
      if (!current()) return;
      this.article = value;
      if (!value.is_read) {
        this.marking = true;
        this.requestUpdate();
        await api.articleMarkRead(this.articleId);
        if (!current()) return;
        this.article = { ...this.article!, is_read: true };
        this.marking = false;
        this.notifyRead();
      }
      this.requestUpdate();
      if (!value.use_readability || value.full_content || value.full_content_status === 'failed') return;
      this.extracting = true;
      this.requestUpdate();
      const full = await api.articleFetchContent(this.articleId);
      if (current()) this.mergeContent(full);
    } catch (error) {
      if (current()) this.error = messageOf(error);
    } finally {
      if (current()) {
        this.extracting = false;
        this.marking = false;
        this.requestUpdate();
      }
    }
  };
  private mergeContent(full: ArticleDetailVO) {
    if (this.article)
      this.article = {
        ...this.article,
        full_content: full.full_content,
        full_content_status: full.full_content_status,
      };
  }
  private notifyRead() {
    this.dispatchEvent(
      new CustomEvent('article-state-changed', {
        bubbles: true,
        detail: { id: this.articleId, is_read: this.article?.is_read },
      }),
    );
  }
  private toggleRead = () => {
    const article = this.article;
    if (!article || this.marking) return;
    void this.action.run(async (current) => {
      await (article.is_read ? api.articleMarkUnread(article.id) : api.articleMarkRead(article.id));
      if (current() && this.article) {
        this.article = { ...this.article, is_read: !article.is_read };
        this.notifyRead();
      }
    });
  };
  private go = (id?: number) => {
    if (id) {
      const params = new URLSearchParams(location.search);
      params.set('article', String(id));
      navigate(`/?${params}`, { state: this.context });
    }
  };
  private keydown = (event: KeyboardEvent) => {
    if (
      document.querySelector('dialog[open]') ||
      event.ctrlKey ||
      event.altKey ||
      event.metaKey ||
      (event.target instanceof HTMLElement &&
        (event.target.isContentEditable || /^(INPUT|TEXTAREA|SELECT|BUTTON)$/.test(event.target.tagName)))
    )
      return;
    if (event.key === 'j') {
      event.preventDefault();
      this.go(this.next);
    }
    if (event.key === 'k') {
      event.preventDefault();
      this.go(this.previous);
    }
    if (event.key === 'm') {
      event.preventDefault();
      this.toggleRead();
    }
  };
  private fetchFull = () =>
    void this.action.run(async (current) => {
      this.extracting = true;
      try {
        const full = await api.articleFetchContent(this.articleId, true);
        if (current()) this.mergeContent(full);
      } finally {
        if (current()) this.extracting = false;
      }
    });
  private favorite = async (folderId: number, checked: boolean) => {
    if (!this.article || this.action.busy) return;
    const before = this.article.folder_ids;
    const revision = this.revision;
    this.article = {
      ...this.article,
      folder_ids: checked ? [...before, folderId] : before.filter((id) => id !== folderId),
    };
    const saved = await this.action.run(async (current) => {
      await (checked
        ? api.articleFavorite(this.articleId, folderId)
        : api.articleUnfavorite(this.articleId, folderId));
      if (current()) this.dispatchEvent(new CustomEvent('favorites-changed', { bubbles: true }));
    });
    if (!saved && this.isConnected && revision === this.revision && this.article) {
      this.article = { ...this.article, folder_ids: before };
      this.requestUpdate();
    }
  };
  render() {
    const article = this.article;
    const href = article && safeHref(article.link);
    return html`<section class=${styles.article}>
      <div class="toolbar">
        <a href=${this.context?.returnTo ?? '/'} @click=${followLink}>返回列表</a>
        <button class="secondary" ?disabled=${!this.previous} @click=${() => this.go(this.previous)}>
          上一篇 (k)
        </button>
        <button class="secondary" ?disabled=${!this.next} @click=${() => this.go(this.next)}>
          下一篇 (j)
        </button>
      </div>
      ${errorMessage(this.error || this.action.error)}
      ${this.error ? html`<button @click=${this.load}>重新加载</button>` : nothing}
      ${
        !article
          ? !this.error
            ? html`<p role="status">加载中…</p>`
            : nothing
          : html`
              <h1>${article.title}</h1>
              <p class="meta">
                ${article.feed_title} · ${article.published_at?.replace('T', ' ').slice(0, 19)}
                ${article.author}
              </p>
              <div class="actions">
                ${href ? html`<a href=${href} target="_blank" rel="noopener noreferrer">查看原文</a>` : nothing}
                <button
                  class="secondary"
                  ?disabled=${this.action.busy || this.marking}
                  @click=${this.toggleRead}
                >
                  ${article.is_read ? '标记未读' : '标记已读'} (m)
                </button>
                ${article.use_readability && !article.full_content ? html`<button class="secondary" ?disabled=${this.action.busy || this.extracting} @click=${this.fetchFull}>重新提取全文</button>` : nothing}
              </div>
              ${this.extracting ? html`<p role="status">正在提取全文，先显示订阅正文…</p>` : nothing}
              ${article.full_content_status === 'failed' ? html`<p class="notice">未能提取全文，已保留订阅正文，可查看原文或重试。</p>` : nothing}
              <details class="panel">
                <summary>
                  收藏到文件夹（已选
                  ${this.folderOptions.filter((folder) => article.folder_ids.includes(folder.id)).length} 个）
                </summary>
                ${folderPaths(this.folderOptions).map((folder) => html`<label class="checkbox"><input type="checkbox" ?disabled=${this.action.busy} .checked=${article.folder_ids.includes(folder.id)} @change=${(e: Event) => this.favorite(folder.id, checkedOf(e))} />${folder.path}</label>`)}
                <p class="meta">在左侧文件树中创建和管理文件夹。</p>
              </details>
              ${safeHtml(article.full_content || article.content || '<p>无正文内容，可查看原文。</p>')}
            `
      }
    </section>`;
  }
}
customElements.define('rss-article-detail', ArticleDetail);
