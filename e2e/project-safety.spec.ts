import fs from 'node:fs/promises';
import path from 'node:path';

import { expect, test } from './fixtures';
import { createRepoAndProject, createTask } from './helpers/setup';
import { getConfigDir } from './helpers/seed';

test.describe('project safety', () => {
  test('same-name projects are disambiguated; create/delete actions are not shown', async ({
    page,
    makeName,
    reposDir,
  }) => {
    const sharedName = makeName('same-name');
    const { project: projectA } = await createRepoAndProject(page.request, {
      name: sharedName,
      reposDir,
      repoFolderName: makeName('repo-a'),
    });
    const { project: projectB } = await createRepoAndProject(page.request, {
      name: sharedName,
      reposDir,
      repoFolderName: makeName('repo-b'),
    });

    await page.goto(`/projects/${projectA.id}/tasks`);
    await page.getByRole('button', { name: 'Switch project' }).click();

    const menu = page.getByRole('menu');
    await expect(menu.getByText(projectA.id)).toBeVisible();
    await expect(menu.getByText(projectB.id)).toBeVisible();

    await expect(menu.locator('[data-project-delete]')).toHaveCount(0);
    await expect(
      menu.getByRole('menuitem', { name: 'Create Project' })
    ).toHaveCount(0);
  });

  test('orphaned project history shows Unknown project banner', async ({
    page,
    makeName,
    reposDir,
  }) => {
    const { project } = await createRepoAndProject(page.request, {
      name: makeName('orphaned-project'),
      reposDir,
      repoFolderName: makeName('repo'),
    });

    const taskTitle = makeName('task');
    await createTask(page.request, {
      projectId: project.id,
      title: taskTitle,
      status: 'todo',
    });

    const configPath = path.join(getConfigDir(), 'config.yaml');
    const raw = await fs.readFile(configPath, 'utf8');
    const current = raw.trim() ? JSON.parse(raw) : {};
    const projects = Array.isArray(current.projects) ? current.projects : [];
    const nextProjects = projects.filter((p: any) => p?.id !== project.id);
    await fs.writeFile(
      configPath,
      `${JSON.stringify({ ...current, projects: nextProjects }, null, 2)}\n`
    );
    await page.request.post('/api/config/reload', { data: {} });

    await page.goto(`/projects/${project.id}/tasks`);
    await expect(page.getByText('Unknown project')).toBeVisible();
    await expect(page.getByText(taskTitle)).toBeVisible();
  });
});

