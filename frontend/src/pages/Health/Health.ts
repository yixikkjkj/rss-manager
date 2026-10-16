import { html, nothing } from 'lit';
import { AppElement, valueOf } from '../../core/AppElement';
import { Resource } from '../../controllers/Resource';
import { Action } from '../../controllers/Action';
import { Poll } from '../../controllers/Poll';
import { errorMessage } from '../../components/Feedback/Feedback';
import { api } from '../../Api';
import { fetchResultLabel } from '../../core/FetchResult';

export class Health extends AppElement {
  static properties = { feed: { state: true }, page: { state: true } };
  declare private feed?: number;
  declare private page: number;
  private overview = new Resource(this, api.healthOverview);
  private logs = new Resource(
    this,
    () => api.fetchLogs(this.feed, this.page),
    () => [this.feed, this.page],
  );
  private action = new Action(this);
  constructor() {
    super();
    this.page = 1;
    new Poll(this, 10000, this.refresh);
  }
  private refresh = async () => {
    await Promise.all([this.overview.refresh(), this.logs.refresh()]);
  };
  render() {
    const overview = this.overview.data;
    return html`<section class="page">
      <h1>健康与抓取日志</h1>
      ${errorMessage(this.overview.error || this.logs.error || this.action.error)}
      ${
        overview
          ? html`<p>
                总计 ${overview.total} · 正常 ${overview.healthy} · 异常 ${overview.failing} · 失效
                ${overview.dead}
              </p>
              <div class="table-scroll">
                <table>
                  <thead>
                    <tr>
                      <th>源</th>
                      <th>状态</th>
                      <th>连续失败</th>
                      <th>最近错误</th>
                      <th>操作</th>
                    </tr>
                  </thead>
                  <tbody>
                    ${overview.feeds.map(
                      (f) =>
                        html`<tr>
                          <td>${f.title}</td>
                          <td>
                            ${({ healthy: '正常', failing: '异常', dead: '失效' } as Record<string, string>)[f.health_status]}
                          </td>
                          <td>${f.consecutive_failures}</td>
                          <td>${f.last_error ?? '—'}</td>
                          <td>
                            <button
                              class="secondary"
                              ?disabled=${this.action.busy}
                              @click=${() =>
                                this.action.run(async (current) => {
                                  await api.feedRetry(f.feed_id);
                                  if (current()) await this.refresh();
                                })}
                            >
                              重试
                            </button>
                          </td>
                        </tr>`,
                    )}
                  </tbody>
                </table>
              </div>`
          : nothing
      }
      <h2>最近 30 天抓取记录</h2>
      <div class="toolbar">
        <label
          >订阅源<select
            @change=${(e: Event) => {
              this.feed = Number(valueOf(e)) || undefined;
              this.page = 1;
            }}
          >
            <option value="" ?selected=${!this.feed}>全部源</option>
            ${overview?.feeds.map((f) => html`<option value=${f.feed_id} ?selected=${this.feed === f.feed_id}>${f.title}</option>`)}
          </select></label
        ><button class="secondary" @click=${this.refresh}>刷新</button>
      </div>
      <div class="table-scroll">
        <table>
          <thead>
            <tr>
              <th>源 / 时间</th>
              <th>结果</th>
              <th>新增</th>
              <th>耗时</th>
              <th>错误</th>
            </tr>
          </thead>
          <tbody>
            ${this.logs.data?.items.map(
              (log) =>
                html`<tr>
                  <td>
                    ${log.feed_title}
                    <div class="meta">${log.fetch_at?.replace('T', ' ').slice(0, 19)}</div>
                  </td>
                  <td>${fetchResultLabel(log.result)} ${log.http_status}</td>
                  <td>${log.new_article_count}</td>
                  <td>${log.duration_ms ?? '—'} ms</td>
                  <td>${log.error_message}</td>
                </tr>`,
            )}
          </tbody>
        </table>
      </div>
      ${this.logs.data?.items.length === 0 ? html`<p class="empty">暂无抓取记录。</p>` : nothing}
      <div class="pager">
        <button
          ?disabled=${this.page === 1}
          @click=${() => {
            this.page--;
          }}
        >
          上一页</button
        ><span>第 ${this.page} 页</span
        ><button
          ?disabled=${(this.logs.data?.items.length ?? 0) < 50}
          @click=${() => {
            this.page++;
          }}
        >
          下一页
        </button>
      </div>
    </section>`;
  }
}
customElements.define('rss-health', Health);
