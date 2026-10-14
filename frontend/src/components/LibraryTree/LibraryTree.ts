import { html, nothing, type TemplateResult } from 'lit';
import { keyed } from 'lit/directives/keyed.js';
import { AppElement, valueOf } from '../../core/AppElement';
import { descendantIds, folderPaths } from '../../core/Folders';
import { followLink } from '../../core/Router';
import { Action } from '../../controllers/Action';
import { errorMessage } from '../Feedback/Feedback';
import { api } from '../../Api';
import type { FeedVO, FolderVO } from '../../Types';
import '../FeedEditor/FeedEditor';
import styles from './LibraryTree.module.scss';

type Editor =
  | { kind: 'create'; parent: number | null }
  | { kind: 'rename' | 'move'; folder: FolderVO }
  | { kind: 'feed'; feed?: FeedVO; parent: number | null };
export class LibraryTree extends AppElement {
  static properties = {
    folders: { attribute: false },
    feeds: { attribute: false },
    folderId: { attribute: false },
    feedId: { attribute: false },
    unread: { type: Boolean },
    editor: { state: true },
    draft: { state: true },
    parent: { state: true },
    expanded: { state: true },
    focusKey: { state: true },
  };
  declare folders: FolderVO[];
  declare feeds: FeedVO[];
  declare folderId?: number;
  declare feedId?: number;
  declare unread: boolean;
  declare private editor?: Editor;
  declare private draft: string;
  declare private parent: number | null;
  declare private expanded: Set<number>;
  declare private focusKey: string;
  private action = new Action(this);
  private message = '';
  private failedIcons = new Set<string>();
  private revealedSelection = '';
  private revealSelection() {
    const folderId = this.feedId
      ? this.feeds.find((feed) => feed.id === this.feedId)?.folder_id
      : this.folderId;
    if (
      (this.feedId && folderId === undefined) ||
      (folderId && !this.folders.some((folder) => folder.id === folderId))
    )
      return;
    let parent =
      !this.feedId && this.folderId
        ? this.folders.find((folder) => folder.id === folderId)?.parent_id
        : folderId;
    const ancestors = new Set<number>();
    while (parent != null && !ancestors.has(parent)) {
      ancestors.add(parent);
      parent = this.folders.find((folder) => folder.id === parent)?.parent_id;
    }
    const key = `${this.folderId}/${this.feedId}/${[...ancestors].join('/')}`;
    if (this.revealedSelection === key) return;
    this.revealedSelection = key;
    for (const id of ancestors) this.expand(id, true);
  }

