export interface Resp<T> {
  code: string;
  msg: string;
  data?: T;
  traceId: string;
  timestamp: string;
}
export interface User {
  user_id: number;
  username: string;
}
export interface FeedVO {
  id: number;
  folder_id: number | null;
  title: string;
  feed_title: string;
  xml_url: string;
  html_url: string | null;
  health_status: 'healthy' | 'failing' | 'dead';
  consecutive_failures: number;
  last_fetch_at: string | null;
  use_readability: boolean;
  unread_count: number;
  fetch_interval_sec: number | null;
}
export interface FeedInput {
  xml_url: string;
  title: string;
  folder_id: number | null;
  html_url: string | null;
  fetch_interval_sec: number | null;
  use_readability: boolean;
}
export interface ArticleListItemVO {
  id: number;
  feed_id: number;
  feed_title: string;
  title: string;
  author: string | null;
  published_at: string | null;
  is_read: boolean;
  summary: string;
}
export interface ArticleListVO {
  items: ArticleListItemVO[];
  total: number;
  has_more: boolean;
}
export interface ArticleDetailVO {
  id: number;
  feed_id: number;
  feed_title: string;
  title: string;
  author: string | null;
  link: string;
  published_at: string | null;
  content: string;
  full_content: string | null;
  is_read: boolean;
  use_readability: boolean;
  full_content_status: 'not_attempted' | 'success' | 'failed';
  folder_ids: number[];
}
export interface ArticleFilter {
  folder_id?: number;
  feed_id?: number;
  unread_only?: boolean;
  page?: number;
  page_size?: number;
}
export interface FolderVO {
  id: number;
  name: string;
  article_count: number;
  parent_id: number | null;
  sort_order: number;
  unread_count: number;
  feed_count: number;
  legacy_group_id: number | null;
}
export interface SearchFilter {
  q: string;
  page?: number;
  page_size?: number;
  feed_id?: number;
  folder_id?: number;
  sort?: 'relevance' | 'newest';
  date_from?: string;
  date_to?: string;
}
export interface SearchItemVO {
  feed_id: number;
  is_read: boolean;
  article_id: number;
  title: string;
  snippet: string;
  feed_title: string;
  published_at: string | null;
}
export interface SearchResultVO {
  total: number;
  items: SearchItemVO[];
  took_ms: number;
}
export interface FeedHealthVO {
  feed_id: number;
  title: string;
  health_status: string;
  consecutive_failures: number;
  last_fetch_at: string | null;
  last_error: string | null;
}
export interface HealthOverviewVO {
  total: number;
  healthy: number;
  failing: number;
  dead: number;
  feeds: FeedHealthVO[];
}
export interface UnreadCountVO {
  total: number;
  folders: { folder_id: number; count: number }[];
}
export interface OpmlImportResultVO {
  import_id: number;
  parsed: number;
  failed: number;
  first_fetch_ok: number;
}
export interface ImportReportVO extends OpmlImportResultVO {
  total_first_fetch: number;
  completed: number;
  normalized: { feed_title: string; action: string; before: string; after: string }[];
  failures: { feed_title: string; reason: string }[];
}
export interface FetchStatus {
  running: boolean;
  active_fetches: number;
  max_concurrency: number;
  queue_len: number;
}
export interface FetchLog {
  id: number;
  feed_id: number;
  feed_title: string;
  fetch_at: string | null;
  result: string;
  http_status: number | null;
  error_message: string | null;
  new_article_count: number;
  duration_ms: number | null;
}
export interface ReadingContext {
  ids: number[];
  returnTo: string;
}
