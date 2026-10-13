import type { User } from './Types';

let user: User | null = null;
const listeners = new Set<() => void>();
export const auth = {
  get user() {
    return user;
  },
  setUser(value: User | null) {
    if (value === user) return;
    user = value;
    listeners.forEach((listener) => listener());
  },
  clear() {
    auth.setUser(null);
  },
  subscribe(listener: () => void) {
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  },
};