  constructor() {
    super();
    this.folders = [];
    this.feeds = [];
    this.unread = false;
    this.draft = '';
    this.parent = null;
    this.focusKey = 'all';
    try {
      const value: unknown = JSON.parse(sessionStorage.getItem('rss-expanded-folders') ?? '[]');
      this.expanded = new Set(Array.isArray(value) ? value.filter(Number.isSafeInteger) : []);
    } catch {
      this.expanded = new Set();
    }
  }
  protected updated() {
    this.revealSelection();
    const dialog = this.querySelector('dialog');
    if (dialog && !dialog.open) dialog.showModal();
  }
  private expand(id: number, open: boolean) {
    const next = new Set(this.expanded);
    if (open) next.add(id);
    else next.delete(id);
    this.expanded = next;
    try {
      sessionStorage.setItem('rss-expanded-folders', JSON.stringify([...next]));
    } catch {
      /* Storage can be disabled. */
    }
  }
  private changed() {
    this.dispatchEvent(new CustomEvent('library-changed', { bubbles: true }));
  }
  private open(editor: Editor) {
    this.editor = editor;
    this.draft = editor.kind === 'rename' ? editor.folder.name : '';
    this.parent = editor.kind === 'move' ? editor.folder.parent_id : null;
    this.action.error = '';
    this.querySelectorAll('details[open]').forEach((details) => details.removeAttribute('open'));
  }
  private async save(event: Event) {
    event.preventDefault();
    const editor = this.editor;
    if (!editor || editor.kind === 'feed') return;
    await this.action.run(async (current) => {
      if (editor.kind === 'create') {
        await api.folderCreate(this.draft, editor.parent);
        if (editor.parent !== null) this.expand(editor.parent, true);
      } else if (editor.kind === 'rename') await api.folderRename(editor.folder.id, this.draft);
      else {
        await api.folderMove(editor.folder.id, this.parent);
        if (this.parent !== null) this.expand(this.parent, true);
      }
      if (current()) {
        this.editor = undefined;
        this.changed();
      }
    });
  }
  private removeFolder(folder: FolderVO) {
    if (
      !window.confirm(
        `删除文件夹“${folder.name}”？其中的收藏关联将移除，文章本身保留。包含子文件夹或订阅源时需先移出。`,
      )
    )
      return;
    void this.action.run(async (current) => {
      await api.folderDelete(folder.id);
      if (current()) this.changed();
    });
  }
  private removeFeed(feed: FeedVO) {
    if (!window.confirm(`删除订阅源“${feed.title}”？历史文章和收藏仍保留。`)) return;
    void this.action.run(async (current) => {
      await api.feedDelete(feed.id);
      if (current()) this.changed();
    });
  }
  private folderMenu(folder: FolderVO) {
    return html`<details class=${styles.menu}>
      <summary aria-label=${`管理文件夹 ${folder.name}`}>⋯</summary>
      <div>
        <button @click=${() => this.open({ kind: 'create', parent: folder.id })}>新建子文件夹</button>
        <button @click=${() => this.open({ kind: 'feed', parent: folder.id })}>添加订阅源</button>
        <button @click=${() => this.open({ kind: 'rename', folder })}>重命名</button>
        <button @click=${() => this.open({ kind: 'move', folder })}>移动文件夹</button>
        <button ?disabled=${this.action.busy} @click=${() => this.removeFolder(folder)}>删除文件夹</button>
      </div>
    </details>`;
  }
  private feedMenu(feed: FeedVO) {
    return html`<details class=${styles.menu}>
      <summary aria-label=${`管理订阅源 ${feed.title}`}>⋯</summary>
      <div>
        <button @click=${() => this.open({ kind: 'feed', feed, parent: feed.folder_id })}>编辑订阅源</button>
        <button
          ?disabled=${this.action.busy}
          @click=${() =>
            this.action.run(async (current) => {
              this.message = '正在测试订阅源…';
              const result = await api.feedTest(feed.id);
              if (current()) {
                this.message = `${result.result}，新增 ${result.new_articles} 篇${result.error_message ? '：' + result.error_message : ''}`;
                this.changed();
              }
            })}
        >
          测试订阅源
        </button>
        <button
          ?disabled=${this.action.busy}
          @click=${() =>
            this.action.run(async (current) => {
              await api.feedRetry(feed.id);
              if (current()) {
                this.message = '已加入重试队列';
                this.changed();
              }
            })}
        >
          重试抓取
        </button>
        <button ?disabled=${this.action.busy} @click=${() => this.removeFeed(feed)}>删除订阅源</button>
      </div>
    </details>`;
  }
  private renderFeedIcon(feed: FeedVO) {
    let iconUrl: string | undefined;
    for (const address of [feed.html_url, feed.xml_url]) {
      if (!address) continue;
      try {
        const url = new URL(address);
        if (url.protocol !== 'https:' && url.protocol !== 'http:') continue;
        iconUrl = new URL('/favicon.ico', url.origin).href;
        break;
      } catch {
        // Older subscriptions may contain an invalid website address.
      }
    }
    const src = [feed.icon_url, iconUrl].find((address) => {
      if (!address || this.failedIcons.has(address)) return false;
      try {
        const url = new URL(address);
        return url.protocol === 'https:' || url.protocol === 'http:';
      } catch {
        return false;
      }
    });
    return html`<span class=${styles.feedIcon} aria-hidden="true">
      ${
        src
          ? keyed(
              src,
              html`<img
                src=${src}
                alt=""
                width="16"
                height="16"
                loading="lazy"
                referrerpolicy="no-referrer"
                @error=${() => {
                  this.failedIcons.add(src);
                  this.requestUpdate();
                }}
              />`,
            )
          : html`<svg viewBox="0 0 24 24" fill="none" focusable="false">
              <circle cx="5" cy="19" r="2" fill="currentColor" />
              <path
                d="M4 11a9 9 0 0 1 9 9M4 4a16 16 0 0 1 16 16"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
              />
            </svg>`
      }
    </span>`;
  }
  private renderChildren(parent: number | null): TemplateResult {
    return html`${this.folders
      .filter((folder) => folder.parent_id === parent)
      .map((folder) => {
        const open = this.expanded.has(folder.id);
        return html`<li
          role="treeitem"
          aria-label=${folder.name}
          aria-expanded=${String(open) as 'true' | 'false'}
          aria-selected=${String(this.folderId === folder.id && !this.feedId) as 'true' | 'false'}
          tabindex=${this.focusKey === `folder-${folder.id}` ? 0 : -1}
          data-key=${`folder-${folder.id}`}
          data-folder=${folder.id}
        >
          <div
            class=${`${styles.row} ${folder.unread_count > 0 ? styles.unread : ''} ${this.folderId === folder.id && !this.feedId ? styles.selected : ''}`}
          >
            <button
              class=${styles.toggle}
              tabindex="-1"
              aria-label=${`${open ? '折叠' : '展开'} ${folder.name}`}
              @click=${() => this.expand(folder.id, !open)}
            >
              ${open ? '▾' : '▸'}
            </button>
            <a href=${`/?folder=${folder.id}`} @click=${followLink} title=${folder.name}
              ><span class=${styles.folderIcon} aria-hidden="true">📂</span> ${folder.name}</a
            ><span class=${styles.count} title="子树未读文章">${folder.unread_count || ''}</span
            >${this.folderMenu(folder)}
          </div>
          ${
            open
              ? html`<ul role="group">
                  ${this.renderChildren(folder.id)}
                </ul>`
              : nothing
          }
        </li>`;
      })}${this.feeds
      .filter((feed) => feed.folder_id === parent)
      .map(
        (feed) =>
          html`<li
            role="treeitem"
            aria-label=${feed.title}
            aria-selected=${String(this.feedId === feed.id) as 'true' | 'false'}
            tabindex=${this.focusKey === `feed-${feed.id}` ? 0 : -1}
            data-key=${`feed-${feed.id}`}
          >
            <div
              class=${`${styles.row} ${feed.unread_count > 0 ? styles.unread : ''} ${this.feedId === feed.id ? styles.selected : ''}`}
            >
              ${this.renderFeedIcon(feed)}<a
                href=${`/?feed=${feed.id}`}
                @click=${followLink}
                title=${feed.title}
                >${feed.title}</a
              ><span class=${styles.count}>${feed.unread_count || ''}</span>${this.feedMenu(feed)}
            </div>
          </li>`,
      )}`;
  }
  private keydown(event: KeyboardEvent) {
    const node = event.target as HTMLElement;
    if (node.getAttribute('role') !== 'treeitem') return;
    const items = [...this.querySelectorAll<HTMLElement>('[role="treeitem"]')];
    const index = items.indexOf(node);
    const folder = Number(node.dataset.folder);
    let next: HTMLElement | undefined;
    if (event.key === 'ArrowDown') next = items[index + 1];
    else if (event.key === 'ArrowUp') next = items[index - 1];
    else if (event.key === 'Home') next = items[0];
    else if (event.key === 'End') next = items[items.length - 1];
    else if (event.key === 'ArrowRight' && folder) {
      if (!this.expanded.has(folder)) this.expand(folder, true);
      else next = items[index + 1];
    } else if (event.key === 'ArrowLeft') {
      if (folder && this.expanded.has(folder)) this.expand(folder, false);
      else next = node.parentElement?.closest<HTMLElement>('[role="treeitem"]') ?? undefined;
    } else if (event.key === 'Enter' || event.key === ' ')
      node.querySelector<HTMLAnchorElement>('a')?.click();
    else return;
    event.preventDefault();
    if (next) {
      this.focusKey = next.dataset.key!;
      next.focus();
    }
  }
  render() {
    const editor = this.editor;
    const excluded =
      editor?.kind === 'move' ? descendantIds(this.folders, editor.folder.id) : new Set<number>();
    return html`<div class=${styles.library}>
      <div class=${styles.heading}>
        <strong>资料库</strong>
        <div>
          <button
            class="secondary"
            aria-label="新建文件夹"
            @click=${() => this.open({ kind: 'create', parent: null })}
          >
            ＋文件夹</button
          ><button
            class="secondary"
            aria-label="添加订阅源"
            @click=${() => this.open({ kind: 'feed', parent: null })}
          >
            ＋订阅
          </button>
        </div>
      </div>
      ${!editor ? errorMessage(this.action.error) : nothing}${this.message ? html`<p role="status" class="meta">${this.message}</p>` : nothing}
      <ul class=${styles.tree} role="tree" aria-label="文件夹和订阅源" @keydown=${this.keydown}>
        <li
          role="treeitem"
          aria-label="全部文章"
          aria-selected=${String(!this.folderId && !this.feedId && !this.unread) as 'true' | 'false'}
          tabindex=${this.focusKey === 'all' ? 0 : -1}
          data-key="all"
        >
          <div class=${styles.row}><a href="/" @click=${followLink}>▤ 全部文章</a></div>
        </li>
        <li
          role="treeitem"
          aria-label="未读文章"
          aria-selected=${String(!this.folderId && !this.feedId && this.unread) as 'true' | 'false'}
          tabindex=${this.focusKey === 'unread' ? 0 : -1}
          data-key="unread"
        >
          <div class=${styles.row}><a href="/?unread=1" @click=${followLink}>○ 未读文章</a></div>
        </li>
        ${this.renderChildren(null)}
      </ul>
      ${!this.folders.length && !this.feeds.length ? html`<p class="meta">创建文件夹、添加订阅源，或从导入导出页面导入 OPML。</p>` : nothing}
      ${
        editor
          ? html`<dialog
              class=${styles.dialog}
              aria-label=${editor.kind === 'feed' ? '订阅源设置' : '文件夹设置'}
              @close=${() => {
                this.editor = undefined;
              }}
            >
              <button
                class=${styles.close}
                aria-label="关闭"
                @click=${() => {
                  this.editor = undefined;
                }}
              >
                ×
              </button>
              ${
                editor.kind === 'feed'
                  ? keyed(
                      editor.feed?.id ?? 'new',
                      html`<rss-feed-editor
                        .feed=${editor.feed}
                        .parentId=${editor.parent ?? undefined}
                        .folders=${this.folders}
                        @cancel=${() => {
                          this.editor = undefined;
                        }}
                        @saved=${(event: CustomEvent<{ folderId: number | null }>) => {
                          if (event.detail.folderId !== null) this.expand(event.detail.folderId, true);
                          this.editor = undefined;
                          this.changed();
                        }}
                      ></rss-feed-editor>`,
                    )
                  : html` <form @submit=${this.save}>
                      <h2>
                        ${editor.kind === 'create' ? '新建文件夹' : editor.kind === 'rename' ? '重命名文件夹' : '移动文件夹'}
                      </h2>
                      ${
                        editor.kind !== 'move'
                          ? html`<label
                              >文件夹名称<input
                                .value=${this.draft}
                                @input=${(e: Event) => {
                                  this.draft = valueOf(e);
                                }}
                                required
                                maxlength="200"
                                autofocus
                            /></label>`
                          : html`<label
                              >目标文件夹<select
                                @change=${(e: Event) => {
                                  this.parent = Number(valueOf(e)) || null;
                                }}
                              >
                                <option value="" ?selected=${this.parent === null}>根目录</option>
                                ${folderPaths(this.folders)
                                  .filter((f) => !excluded.has(f.id))
                                  .map(
                                    (f) =>
                                      html`<option value=${f.id} ?selected=${this.parent === f.id}>
                                        ${f.path}
                                      </option>`,
                                  )}
                              </select></label
                            >`
                      }
                      ${errorMessage(this.action.error)}<button ?disabled=${this.action.busy}>
                        ${editor.kind === 'create' ? '创建文件夹' : '保存'}
                      </button>
                    </form>`
              }
            </dialog>`
          : nothing
      }
    </div>`;
  }
}
customElements.define('rss-library-tree', LibraryTree);
