import type { ReactiveController, ReactiveControllerHost } from 'lit';

export const messageOf = (error: unknown) =>
  error instanceof Error ? error.message : '操作失败，请稍后重试';
type Host = ReactiveControllerHost & { isConnected: boolean };

/** Latest request wins, including across disconnect/reconnect. Dependencies are explicit. */
export class Resource<T> implements ReactiveController {
  data?: T;
  error = '';
  loading = true;
  private revision = 0;
  private dependencies?: readonly unknown[];
  private connected = false;
  constructor(
    private host: Host,
    private load: (current: () => boolean) => Promise<T>,
    private args: () => readonly unknown[] = () => [],
  ) {
    host.addController(this);
  }
  hostConnected() {
    this.connected = true;
    this.dependencies = undefined;
    this.host.requestUpdate();
  }
  hostDisconnected() {
    this.connected = false;
    this.revision++;
  }
  hostUpdate() {
    if (!this.connected) return;
    const next = this.args();
    if (
      !this.dependencies ||
      next.length !== this.dependencies.length ||
      next.some((value, i) => !Object.is(value, this.dependencies![i]))
    ) {
      this.dependencies = [...next];
      void this.refresh();
    }
  }
  refresh = async () => {
    if (!this.connected) return;
    const revision = ++this.revision;
    const current = () => this.connected && this.host.isConnected && revision === this.revision;
    this.loading = true;
    this.error = '';
    this.host.requestUpdate();
    try {
      const value = await this.load(current);
      if (current()) this.data = value;
    } catch (error) {
      if (current()) this.error = messageOf(error);
    } finally {
      if (current()) {
        this.loading = false;
        this.host.requestUpdate();
      }
    }
  };
}
