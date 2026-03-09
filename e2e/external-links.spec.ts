import { expect, test } from './fixtures';

test.describe('external links', () => {
  test('Docs/Support open in a new tab and do not navigate the app', async ({
    page,
  }) => {
    await page.goto('/tasks');

    await page.getByRole('button', { name: 'Main navigation' }).click();
    const menu = page.getByRole('menu');

    const docsLink = menu.getByRole('menuitem', { name: 'Docs' });
    await expect(docsLink).toHaveAttribute('target', '_blank');
    const [docsPopup] = await Promise.all([
      page.waitForEvent('popup'),
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
      page.waitForEvent('popup'),
      supportLink.click(),
    ]);
    await expect(supportPopup).toHaveURL(/github\.com\/BloopAI\/vibe-kanban\/issues/);
    await supportPopup.close();

    await expect(page).toHaveURL(/\/tasks/);
  });
});
