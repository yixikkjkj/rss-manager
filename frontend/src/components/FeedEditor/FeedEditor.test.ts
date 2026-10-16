import { expect, it, vi } from 'vitest';
import { fireEvent, within, waitFor } from '@testing-library/dom';
import { api } from '../../Api';
import { FeedEditor } from './FeedEditor';
import type { FeedVO } from '../../Types';

const feed: FeedVO = {
  id: 7,
  folder_id: null,
  title: 'Example',
  feed_title: 'Example',
  xml_url: 'https://example.com/feed',
  html_url: null,
  health_status: 'healthy',
  consecutive_failures: 0,
  last_fetch_at: null,
  use_readability: false,
  unread_count: 0,
  fetch_interval_sec: null,
};
const config = { headers: [{ name: 'User-Agent', value: 'Test Reader' }], cookie: 'session=example' };
const unifiedConfig = { headers: [...config.headers, { name: 'Cookie', value: config.cookie }], cookie: '' };
async function mount(edit = false) {
  const editor = new FeedEditor();
  if (edit) editor.feed = feed;
  document.body.append(editor);
  await editor.updateComplete;
  if (edit)
    await waitFor(() => expect(within(editor).getByRole('button', { name: '保存订阅' })).toBeEnabled());
  editor.querySelector('details')!.open = true;
  return editor;
}
function input(editor: FeedEditor, label: string, value: string) {
  const field = within(editor).getByLabelText(label);
  fireEvent.input(field, { target: { value } });
  return field;
}
it('tests unsaved configuration without saving and invalidates the result after editing', async () => {
  const create = vi.spyOn(api, 'feedCreate');
  const update = vi.spyOn(api, 'feedUpdate');
  const probe = vi.spyOn(api, 'feedProbe').mockResolvedValue({
    result: 'success',
    http_status: 200,
    error_message: null,
    feed_title: '<img src=x onerror=alert(1)>',
    item_count: 3,
    duration_ms: 12,
  });
  const editor = await mount();
  const ui = within(editor);
  input(editor, '订阅地址', 'https://example.com/feed');
  fireEvent.click(ui.getByRole('button', { name: '添加请求头' }));
  await editor.updateComplete;
  input(editor, '请求头 1', 'User-Agent');
  input(editor, '请求值 1', 'Test Reader');
  await editor.updateComplete;
  fireEvent.click(ui.getByRole('button', { name: '添加请求头' }));
  await editor.updateComplete;
  input(editor, '请求头 2', 'Cookie');
  input(editor, '请求值 2', 'session=example');
  await editor.updateComplete;
  expect(ui.getByLabelText('请求值 2')).toHaveAttribute('type', 'text');
  expect(editor.querySelector('input[type=password]')).toBeNull();
  fireEvent.click(ui.getByRole('button', { name: '测试当前配置' }));
  await waitFor(() => expect(ui.getByRole('status')).toHaveTextContent('解析到 3 篇文章'));
  expect(probe).toHaveBeenCalledWith('https://example.com/feed', unifiedConfig);
  expect(ui.getByRole('status')).toHaveTextContent('HTTP 200');
  expect(editor.querySelector('img')).toBeNull();
  expect(create).not.toHaveBeenCalled();
  expect(update).not.toHaveBeenCalled();
  input(editor, '请求值 2', 'session=changed');
  await editor.updateComplete;
  expect(ui.queryByText('抓取成功')).toBeNull();
});
it('loads, saves and explicitly clears existing credentials and clears them on origin changes', async () => {
  vi.spyOn(api, 'feedRequestConfig').mockResolvedValue(config);
  const update = vi.spyOn(api, 'feedUpdate').mockResolvedValue(feed);
  const editor = await mount(true);
  const ui = within(editor);
  expect(ui.getByLabelText('请求值 2')).toHaveValue('session=example');
  input(editor, '显示名称', 'Renamed');
  fireEvent.submit(editor.querySelector('form')!);
  await waitFor(() =>
    expect(update).toHaveBeenCalledWith(
      expect.objectContaining({ id: 7, title: 'Renamed', request_config: unifiedConfig }),
    ),
  );
  await waitFor(() => expect(ui.getByRole('button', { name: '保存订阅' })).toBeEnabled());
  fireEvent.click(ui.getByRole('button', { name: '清空请求配置' }));
  await editor.updateComplete;
  expect(ui.queryByLabelText('请求头 1')).toBeNull();
  fireEvent.submit(editor.querySelector('form')!);
  await waitFor(() =>
    expect(update).toHaveBeenLastCalledWith(
      expect.objectContaining({ request_config: { headers: [], cookie: '' } }),
    ),
  );
  await waitFor(() => expect(ui.getByRole('button', { name: '保存订阅' })).toBeEnabled());
  fireEvent.click(ui.getByRole('button', { name: '添加请求头' }));
  await editor.updateComplete;
  input(editor, '请求头 1', 'Cookie');
  input(editor, '请求值 1', 'session=new');
  await editor.updateComplete;
  fireEvent.change(input(editor, '订阅地址', 'https://other.example/feed'));
  await editor.updateComplete;
  expect(ui.queryByLabelText('请求头 1')).toBeNull();
  expect(ui.getByRole('status')).toHaveTextContent('站点已变化');
});
it('keeps save disabled after a settings load failure and permits retry or cancel', async () => {
  const load = vi
    .spyOn(api, 'feedRequestConfig')
    .mockRejectedValueOnce(new Error('加载失败'))
    .mockResolvedValue(config);
  const update = vi.spyOn(api, 'feedUpdate');
  const editor = new FeedEditor();
  editor.feed = feed;
  document.body.append(editor);
  const ui = within(editor);
  await waitFor(() => expect(ui.getByRole('alert')).toHaveTextContent('加载失败'));
  expect(ui.getByRole('button', { name: '保存订阅' })).toBeDisabled();
  expect(ui.getByRole('button', { name: '取消' })).toBeEnabled();
  fireEvent.submit(editor.querySelector('form')!);
  expect(update).not.toHaveBeenCalled();
  fireEvent.click(ui.getByRole('button', { name: '重新加载请求配置' }));
  await waitFor(() => expect(ui.getByRole('button', { name: '保存订阅' })).toBeEnabled());
  expect(load).toHaveBeenCalledTimes(2);
  expect(ui.getByLabelText('请求值 2')).toHaveValue('session=example');
});
it('renders the failure diagnosis and ignores a result after the editor closes', async () => {
  const probe = vi.spyOn(api, 'feedProbe').mockResolvedValueOnce({
    result: 'challenge_required',
    http_status: 403,
    error_message: '疑似源站人机验证或防护页面',
    feed_title: null,
    item_count: 0,
    duration_ms: 10,
  });
  const editor = await mount();
  const ui = within(editor);
  input(editor, '订阅地址', 'https://example.com/feed');
  fireEvent.click(ui.getByRole('button', { name: '测试当前配置' }));
  await waitFor(() => expect(ui.getByRole('status')).toHaveTextContent('疑似人机验证'));
  expect(ui.getByRole('status')).toHaveTextContent('HTTP 403');
  let release!: (value: Awaited<ReturnType<typeof api.feedProbe>>) => void;
  probe.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        release = resolve;
      }),
  );
  fireEvent.click(ui.getByRole('button', { name: '测试当前配置' }));
  await editor.updateComplete;
  expect(ui.getByRole('button', { name: '保存订阅' })).toBeDisabled();
  editor.remove();
  release({
    result: 'success',
    http_status: 200,
    error_message: null,
    feed_title: 'Late',
    item_count: 2,
    duration_ms: 12,
  });
  await Promise.resolve();
  await editor.updateComplete;
  expect(editor.textContent).not.toContain('Late');
});
