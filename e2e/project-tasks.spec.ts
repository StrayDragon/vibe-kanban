import { uiIds } from '../frontend/src/lib/uiIds';
import { createRepoAndProject } from './helpers/setup';
import { expect, test } from './fixtures';

test.describe('project tasks', () => {
  test('create/edit/delete/drag + reload consistency', async ({
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

    await page.goto(`/projects/${project.id}/tasks`);

    const taskTitle = makeName('task');
    await page.locator(`#${uiIds.navbarCreateTask}`).click();
    await page.getByRole('menuitem', { name: 'Create new task' }).click();
    await page.locator(`#${uiIds.taskFormTitle}`).fill(taskTitle);
    // Disable auto-start so the task stays in "To Do" for deterministic DnD assertions.
    const autoStartSwitch = page.locator('#autostart-switch');
    await expect(autoStartSwitch).toHaveAttribute('aria-checked', 'true');
    await autoStartSwitch.click();
    await expect(autoStartSwitch).toHaveAttribute('aria-checked', 'false');
    await page.locator(`#${uiIds.taskFormSubmit}`).click();

    const kanban = page.locator('#kanban');
    const todoColumn = kanban.getByTestId('kanban-column-todo');
    const doneColumn = kanban.getByTestId('kanban-column-done');

    await expect(
      todoColumn.getByRole('heading', { name: taskTitle }).first()
    ).toBeVisible({ timeout: 60_000 });

    await doneColumn.scrollIntoViewIfNeeded();
    const draggableCard = todoColumn
      .getByRole('button', { name: new RegExp(taskTitle) })
      .first();
    const from = await draggableCard.boundingBox();
    const to = await doneColumn.boundingBox();
    expect(from).not.toBeNull();
    expect(to).not.toBeNull();

    await page.mouse.move(from!.x + from!.width / 2, from!.y + from!.height / 2);
    await page.mouse.down();
    await page.mouse.move(to!.x + to!.width / 2, to!.y + to!.height / 2, {
      steps: 24,
    });
    await page.mouse.up();

    await expect(
      doneColumn.getByRole('heading', { name: taskTitle }).first()
    ).toBeVisible();
    await expect(
      todoColumn.getByRole('heading', { name: taskTitle }).first()
    ).toHaveCount(0);

    await page.reload();

    const doneColumnAfterReload = page
      .locator('#kanban')
      .getByTestId('kanban-column-done');
    await expect(
      doneColumnAfterReload.getByRole('heading', { name: taskTitle }).first()
    ).toBeVisible();

    const card = doneColumnAfterReload.getByRole('button', {
      name: new RegExp(taskTitle),
    });
    await card.getByRole('button', { name: 'Actions' }).click();
    await page.getByRole('menuitem', { name: 'Edit' }).click();

    const editedTitle = `${taskTitle} edited`;
    await page.locator(`#${uiIds.taskFormTitle}`).fill(editedTitle);
    await page.locator(`#${uiIds.taskFormSubmit}`).click();

    await expect(
      doneColumnAfterReload.getByRole('heading', { name: editedTitle }).first()
    ).toBeVisible({ timeout: 60_000 });

    const editedCard = doneColumnAfterReload.getByRole('button', {
      name: new RegExp(editedTitle),
    });
    await editedCard.getByRole('button', { name: 'Actions' }).click();
    await page.getByRole('menuitem', { name: 'Delete' }).click();

    const deleteDialog = page.getByRole('dialog', { name: /delete/i });
    await expect(deleteDialog).toBeVisible();
    await deleteDialog.getByRole('button', { name: /delete/i }).click();

    await expect(
      doneColumnAfterReload.getByRole('heading', { name: editedTitle }).first()
    ).toHaveCount(0, { timeout: 60_000 });

    await page.reload();
    await expect(
      page
        .locator('#kanban')
        .getByTestId('kanban-column-done')
        .getByRole('heading', { name: editedTitle })
    ).toHaveCount(0);
  });
});
