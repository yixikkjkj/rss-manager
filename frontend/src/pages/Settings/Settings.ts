import { html, nothing } from 'lit';
import { AppElement, valueOf } from '../../core/AppElement';
import { Resource } from '../../controllers/Resource';
import { Action } from '../../controllers/Action';
import { errorMessage } from '../../components/Feedback/Feedback';
import { api } from '../../Api';
import { auth } from '../../Store';

import '../../components/IntervalForm/IntervalForm';

export class Settings extends AppElement {
  private oldPwd = '';
  private newPwd = '';
  private message = '';
  private config = new Resource(this, api.fetchConfig);
  private status = new Resource(this, api.fetchStatus);
  private action = new Action(this);
  render() {
    return html`<section class="page">
      <h1>设置</h1>
      ${errorMessage(this.config.error || this.status.error || this.action.error)}
      ${this.config.data ? html`<rss-interval-form .seconds=${this.config.data.fetch_interval_sec}></rss-interval-form>` : nothing}
      <section class="panel">
        <h2>抓取任务</h2>
        <div class="actions">
          <button
            ?disabled=${this.action.busy}
            @click=${() =>
              this.action.run(async (current) => {
                await api.fetchNow();
                if (current()) {
                  this.message = '已加入抓取队列';
                  void this.status.refresh();
                }
              })}
          >
            立即抓取全部
          </button>
          <button class="secondary" @click=${this.status.refresh}>刷新状态</button>
        </div>
        ${this.message ? html`<p role="status">${this.message}</p>` : nothing}
        ${this.status.data ? html`<p>正在抓取 ${this.status.data.active_fetches} / ${this.status.data.max_concurrency}，等待 ${this.status.data.queue_len}</p>` : nothing}
      </section>
      <form
        class="panel"
        @submit=${(e: Event) => {
          e.preventDefault();
          void this.action.run(async (current) => {
            await api.authChangePassword(this.oldPwd, this.newPwd);
            if (current()) auth.clear();
          });
        }}
      >
        <h2>修改密码</h2>
        <p class="meta">至少 12 位，包含大小写字母、数字和符号。修改后需重新登录。</p>
        <label
          >旧密码<input
            type="password"
            autocomplete="current-password"
            required
            .value=${this.oldPwd}
            @input=${(e: Event) => {
              this.oldPwd = valueOf(e);
            }}
        /></label>
        <label
          >新密码<input
            type="password"
            autocomplete="new-password"
            minlength="12"
            required
            .value=${this.newPwd}
            @input=${(e: Event) => {
              this.newPwd = valueOf(e);
            }}
        /></label>
        <button ?disabled=${this.action.busy}>修改密码</button>
      </form>
      <section class="panel">
        <h2>备份与恢复</h2>
        <p>OPML 导出保存订阅结构。完整备份还需保存 SQLite 数据库，包含文章、阅读状态和收藏。</p>
        <p>部署文档提供在线备份脚本与恢复步骤。恢复前停止服务，并保留当前数据库副本。</p>
      </section>
    </section>`;
  }
}
customElements.define('rss-settings', Settings);
