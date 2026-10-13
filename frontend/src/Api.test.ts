import { afterEach, describe, expect, it, vi } from 'vitest';
import { api, ApiError } from './Api';
import { auth } from './Store';
afterEach(() => {
  vi.unstubAllGlobals();
  auth.clear();
});
describe('session handling', () => {
  it('does not navigate when auth_me returns 401, even on repeated login-page checks', async () => {
    const before = window.location.href;
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue({
        status: 401,
        ok: false,
        json: async () => ({ code: 'C401001', msg: '请重新登录' }),
      }),
    );
    for (let i = 0; i < 3; i++) await expect(api.authMe()).rejects.toBeInstanceOf(ApiError);
    expect(window.location.href).toBe(before);
  });
  it('preserves the credential error and clears an expired authenticated session', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue({
        status: 401,
        ok: false,
        json: async () => ({ code: 'A700001', msg: '用户名或密码错误' }),
      }),
    );
    await expect(api.authLogin('admin', 'wrong')).rejects.toThrow('用户名或密码错误');
    auth.setUser({ user_id: 1, username: 'admin' });
    await expect(api.articleGet(1)).rejects.toThrow();
    expect(auth.user).toBeNull();
  });
  it('handles a non-JSON gateway error', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue({
        status: 502,
        ok: false,
        json: async () => {
          throw new Error('html');
        },
      }),
    );
    await expect(api.authMe()).rejects.toThrow('HTTP 502');
  });
});
