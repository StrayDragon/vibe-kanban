import { expect, test } from './fixtures';
import { createRepoAndProject } from './helpers/setup';

test.describe('lazy routes smoke', () => {
  test('tasks/archives/settings routes render and dialogs open', async ({
    page,
    reposDir,
    makeName,
  }) => {
    const projectName = makeName('project');
    const repoFolderName = makeName('repo');
    const { project } = await createRepoAndProject(page.request, {
      name: projectName,
      reposDir,
      repoFolderName,
    });

    await page.goto('/tasks');
    await expect(page.getByRole('heading', { name: 'All Tasks' })).toBeVisible();

    await page.goto(`/projects/${project.id}/archives`);
    await expect(page.getByRole('heading', { name: 'Archives' })).toBeVisible();

    await page.getByRole('button', { name: 'Archive' }).click();
    const archiveDialog = page.getByRole('dialog', { name: 'Archive kanban' });
    await expect(archiveDialog).toBeVisible();
    await archiveDialog.getByRole('button', { name: 'Cancel' }).click();
    await expect(archiveDialog).toHaveCount(0);

    await page.goto(`/settings/projects?projectId=${project.id}`);
    await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Save Project Settings' })
    ).toBeVisible();
  });
});
