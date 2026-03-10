import { expect, test } from './fixtures';

test.describe('external links', () => {
  test('Docs/Support open in a new tab and do not navigate the app', async ({
    page,
  }) => {
    await page.goto('/tasks');
    const context = page.context();

    // Some dev/CI environments do not allow outbound network access to these
    // domains. Stub them so we can still validate "opens in new tab" semantics.
    const stubExternal = async () => {
      await context.route('https://vibekanban.com/**', async (route) => {
        await route.fulfill({
          status: 200,
          contentType: 'text/html',
          body: '<html><body>ok</body></html>',
        });
      });
      await context.route('https://www.vibekanban.com/**', async (route) => {
        await route.fulfill({
          status: 200,
          contentType: 'text/html',
          body: '<html><body>ok</body></html>',
        });
      });
      await context.route('https://github.com/**', async (route) => {
        await route.fulfill({
          status: 200,
          contentType: 'text/html',
          body: '<html><body>ok</body></html>',
        });
      });
    };
    await stubExternal();

    await page.getByRole('button', { name: 'Main navigation' }).click();
    const menu = page.getByRole('menu');

    const docsLink = menu.getByRole('menuitem', { name: 'Docs' });
    await expect(docsLink).toHaveAttribute('target', '_blank');
    const [docsPopup] = await Promise.all([
      context.waitForEvent('page'),
      docsLink.click(),
    ]);
    await expect(docsPopup).toHaveURL(/vibekanban\.com\/docs/);
    await docsPopup.close();

    await expect(page).toHaveURL(/\/tasks/);

    await page.getByRole('button', { name: 'Main navigation' }).click();
    const supportLink = page
      .getByRole('menu')
      .getByRole('menuitem', { name: 'Support' });
    await expect(supportLink).toHaveAttribute('target', '_blank');
    const [supportPopup] = await Promise.all([
      context.waitForEvent('page'),
      supportLink.click(),
    ]);
    await expect(supportPopup).toHaveURL(/github\.com\/BloopAI\/vibe-kanban\/issues/);
    await supportPopup.close();

    await expect(page).toHaveURL(/\/tasks/);
  });
});
