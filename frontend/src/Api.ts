import { auth } from './Store';
import type {
  ArticleDetailVO,
  ArticleFilter,
  ArticleListVO,
  FeedInput,
  FeedVO,
  FetchLog,
  FetchStatus,
  FolderVO,
  HealthOverviewVO,
  ImportReportVO,
  OpmlImportResultVO,
  Resp,
  SearchFilter,
  SearchResultVO,
  UnreadCountVO,
  User,
} from './Types';

export class ApiError extends Error {
  constructor(
    message: string,
    public status: number,
    public code?: string,
  ) {
    super(message);
  }
}

// Navigation belongs to the application shell. Never navigate or reload from the HTTP layer:
// /login also checks auth_me, and a full-page 401 redirect would loop forever.
export async function rpc<T>(name: string, body: object = {}): Promise<T> {
  const response = await fetch(`/api/rpc/${name}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    credentials: 'same-origin',
    body: JSON.stringify(body),
  });
  if (response.status === 401 && name !== 'auth_login') auth.clear();
  let json: Resp<T>;
  try {
    json = (await response.json()) as Resp<T>;
  } catch {
    throw new ApiError(`请求失败（HTTP ${response.status}），请稍后重试`, response.status);
  }
  if (!response.ok || json.code !== '0')
    throw new ApiError(json.msg || '请求失败', response.status, json.code);
  return json.data as T;
}
export const api = {
  authLogin: (username: string, password: string) => rpc<User>('auth_login', { username, password }),
  authMe: () => rpc<User>('auth_me'),
  authLogout: () => rpc<void>('auth_logout'),
  authChangePassword: (old_password: string, new_password: string) =>
    rpc<void>('auth_change_password', { old_password, new_password }),
  feedList: (folder_id?: number) => rpc<{ items: FeedVO[] }>('feed_list', { folder_id }),
  feedCreate: (req: FeedInput) => rpc<FeedVO>('feed_create', req),
  feedUpdate: (req: FeedInput & { id: number }) => rpc<FeedVO>('feed_update', req),
  feedDelete: (id: number) => rpc<void>('feed_delete', { id }),
  feedTest: (id: number) =>
    rpc<{ result: string; new_articles: number; error_message: string | null }>('feed_test', { id }),
  fetchNow: (feed_id?: number) => rpc<void>('fetch_now', { feed_id }),
  fetchStatus: () => rpc<FetchStatus>('fetch_status'),
  fetchConfig: () => rpc<{ fetch_interval_sec: number }>('fetch_config_get'),
  updateFetchConfig: (fetch_interval_sec: number) =>
    rpc<{ fetch_interval_sec: number }>('fetch_config_update', { fetch_interval_sec }),
  fetchLogs: (feed_id?: number, page = 1) => rpc<{ items: FetchLog[] }>('fetch_log_list', { feed_id, page }),
  healthOverview: () => rpc<HealthOverviewVO>('health_overview'),
  feedRetry: (id: number) => rpc<void>('feed_retry', { id }),
  articleList: (req: ArticleFilter) => rpc<ArticleListVO>('article_list', req),
  articleGet: (id: number) => rpc<ArticleDetailVO>('article_get', { id }),
  articleMarkRead: (id: number) => rpc<void>('article_mark_read', { id }),
  articleMarkUnread: (id: number) => rpc<void>('article_mark_unread', { id }),
  articleReadAll: (folder_id?: number, feed_id?: number) =>
    rpc<void>('article_read_all', { folder_id, feed_id }),
  unreadCount: () => rpc<UnreadCountVO>('unread_count'),
  articleFetchContent: (id: number, retry = false) =>
    rpc<ArticleDetailVO>('article_fetch_content', { id, retry }),
  folderList: () => rpc<{ items: FolderVO[] }>('folder_list'),
  folderCreate: (name: string, parent_id: number | null = null) =>
    rpc<FolderVO>('folder_create', { name, parent_id }),
  folderMove: (id: number, parent_id: number | null) => rpc<void>('folder_move', { id, parent_id }),
  folderRename: (id: number, name: string) => rpc<void>('folder_rename', { id, name }),
  folderDelete: (id: number) => rpc<void>('folder_delete', { id }),
  folderArticleList: (folder_id: number, page = 1) =>
    rpc<ArticleListVO>('folder_article_list', { folder_id, page, page_size: 50 }),
  articleFavorite: (article_id: number, folder_id: number) =>
    rpc<void>('article_favorite', { article_id, folder_id }),
  articleUnfavorite: (article_id: number, folder_id: number) =>
    rpc<void>('article_unfavorite', { article_id, folder_id }),
  searchQuery: (req: SearchFilter) => rpc<SearchResultVO>('search_query', { page_size: 20, ...req }),
  opmlImport: (content: string) => rpc<OpmlImportResultVO>('opml_import', { content }),
  opmlExport: () => rpc<{ content: string }>('opml_export'),
  opmlImportReport: (import_id: number) => rpc<ImportReportVO>('opml_import_report', { import_id }),
};
