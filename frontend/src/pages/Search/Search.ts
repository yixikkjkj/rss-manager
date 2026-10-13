import { html, nothing } from 'lit';
import { repeat } from 'lit/directives/repeat.js';
import { AppElement, valueOf } from '../../core/AppElement';
import { followLink, pageNumber, updateQuery } from '../../core/Router';
import { Resource } from '../../controllers/Resource';
import { errorMessage, pager } from '../../components/Feedback/Feedback';
import { highlight } from '../../components/Highlight/Highlight';
import { api } from '../../Api';
import styles from './Search.module.scss';

export class Search extends AppElement {
  private params = new URLSearchParams(location.search);
  private q = this.params.get('q') ?? '';
  private feed = Number(this.params.get('feed')) || undefined;
  private sort: 'newest' | 'relevance' = this.params.get('sort') === 'newest' ? 'newest' : 'relevance';
  private from = this.params.get('from') || undefined;
  private to = this.params.get('to') || undefined;
  private page = pageNumber(this.params);
  private input = this.q;
  private feeds = new Resource(this, () => api.feedList());
  private result = new Resource(this, () =>
    this.q
      ? api.searchQuery({
          q: this.q,
          feed_id: this.feed,
          sort: this.sort,
          date_from: this.from,
          date_to: this.to,
          page: this.page,
        })
      : Promise.resolve(null),
  );
  render() {
    const data = this.result.data;
    const context = {
      ids: data?.items.map((item) => item.article_id) ?? [],
      returnTo: `/search?${this.params}`,
    };
    return html`<section class="page">
      <h1>全文搜索</h1>
      <form
        class="toolbar"
        @submit=${(e: Event) => {
          e.preventDefault();
          updateQuery('q', this.input.trim());
        }}
      >
        <label class="grow"
          >关键词<input
            .value=${this.input}
            @input=${(e: Event) => {
              this.input = valueOf(e);
            }}
            required
            maxlength="200"
            placeholder="搜索标题和正文" /></label
        ><button>搜索</button>
      </form>
      <div class="toolbar">
        <label
          >订阅源<select @change=${(e: Event) => updateQuery('feed', valueOf(e))}>
            <option value="" ?selected=${!this.feed}>全部源</option>
            ${this.feeds.data?.items.map((f) => html`<option value=${f.id} ?selected=${this.feed === f.id}>${f.title}</option>`)}
          </select></label
        >
        <label
          >排序<select @change=${(e: Event) => updateQuery('sort', valueOf(e))}>
            <option value="relevance" ?selected=${this.sort === 'relevance'}>相关度</option>
            <option value="newest" ?selected=${this.sort === 'newest'}>最新发布</option>
          </select></label
        >
        <label
          >开始日期<input
            type="date"
            .value=${this.from ?? ''}
            @change=${(e: Event) => updateQuery('from', valueOf(e))}
        /></label>
        <label
          >结束日期<input
            type="date"
            .value=${this.to ?? ''}
            @change=${(e: Event) => updateQuery('to', valueOf(e))}
        /></label>
      </div>
      ${errorMessage(this.result.error || this.feeds.error)}
      ${
        this.q && this.result.loading
          ? html`<p role="status">搜索中…</p>`
          : data
            ? html` <p class="meta">共 ${data.total} 条结果，耗时 ${data.took_ms} ms</p>
                <ul class=${styles.list} aria-label="文章列表">
                  ${repeat(
                    data.items,
                    (item) => item.article_id,
                    (item) =>
                      html`<li>
                        <a
                          href=${`/article/${item.article_id}`}
                          @click=${(e: MouseEvent) => followLink(e, context)}
                          >${highlight(item.title, this.q)}</a
                        >
                        <p class="meta">${item.feed_title} · ${item.published_at?.slice(0, 10)}</p>
                        <p class=${styles.summary}>${highlight(item.snippet, this.q)}</p>
                      </li>`,
                  )}
                </ul>
                ${!data.total ? html`<p class="empty">没有找到匹配文章，试试其他关键词或筛选条件。</p>` : nothing}
                ${pager(this.page, data.total, 20, (p) => updateQuery('page', String(p)))}`
            : nothing
      }
    </section>`;
  }
}
customElements.define('rss-search', Search);
