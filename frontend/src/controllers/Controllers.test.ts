import { html } from 'lit';
import { expect, it, vi } from 'vitest';
import { AppElement } from '../core/AppElement';
import { Resource } from './Resource';
import { Action } from './Action';
import { Poll } from './Poll';

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
class Host extends AppElement {
  render() {
    return html`<p>test</p>`;
  }
}
customElements.define('test-controller-host', Host);
it('ignores obsolete data after filter changes and unmount', async () => {
  const host = new Host();
  let filter = 1;
  const first = deferred<string>(),
    second = deferred<string>(),
    third = deferred<string>();
  const load = vi
    .fn()
    .mockReturnValueOnce(first.promise)
    .mockReturnValueOnce(second.promise)
    .mockReturnValueOnce(third.promise);
  const resource = new Resource<string>(host, load, () => [filter]);
  document.body.append(host);
  await host.updateComplete;
  filter = 2;
  host.requestUpdate();
  await host.updateComplete;
  second.resolve('new');
  await second.promise;
  await host.updateComplete;
  first.resolve('old');
  await first.promise;
  expect(resource.data).toBe('new');
  const refresh = resource.refresh();
  host.remove();
  third.resolve('detached');
  await refresh;
  expect(resource.data).toBe('new');
});
it('blocks duplicate actions and invalidates completion after reconnect', async () => {
  const host = new Host();
  const action = new Action(host);
  document.body.append(host);
  const request = deferred<void>();
  let applied = false;
  const pending = action.run(async (current) => {
    await request.promise;
    if (current()) applied = true;
  });
  const duplicate = vi.fn();
  await action.run(duplicate);
  expect(duplicate).not.toHaveBeenCalled();
  host.remove();
  document.body.append(host);
  request.resolve();
  expect(await pending).toBe(false);
  expect(applied).toBe(false);
  expect(action.busy).toBe(false);
});
it('does not overlap polls or reschedule after disconnect', async () => {
  vi.useFakeTimers();
  const host = new Host();
  const request = deferred<void>();
  const work = vi.fn(() => request.promise);
  new Poll(host, 1000, work);
  document.body.append(host);
  await vi.advanceTimersByTimeAsync(5000);
  expect(work).toHaveBeenCalledTimes(1);
  host.remove();
  request.resolve();
  await vi.advanceTimersByTimeAsync(5000);
  expect(work).toHaveBeenCalledTimes(1);
});
