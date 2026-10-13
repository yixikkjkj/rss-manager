import { fireEvent, screen, waitFor } from '@testing-library/dom';
import { expect, it, vi } from 'vitest';
import { ImportExport } from './ImportExport';
import { api } from '../../Api';

const completed = {
  import_id: 1,
  parsed: 1,
  failed: 0,
  first_fetch_ok: 1,
  total_first_fetch: 1,
  completed: 1,
  normalized: [],
  failures: [],
};
it('imports only on submit and displays the completed report', async () => {
  const importSpy = vi.spyOn(api, 'opmlImport').mockResolvedValue(completed);
  vi.spyOn(api, 'opmlImportReport').mockResolvedValue(completed);
  const page = new ImportExport();
  document.body.append(page);
  await page.updateComplete;
  fireEvent.input(screen.getByLabelText('OPML 内容'), { target: { value: '<opml>edited</opml>' } });
  await page.updateComplete;
  expect(importSpy).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: '开始导入' }));
  await waitFor(() => expect(importSpy).toHaveBeenCalledTimes(1));
  expect(await screen.findByText(/首抓已完成 1 \/ 1/)).toBeInTheDocument();
});
it('stops polling when disconnected, including an in-flight response', async () => {
  vi.useFakeTimers();
  vi.spyOn(api, 'opmlImport').mockResolvedValue(completed);
  let resolve!: (value: typeof completed) => void;
  const report = vi.spyOn(api, 'opmlImportReport').mockImplementation(
    () =>
      new Promise((done) => {
        resolve = done;
      }),
  );
  const page = new ImportExport();
  document.body.append(page);
  await page.updateComplete;
  fireEvent.input(screen.getByLabelText('OPML 内容'), { target: { value: '<opml/>' } });
  await page.updateComplete;
  fireEvent.submit(page.querySelector('form')!);
  await vi.advanceTimersByTimeAsync(0);
  expect(report).toHaveBeenCalledTimes(1);
  page.remove();
  resolve({ ...completed, completed: 0 });
  await vi.advanceTimersByTimeAsync(10000);
  expect(report).toHaveBeenCalledTimes(1);
  expect(page.textContent).not.toContain('首抓已完成');
});
