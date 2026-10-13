import { html, nothing } from 'lit';
import { AppElement } from '../../core/AppElement';
import { followLink, navigate, subscribeRoute, readingContext } from '../../core/Router';
import { Action } from '../../controllers/Action';
import { messageOf } from '../../controllers/Resource';
import { errorMessage } from '../Feedback/Feedback';
import { api, ApiError } from '../../Api';
import { auth } from '../../Store';
import '../../pages/Login/Login';
import styles from './App.module.scss';

const routes: Record<string, { tag: string; load: () => Promise<unknown> }> = {
  '/': { tag: 'rss-reader', load: () => import('../../pages/Reader/Reader') },
  '/search': { tag: 'rss-search', load: () => import('../../pages/Search/Search') },
  '/health': { tag: 'rss-health', load: () => import('../../pages/Health/Health') },
  '/import-export': { tag: 'rss-import-export', load: () => import('../../pages/ImportExport/ImportExport') },
  '/settings': { tag: 'rss-settings', load: () => import('../../pages/Settings/Settings') },
};
const navigation = [
  ['/', '文章'],
  ['/search', '搜索'],
  ['/health', '健康与日志'],
  ['/import-export', '导入导出'],
  ['/settings', '设置'],
];
export class App extends AppElement {
  private booting = true;
  private bootError = '';
  private routeError = '';
  private page?: HTMLElement;
  private revision = 0;
  private bootRevision = 0;
  private unsubscribe: (() => void)[] = [];
  private action = new Action(this);
  connectedCallback() {
    super.connectedCallback();
    this.unsubscribe = [
      subscribeRoute(this.showRoute),
      auth.subscribe(() => {
        if (!this.booting) void this.showRoute();
      }),
    ];
    void this.boot();
  }
  disconnectedCallback() {
    super.disconnectedCallback();
    this.revision++;
    this.bootRevision++;
    this.unsubscribe.forEach((off) => off());
  }
  private boot = async () => {
    const revision = ++this.bootRevision;
    this.booting = true;
    this.bootError = '';
    this.requestUpdate();
    try {
      const user = await api.authMe();
      if (this.isConnected && revision === this.bootRevision) auth.setUser(user);
    } catch (error) {
      if (!this.isConnected || revision !== this.bootRevision) return;
      auth.clear();
      if (!(error instanceof ApiError && error.status === 401)) this.bootError = messageOf(error);
    }
    if (!this.isConnected || revision !== this.bootRevision) return;
    this.booting = false;
    if (!this.bootError) await this.showRoute();
    this.requestUpdate();
  };
  private showRoute = async () => {
    if (this.booting || !this.isConnected) return;
    const revision = ++this.revision;
    const path = location.pathname;
    if (auth.user && path === '/' && this.page?.localName === 'rss-reader') {
      this.requestUpdate();
      return;
    }
    this.page = undefined;
    this.routeError = '';
    this.requestUpdate();
    if (!auth.user && path !== '/login') {
      navigate('/login', { replace: true });
      return;
    }
    if (auth.user && path === '/login') {
      navigate('/', { replace: true });
      return;
    }
    if (path === '/login') {
      this.page = document.createElement('rss-login');
      this.requestUpdate();
      return;
    }
    if (path === '/folders' || path === '/feeds') {
      navigate(path === '/folders' ? `/${location.search}` : '/', { replace: true });
      return;
    }
    if (/^\/article\/[1-9]\d*$/.test(path)) {
      const context = readingContext();
      const params = new URLSearchParams(context ? new URL(context.returnTo, location.origin).search : '');
      params.set('article', path.split('/').pop()!);
      navigate(`/?${params}`, { replace: true, state: context });
      return;
    }
    const route = routes[path];
    if (!route) {
      navigate('/', { replace: true });
      return;
    }
    try {
      await route.load();
      if (this.isConnected && revision === this.revision) this.page = document.createElement(route.tag);
    } catch (error) {
      if (revision === this.revision) this.routeError = messageOf(error);
    }
    if (revision === this.revision) this.requestUpdate();
  };
  render() {
    if (this.booting) return html`<div class="center">加载中…</div>`;
    if (this.bootError)
      return html`<div class="center">
        ${errorMessage(this.bootError)}<button @click=${this.boot}>重新连接</button>
      </div>`;
    return html`<div class=${styles.layout}>
      ${
        auth.user
          ? html`<nav class=${styles.nav} aria-label="主导航">
              ${navigation.map(([path, label]) => html`<a href=${path} class=${location.pathname === path ? styles.active : ''} aria-current=${location.pathname === path ? 'page' : 'false'} @click=${followLink}>${label}</a>`)}
              <span class=${styles.spacer}></span><span class=${styles.user}>${auth.user.username}</span>
              <button
                ?disabled=${this.action.busy}
                @click=${() =>
                  this.action.run(async (current) => {
                    await api.authLogout();
                    if (current()) auth.clear();
                  })}
              >
                退出
              </button>
            </nav>`
          : nothing
      }
      ${errorMessage(this.action.error)}
      <main class=${location.pathname === '/' ? styles.reader : styles.content}>
        ${this.routeError ? html`${errorMessage(this.routeError)}<button @click=${this.showRoute}>重新加载</button>` : (this.page ?? html`<p role="status">加载中…</p>`)}
      </main>
    </div>`;
  }
}
customElements.define('rss-app', App);
