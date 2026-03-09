import { expect, test } from './fixtures';
import { createRepoAndProject, createTask } from './helpers/setup';

test.describe('tasks overview', () => {
  test('grouping/collapse/filter/inbox + reload consistency', async ({
    page,
    makeName,
    reposDir,
  }) => {
    const projectName = makeName('project');
    const repoFolderName = makeName('repo');
    const { project } = await createRepoAndProject(page.request, {
      name: projectName,
      reposDir,
      repoFolderName,
    });

    const todoTitle = makeName('todo');
    const reviewTitle = makeName('review');
    const doneTitle = makeName('done');
    await createTask(page.request, {
      projectId: project.id,
      title: todoTitle,
      status: 'todo',
    });
    const reviewTask = await createTask(page.request, {
      projectId: project.id,
      title: reviewTitle,
      status: 'inreview',
    });
    await createTask(page.request, {
      projectId: project.id,
      title: doneTitle,
      status: 'done',
    });

    await page.goto('/tasks');

    await expect(page.getByRole('heading', { name: 'All Tasks' })).toBeVisible();

    const projectSection = page
      .locator('section')
      .filter({ has: page.getByRole('button', { name: projectName }) })
      .first();
    await expect(projectSection).toBeVisible();

    // "To Do" is folded by default, so expand it for visibility assertions.
    await projectSection.getByRole('button', { name: 'Expand To Do' }).click();

    await expect(projectSection.getByText(todoTitle)).toBeVisible();
    await expect(projectSection.getByText(reviewTitle)).toBeVisible();

    // Collapse "In Review" globally via the fold-statuses menu (persists via localStorage)
    await page.getByRole('button', { name: 'Auto-fold statuses' }).click();
    const inReviewCheckbox = page.getByRole('menuitemcheckbox', {
      name: 'In Review',
    });
    await inReviewCheckbox.click();
    await expect(inReviewCheckbox).toHaveAttribute('aria-checked', 'true');
    await page.keyboard.press('Escape');

    await expect(projectSection.getByText(reviewTitle)).toHaveCount(0);

    await page.reload();

    const projectSectionAfterReload = page
      .locator('section')
      .filter({ has: page.getByRole('button', { name: projectName }) })
      .first();
    await expect(
      projectSectionAfterReload.getByRole('button', { name: 'Expand In Review' })
    ).toHaveAttribute('aria-expanded', 'false');

    await projectSectionAfterReload
      .getByRole('button', { name: 'Expand To Do' })
      .click();

    // Search filters narrow visible tasks without navigation.
    await page.locator('input[name="projectSearch"]').fill(todoTitle);
    await expect(projectSectionAfterReload.getByText(todoTitle)).toBeVisible();
    await expect(projectSectionAfterReload.getByText(doneTitle)).toHaveCount(0);

    await page.locator('input[name="projectSearch"]').fill('');

    // Review inbox opens and routes to the selected task without leaving the app.
    await page.getByRole('button', { name: 'Open inbox' }).click();
    await expect(page.getByText('Review inbox')).toBeVisible();
    await page.getByRole('menuitem', { name: new RegExp(reviewTitle) }).click();
    await expect(page).toHaveURL(
      new RegExp(`/tasks/${reviewTask.project_id}/${reviewTask.id}/attempts/latest`)
    );
  });
});
