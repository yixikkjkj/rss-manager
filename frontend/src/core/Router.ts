import type { ReadingContext } from '../Types';

const listeners = new Set<() => void>();
const notify = () => listeners.forEach((listener) => listener());
export function subscribeRoute(listener: () => void) {
  if (!listeners.size) window.addEventListener('popstate', notify);
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
    if (!listeners.size) window.removeEventListener('popstate', notify);
  };
}
export function navigate(to: string, options: { replace?: boolean; state?: ReadingContext | null } = {}) {
  const url = new URL(to, location.href);
  if (url.origin !== location.origin) return;
  history[options.replace ? 'replaceState' : 'pushState'](options.state ?? null, '', url);
  notify();
}
/** Only intercept ordinary clicks on explicitly marked application links. */
export function followLink(event: MouseEvent, state?: ReadingContext) {
  const anchor = event
    .composedPath()
    .find((node): node is HTMLAnchorElement => node instanceof HTMLAnchorElement);
  if (
    !anchor ||
    event.defaultPrevented ||
    event.button !== 0 ||
    event.metaKey ||
    event.ctrlKey ||
    event.shiftKey ||
    event.altKey ||
    anchor.hasAttribute('download') ||
    (anchor.target && anchor.target !== '_self')
  )
    return;
  const url = new URL(anchor.href);
  if (url.origin !== location.origin) return;
  event.preventDefault();
  navigate(url.href, { state });
}
export function updateQuery(key: string, value: string, clear: string[] = []) {
  const params = new URLSearchParams(location.search);
  if (value) params.set(key, value);
  else params.delete(key);
  if (key !== 'page') params.delete('page');
  clear.forEach((name) => params.delete(name));
  navigate(`${location.pathname}?${params}`);
}
export function pageNumber(params = new URLSearchParams(location.search)) {
  const value = Number(params.get('page'));
  return Number.isSafeInteger(value) && value > 0 ? value : 1;
}
export function readingContext(): ReadingContext | null {
  const value = history.state as Partial<ReadingContext> | null;
  if (
    !value ||
    !Array.isArray(value.ids) ||
    !value.ids.every((id) => Number.isSafeInteger(id) && id > 0) ||
    typeof value.returnTo !== 'string'
  )
    return null;
  try {
    const url = new URL(value.returnTo, location.href);
    if (url.origin !== location.origin) return null;
    return { ids: value.ids, returnTo: url.pathname + url.search };
  } catch {
    return null;
  }
}
