import { expect, test } from './fixtures';
import { createRepoAndProject, unsafeWorktreesDir } from './helpers/setup';
import { uiIds } from '../frontend/src/lib/uiIds';
import type { Locator } from '@playwright/test';
import path from 'node:path';

async function expectAnyTextVisible(
  container: Locator,
  texts: string[]
): Promise<void> {
  for (const text of texts) {
    // eslint-disable-next-line no-await-in-loop
    if ((await container.getByText(text).count()) > 0) {
      // eslint-disable-next-line no-await-in-loop
      await expect(container.getByText(text).first()).toBeVisible();
      return;
    }
  }
  throw new Error(`Expected to find one of: ${texts.join(', ')}`);
}

async function findProjectRowByDisambiguator(
  menu: Locator,
  disambiguators: string[]
) {
  for (const value of disambiguators) {
    // eslint-disable-next-line no-await-in-loop
    const row = menu
      .getByRole('menuitemradio')
      .filter({ hasText: value })
      .first();
    // eslint-disable-next-line no-await-in-loop
    if ((await row.count()) > 0) {
      return row;
    }
  }
  throw new Error(`Could not find project row by: ${disambiguators.join(', ')}`);
}

test.describe('project safety', () => {
  test('same-name projects are disambiguated and delete confirm is unambiguous', async ({
    page,
    makeName,
    reposDir,
  }) => {
    const sharedName = makeName('same-name');
    const { repo: repoA, project: projectA } = await createRepoAndProject(
      page.request,
      {
        name: sharedName,
        reposDir,
        repoFolderName: makeName('repo-a'),
      }
    );
    const { repo: repoB, project: projectB } = await createRepoAndProject(
      page.request,
      {
        name: sharedName,
        reposDir,
        repoFolderName: makeName('repo-b'),
      }
    );

    const repoAFolder = path.basename(repoA.path);
    const repoBFolder = path.basename(repoB.path);

    await page.goto(`/projects/${projectA.id}/tasks`);
    await page.getByRole('button', { name: 'Switch project' }).click();

    const switcher = page.getByRole('menu');
    await expectAnyTextVisible(switcher, [repoAFolder, projectA.id]);
    await expectAnyTextVisible(switcher, [repoBFolder, projectB.id]);

    const projectBRow = await findProjectRowByDisambiguator(switcher, [
      repoBFolder,
      projectB.id,
    ]);
    await projectBRow.locator('[data-project-delete]').click();

    const confirm = page.getByRole('dialog', { name: /delete/i });
    await expect(confirm).toBeVisible();
    await expect(confirm.getByText(sharedName)).toBeVisible();
    await expectAnyTextVisible(confirm, [repoBFolder, projectB.id]);
    await confirm.getByRole('button', { name: 'Delete' }).click();

    await page.getByRole('button', { name: 'Switch project' }).click();
    await expect(page.getByRole('menu').getByText(repoBFolder)).toHaveCount(0, {
      timeout: 60_000,
    });
    await expect(page.getByRole('menu').getByText(projectB.id)).toHaveCount(0, {
      timeout: 60_000,
    });

    // Keep the remaining project usable.
    await expect(page).toHaveURL(new RegExp(`/projects/${projectA.id}/tasks`));
    expect(projectB.id).toBeTruthy();
  });

  test('project wizard is explicit and unsafe repo paths require acknowledgement', async ({
    page,
    makeName,
    reposDir,
  }) => {
    const baseName = makeName('base-project');
    const { project: baseProject } = await createRepoAndProject(page.request, {
      name: baseName,
      reposDir,
      repoFolderName: makeName('base-repo'),
    });

    await page.goto(`/projects/${baseProject.id}/tasks`);

    await page.getByRole('button', { name: 'Switch project' }).click();
    await page.getByRole('menuitem', { name: 'Create Project' }).click();

    // Step 1: pick or create a repo in an unsafe-looking directory.
    await page.locator(`#${uiIds.projectWizardPickRepo}`).click();
    await page.locator(`#${uiIds.repoPickerOptionNew}`).click();
    await page.locator(`#${uiIds.repoPickerName}`).fill(makeName('unsafe-repo'));
    await page
      .locator(`#${uiIds.repoPickerParentPath}`)
      .fill(unsafeWorktreesDir(reposDir));
    await page.locator(`#${uiIds.repoPickerSubmitCreate}`).click();

    // Selecting a repo must NOT create/navigate until the wizard is explicitly confirmed.
    await expect(page).toHaveURL(
      new RegExp(`/projects/${baseProject.id}/tasks`)
    );

    // Step 2: confirm name + acknowledge unsafe path.
    await page.locator(`#${uiIds.projectWizardName}`).fill(makeName('new-project'));

    const createButton = page.locator(`#${uiIds.projectWizardSubmitCreate}`);
    await expect(createButton).toBeDisabled();

    await page.locator(`#${uiIds.projectWizardUnsafeAck}`).click();
    await expect(createButton).toBeEnabled();

    await createButton.click();
    await expect(page).toHaveURL(/\/projects\/[^/]+\/tasks/);
  });
});
