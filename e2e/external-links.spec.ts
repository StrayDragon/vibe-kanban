import { expect, test } from './fixtures';

test.describe('external links', () => {
  test('Support opens in a new tab and does not navigate the app', async ({
    page,
  }) => {
    await page.goto('/tasks');
    const context = page.context();

    // Some dev/CI environments do not allow outbound network access to external
    // domains. Stub them so we can still validate "opens in new tab" semantics.
    const stubExternal = async () => {
      await context.route(
        'https://github.com/BloopAI/vibe-kanban/issues**',
        async (route) => {
          await route.fulfill({
            status: 200,
            contentType: 'text/html',
            body: '<html><body>ok</body></html>',
          });
        }
      );
    };
    await stubExternal();

    await page.getByRole('button', { name: 'Main navigation' }).click();
    const menu = page.getByRole('menu');

    await expect(menu.getByRole('menuitem', { name: 'Docs' })).toHaveCount(0);

    const supportLink = menu.getByRole('menuitem', { name: 'Support' });
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
