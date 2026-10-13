import { html, nothing } from 'lit';
import { keyed } from 'lit/directives/keyed.js';
import { repeat } from 'lit/directives/repeat.js';
import { AppElement, checkedOf } from '../../core/AppElement';
import { followLink, navigate, pageNumber, readingContext, subscribeRoute } from '../../core/Router';
import { Resource } from '../../controllers/Resource';
import { Action } from '../../controllers/Action';
import { errorMessage, pager } from '../../components/Feedback/Feedback';
import { api } from '../../Api';
import type { ArticleListVO, ReadingContext } from '../../Types';
import '../../components/LibraryTree/LibraryTree';
import '../ArticleDetail/ArticleDetail';
import styles from './Reader.module.scss';

export class Reader extends AppElement {
  private params = new URLSearchParams(location.search);
  private folder?: number;
  private feed?: number;
  private articleId?: number;
  private unread = false;
  private page = 1;
  private query = '';
  private pane = 'tree';
  private externalContext: ReadingContext | null = null;
  private unsubscribe?: () => void;
  private library = new Resource(this, async () => {
    const [folders, feeds] = await Promise.all([api.folderList(), api.feedList()]);
    return { folders: folders.items, feeds: feeds.items };
  });
  private articles = new Resource<ArticleListVO>(
    this,
    async () => {
      if (this.query) {
        const result = await api.searchQuery({
          q: this.query,
          folder_id: this.folder,
          feed_id: this.feed,
          page: this.page,
          page_size: 20,
          sort: this.params.get('sort') === 'newest' ? 'newest' : 'relevance',
          date_from: this.params.get('from') || undefined,
          date_to: this.params.get('to') || undefined,
        });
        return {
          total: result.total,
          has_more: this.page * 20 < result.total,
          items: result.items.map((item) => ({
            id: item.article_id,
            feed_id: item.feed_id,
            title: item.title,
            feed_title: item.feed_title,
            summary: item.snippet,
            author: null,
            published_at: item.published_at,
            is_read: item.is_read,
          })),
        };
      }
      return api.articleList({
        folder_id: this.folder,
        feed_id: this.feed,
        unread_only: this.unread,
        page: this.page,
        page_size: 50,
      });
    },
    () => [
      this.folder,
      this.feed,
      this.unread,
      this.page,
      this.query,
      this.params.get('sort'),
      this.params.get('from'),
      this.params.get('to'),
    ],
  );
  private action = new Action(this);
  constructor() {
    super();
    this.sync();
  }
  connectedCallback() {
    super.connectedCallback();
    this.unsubscribe = subscribeRoute(this.sync);
  }
  disconnectedCallback() {
    super.disconnectedCallback();
    this.unsubscribe?.();
  }
  private sync = () => {
    if (location.pathname !== '/') return;
    const beforeArticle = this.articleId;
    const beforeScope = `${this.folder}/${this.feed}/${this.unread}/${this.page}/${this.query}`;
    this.params = new URLSearchParams(location.search);
    const id = (key: string) => {
      const n = Number(this.params.get(key));
      return Number.isSafeInteger(n) && n > 0 ? n : undefined;
    };
    this.folder = id('folder');
    this.feed = id('feed');
    this.articleId = id('article');
    this.unread = this.params.get('unread') === '1';
    this.page = pageNumber(this.params);
    this.query = this.params.get('q') ?? '';
    this.externalContext = readingContext();
    if (this.articleId) this.pane = 'article';
    else if (this.isConnected || beforeArticle || this.folder || this.feed || this.unread || this.query)
      this.pane = 'list';
    if (beforeArticle !== this.articleId) this.querySelector('[data-detail]')?.scrollTo(0, 0);
    if (beforeScope !== `${this.folder}/${this.feed}/${this.unread}/${this.page}/${this.query}`) {
      this.articles.data = undefined;
      this.querySelector('[data-list]')?.scrollTo(0, 0);
    }
    this.requestUpdate();
  };
  protected updated() {
    const group = Number(this.params.get('group'));
    if (group && this.library.data) {
      const folder = this.library.data.folders.find((f) => f.legacy_group_id === group);
      const params = new URLSearchParams(this.params);
      params.delete('group');
      if (folder) params.set('folder', String(folder.id));
      navigate(`/?${params}`, { replace: true, state: this.externalContext });
    }
  }
  private get listUrl() {
    const params = new URLSearchParams(this.params);
    params.delete('article');
    return `/?${params}`;
  }
  private get context(): ReadingContext {
    return {
      ids: this.articles.data?.items.map((item) => item.id) ?? this.externalContext?.ids ?? [],
      returnTo:
        this.query && this.externalContext?.returnTo.startsWith('/search')
          ? this.externalContext.returnTo
          : this.listUrl,
    };
  }
  private change(key: string, value: string) {
    const params = new URLSearchParams(this.params);
    params.delete('article');
    if (key !== 'page') params.delete('page');
    if (value) params.set(key, value);
    else params.delete(key);
    navigate(`/?${params}`);
  }
  private libraryChanged = async () => {
    await this.library.refresh();
    if (!this.isConnected) return;
    const data = this.library.data;
    if (
      data &&
      ((this.folder && !data.folders.some((f) => f.id === this.folder)) ||
        (this.feed && !data.feeds.some((f) => f.id === this.feed)))
    )
      navigate('/');
    else await this.articles.refresh();
  };
  private readChanged = (event: CustomEvent<{ id: number; is_read: boolean }>) => {
    if (this.articles.data)
      this.articles.data = {
        ...this.articles.data,
        items: this.articles.data.items.map((item) =>
          item.id === event.detail.id ? { ...item, is_read: event.detail.is_read } : item,
        ),
      };
    void this.library.refresh();
    this.requestUpdate();
  };
  render() {
    const library = this.library.data;
    const data = this.articles.data;
    const title = this.query
      ? `搜索：${this.query}`
      : this.feed
        ? (library?.feeds.find((f) => f.id === this.feed)?.title ?? '订阅源')
        : this.folder
          ? (library?.folders.find((f) => f.id === this.folder)?.name ?? '文件夹')
          : this.unread
            ? '未读文章'
            : '全部文章';
    return html`<div class=${styles.shell} data-pane=${this.pane}>
      <div class=${styles.tabs} aria-label="阅读器面板">
        ${[
          ['tree', '文件树'],
          ['list', '文章列表'],
          ['article', '正文'],
        ].map(
          ([pane, label]) =>
            html`<button
              class="secondary"
              aria-pressed=${String(this.pane === pane) as 'true' | 'false'}
              @click=${() => {
                this.pane = pane;
                this.requestUpdate();
              }}
            >
              ${label}
            </button>`,
        )}
      </div>
      <aside class=${styles.tree} aria-label="资料库导航">
        ${errorMessage(this.library.error)}${this.library.error ? html`<button @click=${this.library.refresh}>重试资料库</button>` : nothing}
        <rss-library-tree
          .folders=${library?.folders ?? []}
          .feeds=${library?.feeds ?? []}
          .folderId=${this.folder}
          .feedId=${this.feed}
          .unread=${this.unread}
          @library-changed=${this.libraryChanged}
        ></rss-library-tree>
      </aside>
      <section class=${styles.middle} aria-label="文章列表栏">
        <header class=${styles.heading}>
          <h1>${title}</h1>
          <p>${data?.total ?? '…'} 篇文章</p>
          ${
            !this.query
              ? html`<div class=${styles.controls}>
                  <label class="checkbox"
                    ><input
                      type="checkbox"
                      .checked=${this.unread}
                      @change=${(e: Event) => this.change('unread', checkedOf(e) ? '1' : '')}
                    />仅未读</label
                  ><button
                    class="secondary"
                    ?disabled=${this.action.busy || this.articles.loading || !data?.total}
                    @click=${() =>
                      this.action.run(async (current) => {
                        await api.articleReadAll(this.folder, this.feed);
                        if (current()) {
                          await this.library.refresh();
                          await this.articles.refresh();
                          if (this.articleId) {
                            this.articleId = undefined;
                            this.change('article', '');
                          }
                        }
                      })}
                  >
                    全部标为已读
                  </button>
                </div>`
              : nothing
          }
        </header>
        ${errorMessage(this.articles.error || this.action.error)}${this.articles.error ? html`<button @click=${this.articles.refresh}>重试文章列表</button>` : nothing}
        <div class=${styles.listScroll} data-list>
          ${
            this.articles.loading
              ? html`<p role="status" class="empty">加载中…</p>`
              : html`<ul class=${styles.list} aria-label="文章列表">
                    ${repeat(
                      data?.items ?? [],
                      (item) => item.id,
                      (item) => {
                        const params = new URLSearchParams(this.params);
                        params.set('article', String(item.id));
                        return html`<li
                          class=${`${this.articleId === item.id ? styles.selected : ''} ${item.is_read ? styles.read : ''}`}
                        >
                          <a
                            href=${`/?${params}`}
                            aria-current=${this.articleId === item.id ? 'true' : 'false'}
                            @click=${(e: MouseEvent) => followLink(e, this.context)}
                            ><span class=${styles.meta}
                              >${item.feed_title} · ${item.published_at?.slice(0, 10) ?? ''}</span
                            >
                            <h2>
                              ${!item.is_read ? html`<span class=${styles.dot} aria-label="未读"></span>` : nothing}${item.title}
                            </h2>
                            <p>${item.summary}</p></a
                          >
                        </li>`;
                      },
                    )}
                  </ul>
                  ${!data?.total && !this.articles.error ? html`<p class="empty">暂无文章。选择其他文件夹或订阅源试试。</p>` : nothing}`
          }
        </div>
        <footer class=${styles.footer}>
          ${pager(this.page, data?.total ?? 0, this.query ? 20 : 50, (page) => this.change('page', String(page)))}
        </footer>
      </section>
      <section class=${styles.detail} data-detail aria-label="文章正文栏">
        ${
          this.articleId
            ? keyed(
                this.articleId,
                html`<rss-article-detail
                  .articleId=${this.articleId}
                  .context=${this.context}
                  .folderOptions=${library?.folders ?? []}
                  @article-state-changed=${this.readChanged}
                  @favorites-changed=${this.libraryChanged}
                ></rss-article-detail>`,
              )
            : html`<div class=${styles.placeholder}>
                <span aria-hidden="true">▤</span>
                <h2>选择一篇文章，开始阅读</h2>
                <p>从左侧整理订阅与收藏，在这里专注阅读。</p>
              </div>`
        }
      </section>
    </div>`;
  }
}
customElements.define('rss-reader', Reader);
