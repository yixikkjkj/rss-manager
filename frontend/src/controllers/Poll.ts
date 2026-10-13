import type { ReactiveController, ReactiveControllerHost } from 'lit';

/** Schedules the next poll only after the previous request completes. */
export class Poll implements ReactiveController {
  private timer?: ReturnType<typeof setTimeout>;
  private revision = 0;
  private connected = false;
  constructor(
    host: ReactiveControllerHost,
    private interval: number,
    private work: () => Promise<boolean | void>,
    private immediate = false,
  ) {
    host.addController(this);
  }
  hostConnected() {
    this.connected = true;
    this.start(this.immediate);
  }
  hostDisconnected() {
    this.connected = false;
    this.stop();
  }
  stop() {
    this.revision++;
    clearTimeout(this.timer);
  }
  start(immediate = true) {
    this.stop();
    if (!this.connected) return;
    const revision = this.revision;
    const tick = async () => {
      if (!this.connected || revision !== this.revision) return;
      const again = await this.work();
      if (again !== false && this.connected && revision === this.revision)
        this.timer = setTimeout(() => void tick(), this.interval);
    };
    this.timer = setTimeout(() => void tick(), immediate ? 0 : this.interval);
  }
}
