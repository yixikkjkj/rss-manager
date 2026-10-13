import { expect, it, vi } from 'vitest';
import { followLink, navigate, pageNumber, readingContext, subscribeRoute, updateQuery } from './Router';

it('preserves query parameters and reading state through navigation', () => {
  navigate('/search?q=hello&feed=2&page=3');
  updateQuery('sort', 'newest');
  expect(location.search).toBe('?q=hello&feed=2&sort=newest');
  const context = { ids: [1, 2], returnTo: '/search?q=hello&page=3' };
  navigate('/article/1', { state: context });
  expect(readingContext()).toEqual(context);
  navigate(context.returnTo);
  expect(location.search).toBe('?q=hello&page=3');
  expect(readingContext()).toBeNull();
});
it('notifies for push/replace and popstate and unsubscribes', () => {
  const listener = vi.fn();
  const off = subscribeRoute(listener);
  navigate('/feeds');
  navigate('/settings', { replace: true });
  window.dispatchEvent(new PopStateEvent('popstate'));
  expect(listener).toHaveBeenCalledTimes(3);
  off();
  window.dispatchEvent(new PopStateEvent('popstate'));
  expect(listener).toHaveBeenCalledTimes(3);
});
it('intercepts only normal same-origin link clicks', () => {
  const anchor = document.createElement('a');
  anchor.href = '/search';
  document.body.append(anchor);
  anchor.addEventListener('click', (e) => {
    followLink(e);
    if (!e.defaultPrevented) e.preventDefault();
  });
  anchor.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, ctrlKey: true }));
  expect(location.pathname).toBe('/');
  anchor.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
  expect(location.pathname).toBe('/search');
});
it('rejects invalid page numbers and external return locations', () => {
  expect(pageNumber(new URLSearchParams('page=Infinity'))).toBe(1);
  expect(pageNumber(new URLSearchParams('page=2.5'))).toBe(1);
  history.replaceState({ ids: [1], returnTo: 'https://example.com' }, '', '/');
  expect(readingContext()).toBeNull();
});
