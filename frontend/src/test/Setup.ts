import '@testing-library/jest-dom/vitest';
import { afterEach, vi } from 'vitest';
import { auth } from '../Store';
afterEach(() => {
  document.body.replaceChildren();
  auth.clear();
  history.replaceState(null, '', '/');
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});
