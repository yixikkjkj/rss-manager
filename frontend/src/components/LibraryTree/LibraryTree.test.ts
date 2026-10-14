import { expect, it } from 'vitest';
import { LibraryTree } from './LibraryTree';
import type { FeedVO } from '../../Types';

const feed: FeedVO = {
  id: 1,
  folder_id: null,
  title: 'Example',
  feed_title: 'Example',
  xml_url: 'https://feeds.example.com/rss',
  html_url: 'https://example.com/blog/',
  icon_url: 'https://cdn.example.com/assets/logo.svg',
  health_status: 'healthy',
  consecutive_failures: 0,
  last_fetch_at: null,
  use_readability: false,
  unread_count: 1,
  fetch_interval_sec: null,
};

it('falls back from the discovered icon to the website favicon then the default icon', async () => {
  const tree = new LibraryTree();
  tree.feeds = [feed];
  document.body.append(tree);
  await tree.updateComplete;
  expect(tree.querySelector('img')).toHaveAttribute('src', feed.icon_url);
  tree.querySelector('img')!.dispatchEvent(new Event('error'));
  await tree.updateComplete;
  expect(tree.querySelector('img')).toHaveAttribute('src', 'https://example.com/favicon.ico');
  tree.querySelector('img')!.dispatchEvent(new Event('error'));
  await tree.updateComplete;
  expect(tree.querySelector('img')).toBeNull();
  expect(tree.querySelector('[data-key="feed-1"] svg')).not.toBeNull();
  tree.feeds = [{ ...feed, unread_count: 0 }];
  await tree.updateComplete;
  expect(tree.querySelector('img')).toBeNull();
  tree.feeds = [{ ...feed, icon_url: 'https://cdn.example.com/new.png' }];
  await tree.updateComplete;
  expect(tree.querySelector('img')).toHaveAttribute('src', 'https://cdn.example.com/new.png');
});

it('ignores unsafe icon addresses and supports subscriptions without website metadata', async () => {
  const tree = new LibraryTree();
  tree.feeds = [{ ...feed, html_url: null, icon_url: 'javascript:alert(1)' }];
  document.body.append(tree);
  await tree.updateComplete;
  expect(tree.querySelector('img')).toHaveAttribute('src', 'https://feeds.example.com/favicon.ico');
});
