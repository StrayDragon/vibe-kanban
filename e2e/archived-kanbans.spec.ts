import { createRepoAndProject, createTask } from './helpers/setup';
import { expect, test } from './fixtures';

test('archive + restore updates the project kanban without manual refresh', async ({
  page,
  makeName,
  reposDir,
}) => {
  test.setTimeout(120_000);

  const projectName = makeName('project');
  const repoFolderName = makeName('repo');
  const { project } = await createRepoAndProject(page.request, {
    name: projectName,
    reposDir,
    repoFolderName,
  });

  const todoTitle = makeName('todo');
  const doneTitle = makeName('done');
  const cancelledTitle = makeName('cancelled');

  await createTask(page.request, {
    projectId: project.id,
    title: todoTitle,
    status: 'todo',
  });
  await createTask(page.request, {
    projectId: project.id,
    title: doneTitle,
    status: 'done',
  });
  await createTask(page.request, {
    projectId: project.id,
    title: cancelledTitle,
    status: 'cancelled',
  });

  await page.goto(`/projects/${project.id}/tasks`);

  const doneColumn = page.getByTestId('kanban-column-done');
  const cancelledColumn = page.getByTestId('kanban-column-cancelled');

  await expect(
    doneColumn.getByRole('heading', { name: doneTitle }).first()
  ).toBeVisible({ timeout: 60_000 });
  await expect(
    cancelledColumn.getByRole('heading', { name: cancelledTitle }).first()
  ).toBeVisible();

  await page.getByRole('button', { name: 'Archive', exact: true }).click();

  const archiveDialog = page.getByRole('dialog', { name: 'Archive kanban' });
  await expect(archiveDialog).toBeVisible();
  await archiveDialog.getByRole('textbox').fill(makeName('archive'));

  await archiveDialog.getByRole('button', { name: 'Archive', exact: true }).click();

  await page.waitForURL(
    new RegExp(`/projects/${project.id}/archives/[^/]+$`)
  );
  const archiveUrl = page.url();

  await expect(
    page.getByTestId('kanban-column-done').getByRole('heading', {
      name: doneTitle,
    })
  ).toBeVisible({ timeout: 60_000 });
  await expect(
    page.getByTestId('kanban-column-cancelled').getByRole('heading', {
      name: cancelledTitle,
    })
  ).toBeVisible();

  await page.getByRole('button', { name: 'Restore', exact: true }).click();
  const restoreDialog = page.getByRole('dialog', { name: 'Restore archive' });
  await expect(restoreDialog).toBeVisible();
  await restoreDialog.getByRole('button', { name: 'Restore', exact: true }).click();

  await page.waitForURL(new RegExp(`/projects/${project.id}/tasks`));

  await expect(
    page.getByTestId('kanban-column-done').getByRole('heading', {
      name: doneTitle,
    })
  ).toBeVisible({ timeout: 60_000 });
  await expect(
    page.getByTestId('kanban-column-cancelled').getByRole('heading', {
      name: cancelledTitle,
    })
  ).toBeVisible();

  // Sanity: restored tasks no longer appear in the archive detail board.
  await page.goto(archiveUrl);
  await expect(
    page.getByTestId('kanban-column-done').getByRole('heading', {
      name: doneTitle,
    })
  ).toHaveCount(0, { timeout: 60_000 });
});
