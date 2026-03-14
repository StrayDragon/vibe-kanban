import { uiIds } from '../frontend/src/lib/uiIds';
import { createRepoAndProject } from './helpers/setup';
import { expect, test } from './fixtures';

test('follow-up send triggers execution-processes resync', async ({
  page,
  seed,
  reposDir,
  makeName,
}) => {
  test.setTimeout(120_000);

  await page.addInitScript(() => {
    // Capture WebSocket close calls so we can assert follow-up triggers a resync.
    (window as any).__vkWsCloseEvents = [];

    const originalClose = WebSocket.prototype.close;
    WebSocket.prototype.close = function close(code?: number, reason?: string) {
      (window as any).__vkWsCloseEvents.push({
        url: (this as WebSocket).url,
        code,
        reason,
      });
      return originalClose.call(this, code as any, reason as any);
    };
  });

  const resyncEventPredicate = (evt: {
    url?: string;
    code?: number;
    reason?: string;
  }) =>
    typeof evt?.url === 'string' &&
    evt.url.includes('/api/execution-processes/stream/ws') &&
    evt.code === 4000 &&
    typeof evt.reason === 'string' &&
    evt.reason.includes('resync:follow-up-sent');

  const getResyncCloseCount = async () => {
    const closeEvents = await page.evaluate(() => {
      return (window as any).__vkWsCloseEvents ?? [];
    });
    return closeEvents.filter(resyncEventPredicate).length;
  };

  const projectName = makeName('project');
  const repoFolderName = makeName('repo');
  const { project } = await createRepoAndProject(page.request, {
    name: projectName,
    reposDir,
    repoFolderName,
  });
  const taskTitle = `E2E Seed ${seed} - ${makeName('task')}`;

  await page.goto(`/projects/${project.id}/tasks`);

  await page.locator(`#${uiIds.navbarCreateTask}`).click();
  await page.getByRole('menuitem', { name: 'Create new task' }).click();
  await page.locator(`#${uiIds.taskFormTitle}`).fill(taskTitle);
  await page.locator(`#${uiIds.taskFormSubmit}`).click();

  await page.waitForURL(
    /\/projects\/[^/]+\/tasks\/[^/]+\/attempts\/(?:latest|[^/]+)/
  );
  await expect(
    page.locator('#kanban').getByRole('heading', { name: taskTitle })
  ).toBeVisible({ timeout: 15_000 });

  // Ensure attempt history is loaded before asserting on follow-up UI.
  await expect(page.getByText('Loading History')).toBeHidden({
    timeout: 60_000,
  });

  // Wait until the attempt is idle so the follow-up action bar shows "Send".
  const sendButton = page.getByRole('button', { name: 'Send', exact: true });
  await expect(sendButton).toBeVisible({ timeout: 60_000 });

  const editor = page.getByLabel('Markdown editor').last();

  const sendFollowUp = async (message: string) => {
    const prevResyncCount = await getResyncCloseCount();

    await editor.click();
    await editor.type(message);
    await expect(sendButton).toBeEnabled({ timeout: 60_000 });
    const followUpResponse = page.waitForResponse((response) => {
      if (response.request().method() !== 'POST') return false;
      const url = response.url();
      return url.includes('/api/sessions/') && url.endsWith('/follow-up');
    });
    await sendButton.click();
    const followUpJson = await (await followUpResponse).json();

    // Follow-up completion triggers `resyncExecutionProcesses('follow-up-sent')`,
    // which closes the execution-processes WS with a resync reason.
    await page.waitForFunction(({ prevResyncCount }) => {
      const events: Array<{ url?: string; code?: number; reason?: string }> =
        (window as any).__vkWsCloseEvents ?? [];
      const count = events.filter(
        (evt) =>
          typeof evt?.url === 'string' &&
          evt.url.includes('/api/execution-processes/stream/ws') &&
          evt.code === 4000 &&
          typeof evt.reason === 'string' &&
          evt.reason.includes('resync:follow-up-sent')
      ).length;
      return count > prevResyncCount;
    }, { prevResyncCount });

    // Best-effort: editor is cleared after follow-up send.
    await page.waitForFunction(() => {
      const elements = Array.from(
        document.querySelectorAll<HTMLElement>('[aria-label="Markdown editor"]')
      );
      const last = elements[elements.length - 1];
      return (last?.textContent ?? '').trim().length === 0;
    });

    // Ensure the follow-up message is visible in the conversation history.
    await expect(page.getByText(message, { exact: true })).toBeVisible({
      timeout: 15_000,
    });

    await expect(sendButton).toBeDisabled({ timeout: 10_000 });

    return followUpJson?.data?.id as string | undefined;
  };

  const message1 = `Follow up e2e ${seed} #1`;
  const process1 = await sendFollowUp(message1);

  await expect(sendButton).toBeVisible({ timeout: 60_000 });
  const message2 = `Follow up e2e ${seed} #2`;
  const process2 = await sendFollowUp(message2);

  // Ensure the resync close events match our expected shape at least once.
  const closeEvents = await page.evaluate(() => {
    return (window as any).__vkWsCloseEvents ?? [];
  });
  expect(closeEvents.some(resyncEventPredicate)).toBeTruthy();

  // Sanity: each follow-up call returns a new execution process.
  expect(process1).toBeTruthy();
  expect(process2).toBeTruthy();
  expect(process2).not.toBe(process1);
});
