import { expect, test } from './fixtures';
import fs from 'node:fs/promises';
import path from 'node:path';
import { getConfigDir } from './helpers/seed';

test.describe('settings', () => {
  test('deep links work; language updates via config reload', async ({ page }) => {
    const configPath = path.join(getConfigDir(), 'config.yaml');

    await page.goto('/settings/mcp');
    await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible();
    await expect(page.getByRole('link', { name: 'MCP Servers' })).toBeVisible();

    await page.goto('/settings/general');
    await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible();

    const raw = await fs.readFile(configPath, 'utf8');
    const current = raw.trim() ? JSON.parse(raw) : {};
    await fs.writeFile(
      configPath,
      `${JSON.stringify({ ...current, language: 'ZH_HANS' }, null, 2)}\n`
    );

    await page.getByRole('button', { name: 'Reload' }).click();
    await expect(page.getByRole('heading', { name: '设置' })).toBeVisible();

    await page.reload();
    await expect(page.getByRole('heading', { name: '设置' })).toBeVisible();

    // Reset to English so the rest of the suite stays deterministic.
    const rawReset = await fs.readFile(configPath, 'utf8');
    const currentReset = rawReset.trim() ? JSON.parse(rawReset) : {};
    await fs.writeFile(
      configPath,
      `${JSON.stringify({ ...currentReset, language: 'EN' }, null, 2)}\n`
    );
    await page.request.post('/api/config/reload', { data: {} });
    await page.reload();

    await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible();
  });
});
