import { expect, test } from './fixtures';

test.describe('settings', () => {
  test('deep links work; language switches without refresh and persists across reload', async ({
    page,
  }) => {
    await page.goto('/settings/mcp');
    await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible();
    await expect(page.getByRole('link', { name: 'MCP Servers' })).toBeVisible();

    await page.goto('/settings/general');
    await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible();

    await page.getByLabel('Language').click();
    await page.getByRole('option', { name: '简体中文' }).click();

    await page.getByRole('button', { name: 'Save Settings' }).click();
    await expect(page.getByRole('heading', { name: '设置' })).toBeVisible();

    await page.reload();
    await expect(page.getByRole('heading', { name: '设置' })).toBeVisible();

    // Reset to English so the rest of the suite stays deterministic.
    await page.getByLabel('语言').click();
    await page.getByRole('option', { name: 'English' }).click();
    await page.getByRole('button', { name: '保存设置' }).click();
    await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible();
  });
});

