import type { ReactiveController, ReactiveControllerHost } from 'lit';
import { messageOf } from './Resource';

export class Action implements ReactiveController {
  busy = false;
  error = '';
  private revision = 0;
  constructor(private host: ReactiveControllerHost & { isConnected: boolean }) {
    host.addController(this);
  }
  hostDisconnected() {
    this.revision++;
    this.busy = false;
  }
  run = async (work: (current: () => boolean) => Promise<void>): Promise<boolean> => {
    if (this.busy || !this.host.isConnected) return false;
    const revision = ++this.revision;
    const current = () => this.host.isConnected && revision === this.revision;
    this.busy = true;
    this.error = '';
    this.host.requestUpdate();
    try {
      await work(current);
      return current();
    } catch (error) {
      if (current()) this.error = messageOf(error);
      return false;
    } finally {
      if (current()) {
        this.busy = false;
        this.host.requestUpdate();
      }
    }
  };
}
