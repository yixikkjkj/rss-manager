import { test, expect, type Page } from '@playwright/test';

for (const path of ['/', '/login']) {
  test(`unauthenticated ${path} settles on login without reloading`, async ({ page }) => {
    let documents = 0;
    let authChecks = 0;
    page.on('request', (request) => {
      if (request.resourceType() === 'document') documents++;
      if (request.url().endsWith('/api/rpc/auth_me')) authChecks++;
    });
    await page.goto(path);
    const password = page.getByLabel('密码', { exact: true });
    await password.fill('keep-this-input');
    // Observe an idle interval: the old 401 loop reloads every ~100 ms.
    await page.waitForTimeout(1000);
    await expect(page).toHaveURL(/\/login$/);
    await expect(password).toHaveValue('keep-this-input');
    expect(documents).toBe(1);
    expect(authChecks).toBe(1);
  });
}

async function login(page: Page) {
  await page.goto('/');
  await page.getByLabel('密码', { exact: true }).fill('E2e-Test-Password-123!');
  await page.getByRole('button', { name: '登录', exact: true }).click();
  await expect(page.locator('rss-reader')).toBeVisible();
  await expect(page.getByRole('list', { name: '文章列表', includeHidden: true })).toBeAttached();
}
test('login is stable, errors are visible, and logout returns to a usable login form', async ({ page }) => {
  let authChecks = 0;
  page.on('request', (r) => {
    if (r.url().endsWith('/auth_me')) authChecks++;
  });
  await page.goto('/login');
  await page.getByLabel('密码', { exact: true }).fill('wrong-password');
  await page.getByRole('button', { name: '登录', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('用户名或密码错误');
  expect(authChecks).toBeLessThanOrEqual(2);
  await page.getByLabel('密码', { exact: true }).fill('E2e-Test-Password-123!');
  await page.getByRole('button', { name: '登录', exact: true }).click();
  await page.getByRole('button', { name: '退出', exact: true }).click();
  await expect(page.getByRole('button', { name: '登录', exact: true })).toBeVisible();
});
test('search paginates and article reading sanitizes legacy HTML and supports keyboard navigation', async ({
  page,
}) => {
  await login(page);
  await page.getByRole('link', { name: '搜索', exact: true }).click();
  await page.getByLabel('关键词').fill('Searchneedle');
  await page.getByRole('button', { name: '搜索', exact: true }).click();
  await expect(page.getByRole('list', { name: '文章列表' }).getByRole('listitem')).toHaveCount(20);
  await page.getByRole('button', { name: '下一页', exact: true }).click();
  await expect(page.getByRole('list', { name: '文章列表' }).getByRole('listitem')).toHaveCount(6);
  await page.getByRole('list', { name: '文章列表' }).getByRole('link').first().click();
  await expect(page.getByTestId('article-content')).toContainText('Readable body needle.');
  await expect(page.getByTestId('article-content').locator('[onerror], script')).toHaveCount(0);
  expect(await page.evaluate(() => '__rssInjected' in window)).toBe(false);
  await expect(page.getByText('未能提取全文', { exact: false })).toBeVisible();
  const heading = await page.locator('rss-article-detail h1').textContent();
  await page.locator('rss-article-detail h1').click();
  await page.keyboard.press('j');
  await expect(page.locator('rss-article-detail h1')).not.toHaveText(heading!);
  await expect(page.getByRole('button', { name: '标记未读 (m)' })).toBeEnabled();
  await page.keyboard.press('m');
  await expect(page.getByRole('button', { name: '标记已读 (m)' })).toBeVisible();
  await page.getByRole('link', { name: '返回列表' }).click();
  await expect(page.getByRole('list', { name: '文章列表' }).getByRole('listitem')).toHaveCount(6);
});
async function createFolder(page: Page, name: string) {
  await page.getByRole('button', { name: '新建文件夹', exact: true }).click();
  await page.getByLabel('文件夹名称', { exact: true }).fill(name);
  await page.getByRole('button', { name: '创建文件夹', exact: true }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(page.getByRole('treeitem', { name, exact: true })).toBeVisible();
}
async function folderMenu(page: Page, name: string) {
  await page.getByLabel(`管理文件夹 ${name}`, { exact: true }).click();
}
test('nested folders manage favorites, move safely, rename and delete from the tree', async ({ page }) => {
  await login(page);
  await createFolder(page, 'Browser saved');
  await folderMenu(page, 'Browser saved');
  await page.getByRole('button', { name: '新建子文件夹' }).click();
  await page.getByLabel('文件夹名称', { exact: true }).fill('Child saved');
  await page.getByRole('button', { name: '创建文件夹', exact: true }).click();
  await expect(page.getByRole('treeitem', { name: 'Child saved', exact: true })).toBeVisible();
  await page.getByRole('list', { name: '文章列表' }).getByRole('link').first().click();
  await page.locator('rss-article-detail summary').click();
  await page.getByLabel('Browser saved / Child saved', { exact: true }).check();
  await expect(page.getByLabel('Browser saved / Child saved', { exact: true })).toBeEnabled();
  await page.getByRole('link', { name: 'Browser saved', exact: true }).click();
  await expect(page.getByRole('list', { name: '文章列表' }).getByRole('listitem')).toHaveCount(1);
  await folderMenu(page, 'Browser saved');
  await page.getByRole('button', { name: '移动文件夹', exact: true }).click();
  await expect(page.getByLabel('目标文件夹').locator('option')).not.toContainText(['Browser saved']);
  await page.getByRole('button', { name: '关闭', exact: true }).click();
  await folderMenu(page, 'Child saved');
  await page.getByRole('button', { name: '移动文件夹', exact: true }).click();
  await page.getByLabel('目标文件夹').selectOption('');
  await page.getByRole('button', { name: '保存', exact: true }).click();
  await expect(page.getByRole('list', { name: '文章列表' }).getByRole('listitem')).toHaveCount(0);
  await folderMenu(page, 'Child saved');
  await page.getByRole('button', { name: '重命名', exact: true }).click();
  await page.getByLabel('文件夹名称', { exact: true }).fill('Renamed saved');
  await page.getByRole('button', { name: '保存', exact: true }).click();
  await expect(page.getByRole('treeitem', { name: 'Renamed saved', exact: true })).toBeVisible();
  await page.getByRole('link', { name: 'Renamed saved', exact: true }).click();
  await page.getByRole('list', { name: '文章列表' }).getByRole('link').first().click();
  await page.locator('rss-article-detail summary').click();
  await page.getByRole('checkbox', { name: 'Renamed saved', exact: true }).uncheck();
  await expect(page.getByRole('checkbox', { name: 'Renamed saved', exact: true })).toBeEnabled();
  await expect(page.getByRole('list', { name: '文章列表' }).getByRole('listitem')).toHaveCount(0);
  for (const name of ['Renamed saved', 'Browser saved']) {
    await folderMenu(page, name);
    page.once('dialog', (dialog) => dialog.accept());
    await page.getByRole('button', { name: '删除文件夹', exact: true }).click();
    await expect(page.getByRole('treeitem', { name, exact: true })).toHaveCount(0);
  }
});
test('edit and move a subscription in the tree and persist fetch settings', async ({ page }) => {
  await login(page);
  await createFolder(page, 'Moved sources');
  await page.getByRole('button', { name: '展开 Fixture group', exact: true }).click();
  await page.getByLabel('管理订阅源 Fixture source', { exact: true }).click();
  await page.getByRole('button', { name: '编辑订阅源', exact: true }).click();
  await page.getByLabel('显示名称').fill('Edited fixture');
  await page.getByLabel('所属文件夹').selectOption({ label: 'Moved sources' });
  await page.getByLabel('抓取间隔（分钟）').fill('30');
  await page.getByLabel('打开文章时自动提取全文').uncheck();
  await page.getByRole('button', { name: '保存订阅' }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await page.getByRole('link', { name: 'Edited fixture', exact: true }).click();
  await expect(page.getByRole('list', { name: '文章列表' }).getByRole('listitem')).toHaveCount(26);
  await page.getByLabel('管理订阅源 Edited fixture', { exact: true }).click();
  await page.getByRole('button', { name: '编辑订阅源', exact: true }).click();
  await expect(page.getByLabel('所属文件夹').locator('option:checked')).toHaveText('Moved sources');
  await expect(page.getByLabel('抓取间隔（分钟）')).toHaveValue('30');
  await page.getByLabel('所属文件夹').selectOption('');
  await page.getByRole('button', { name: '保存订阅' }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await folderMenu(page, 'Moved sources');
  page.once('dialog', (dialog) => dialog.accept());
  await page.getByRole('button', { name: '删除文件夹', exact: true }).click();
  await expect(page.getByRole('treeitem', { name: 'Moved sources', exact: true })).toHaveCount(0);
  await page.getByRole('link', { name: '设置', exact: true }).click();
  await page.getByLabel('间隔（分钟）', { exact: true }).fill('120');
  await page.getByRole('button', { name: '保存间隔' }).click();
  await expect(page.getByText('已保存', { exact: true })).toBeVisible();
  await page.reload();
  await expect(page.getByLabel('间隔（分钟）', { exact: true })).toHaveValue('120');
});
test('mobile reading stays within viewport', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await login(page);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.getByRole('link', { name: '▤ 全部文章', exact: true }).click();
  await page.getByRole('button', { name: '文章列表', exact: true }).click();
  await page.getByRole('list', { name: '文章列表' }).getByRole('link').first().click();
  await expect(page.getByTestId('article-content')).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: test.info().outputPath('reader-mobile.png') });
});

test('history restores filters, pagination and reading context, including a direct detail reload', async ({
  page,
}) => {
  await login(page);
  await page.getByRole('link', { name: '搜索', exact: true }).click();
  await page.getByLabel('关键词').fill('Searchneedle');
  await page.getByRole('button', { name: '搜索', exact: true }).click();
  await page.getByRole('button', { name: '下一页', exact: true }).click();
  const list = page.getByRole('list', { name: '文章列表' });
  await expect(list.getByRole('listitem')).toHaveCount(6);
  await list.getByRole('link').first().click();
  await expect(page.getByTestId('article-content')).toBeVisible();
  const first = await page.locator('rss-article-detail h1').textContent();
  await page.getByRole('button', { name: '下一篇 (j)' }).click();
  await expect(page.locator('rss-article-detail h1')).not.toHaveText(first!);
  await page.goBack();
  await expect(page.locator('rss-article-detail h1')).toHaveText(first!);
  await page.goBack();
  await expect(page.getByLabel('关键词')).toHaveValue('Searchneedle');
  await expect(list.getByRole('listitem')).toHaveCount(6);
  await page.goForward();
  await expect(page.locator('rss-article-detail h1')).toHaveText(first!);
  await page.reload();
  await expect(page.locator('rss-article-detail h1')).toHaveText(first!);
  await page.getByRole('link', { name: '返回列表' }).click();
  await expect(list.getByRole('listitem')).toHaveCount(6);
  await page.goto('/article/1');
  await expect(page.getByTestId('article-content')).toBeVisible();
  await expect(page.getByRole('button', { name: '上一篇 (k)' })).toBeEnabled();
});

test('production loads extracted module CSS and renders application components in light DOM', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await login(page);
  const list = page.getByRole('list', { name: '文章列表' });
  await expect(list).toHaveCSS('list-style-type', 'none');
  await expect(list).toHaveAttribute('class', /Reader-[a-f0-9]+-list/);
  await expect(page.locator('link[rel="stylesheet"][href*="/assets/"]').first()).toBeAttached();
  expect(await page.locator('rss-app').evaluate((element) => element.shadowRoot)).toBeNull();
  await list.getByRole('link').first().click();
  await expect(page.getByTestId('article-content')).toHaveCSS('overflow-wrap', 'anywhere');
  expect(errors).toEqual([]);
});

test('a late article response cannot replace the next article or mark the abandoned article read', async ({
  page,
}) => {
  await login(page);
  let release!: () => void;
  let started!: () => void;
  const waiting = new Promise<void>((resolve) => {
    started = resolve;
  });
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  const marked: number[] = [];
  page.on('request', (request) => {
    if (request.url().endsWith('/article_mark_read')) marked.push(request.postDataJSON().id);
  });
  await page.route('**/api/rpc/article_get', async (route) => {
    if (route.request().postDataJSON().id !== 26) return route.continue();
    const response = await route.fetch();
    started();
    await gate;
    await route.fulfill({ response });
  });
  await page.getByRole('list', { name: '文章列表' }).getByRole('link').first().click();
  await waiting;
  try {
    await page.getByRole('button', { name: '下一篇 (j)' }).click();
    await expect(page.locator('rss-article-detail h1')).toHaveText('Searchneedle article 25');
  } finally {
    release();
  }
  await page.waitForTimeout(200);
  await expect(page.locator('rss-article-detail h1')).toHaveText('Searchneedle article 25');
  expect(marked).not.toContain(26);
});

test('reader keeps panes mounted and list position while selecting articles', async ({ page }) => {
  await login(page);
  const reader = page.locator('rss-reader');
  await reader.evaluate((element) => {
    element.setAttribute('data-persisted', 'yes');
  });
  const list = page.locator('[data-list]');
  await list.evaluate((element) => {
    element.scrollTop = 300;
  });
  const scroll = await list.evaluate((element) => element.scrollTop);
  await page.getByRole('list', { name: '文章列表' }).getByRole('link').nth(4).click();
  await expect(page.getByTestId('article-content')).toBeVisible();
  await expect(reader).toHaveAttribute('data-persisted', 'yes');
  expect(await list.evaluate((element) => element.scrollTop)).toBe(scroll);
  await page.screenshot({ path: test.info().outputPath('reader-desktop.png') });
  const treeBox = await page.getByRole('complementary', { name: '资料库导航' }).boundingBox();
  const listBox = await page.getByRole('region', { name: '文章列表栏' }).boundingBox();
  const detailBox = await page.getByRole('region', { name: '文章正文栏' }).boundingBox();
  expect(treeBox!.x).toBeLessThan(listBox!.x);
  expect(listBox!.x).toBeLessThan(detailBox!.x);
  expect(detailBox!.width).toBeGreaterThan(listBox!.width);
  await page.getByRole('button', { name: '下一篇 (j)' }).click();
  await expect(reader).toHaveAttribute('data-persisted', 'yes');
  expect(await list.evaluate((element) => element.scrollTop)).toBe(scroll);
});

test('legacy folder and source management URLs resolve to the unified reader', async ({ page }) => {
  await login(page);
  await page.goto('/?group=1');
  await expect(page).toHaveURL(/folder=\d+/);
  await expect(page.getByRole('heading', { name: 'Fixture group', exact: true })).toBeVisible();
  await page.goto('/folders');
  await expect(page.locator('rss-reader')).toBeVisible();
  await expect(page).toHaveURL(/\/$/);
  await page.goto('/feeds');
  await expect(page.locator('rss-reader')).toBeVisible();
  await expect(page.getByRole('link', { name: '源管理', exact: true })).toHaveCount(0);
});
