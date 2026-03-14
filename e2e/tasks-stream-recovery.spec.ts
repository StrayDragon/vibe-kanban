import { uiIds } from '../frontend/src/lib/uiIds';
import { createRepoAndProject } from './helpers/setup';
import { expect, test } from './fixtures';

test('task creation remains visible when tasks WS misses create patch', async ({
  page,
  seed,
  reposDir,
  makeName,
}) => {
  test.setTimeout(120_000);

  await page.addInitScript(() => {
    (window as any).__vkWsCloseEvents = [];
    (window as any).__vkWsOpenEvents = [];
    (window as any).__vkDropTaskTitle = null;
    (window as any).__vkDroppedTaskPatch = false;

    const OriginalWebSocket = WebSocket;

    const originalClose = OriginalWebSocket.prototype.close;
    OriginalWebSocket.prototype.close = function close(code?: number, reason?: string) {
      (window as any).__vkWsCloseEvents.push({
        url: (this as WebSocket).url,
        code,
        reason,
      });
      return originalClose.call(this, code as any, reason as any);
    };

    (window as any).WebSocket = function WebSocket(
      url: string,
      protocols?: string | string[]
    ) {
      const ws =
        protocols === undefined
          ? new OriginalWebSocket(url)
          : new OriginalWebSocket(url, protocols);

      try {
        if (typeof url === 'string' && url.includes('/api/tasks/stream/ws')) {
          ws.addEventListener('open', () => {
            (window as any).__vkWsOpenEvents.push({ url: ws.url });
          });

          const descriptor = Object.getOwnPropertyDescriptor(
            OriginalWebSocket.prototype,
            'onmessage'
          );
          if (descriptor?.set && descriptor.get) {
            Object.defineProperty(ws, 'onmessage', {
              configurable: true,
              enumerable: descriptor.enumerable,
              get: () => descriptor.get!.call(ws),
              set(handler) {
                const original = handler as any;
                if (typeof original !== 'function') {
                  descriptor.set!.call(ws, original);
                  return;
                }

                const wrapped = function wrappedOnMessage(event: MessageEvent) {
                  try {
                    const dropTitle = (window as any).__vkDropTaskTitle;
                    const alreadyDropped = (window as any).__vkDroppedTaskPatch;
                    if (
                      typeof dropTitle === 'string' &&
                      dropTitle.length > 0 &&
                      !alreadyDropped
                    ) {
                      const parsed = JSON.parse((event as any).data);
                      const patches = parsed?.JsonPatch;
                      if (
                        Array.isArray(patches) &&
                        patches.some(
                          (patch: any) =>
                            (patch?.op === 'add' || patch?.op === 'replace') &&
                            typeof patch?.path === 'string' &&
                            ((patch.path.startsWith('/tasks/') &&
                              patch?.value?.title === dropTitle) ||
                              (patch.path === '/tasks' &&
                                patch?.op === 'replace' &&
                                patch?.value &&
                                typeof patch.value === 'object' &&
                                Object.values(patch.value).some(
                                  (task: any) => task?.title === dropTitle
                                )))
                        )
                      ) {
                        (window as any).__vkDroppedTaskPatch = true;
                        return;
                      }
                    }
                  } catch {
                    // ignore
                  }
                  return original.call(this, event);
                };

                descriptor.set!.call(ws, wrapped);
              },
            });
          }
        }
      } catch {
        // ignore
      }

      return ws;
    } as any;

    // Preserve prototype chain + constants for code that relies on them.
    (window as any).WebSocket.prototype = OriginalWebSocket.prototype;
    (window as any).WebSocket.CONNECTING = OriginalWebSocket.CONNECTING;
    (window as any).WebSocket.OPEN = OriginalWebSocket.OPEN;
    (window as any).WebSocket.CLOSING = OriginalWebSocket.CLOSING;
    (window as any).WebSocket.CLOSED = OriginalWebSocket.CLOSED;
  });

  const projectName = makeName('project');
  const repoFolderName = makeName('repo');
  const { project } = await createRepoAndProject(page.request, {
    name: projectName,
    reposDir,
    repoFolderName,
  });

  let shouldFailNextTaskGet = false;
  let createdTaskId: string | null = null;
  let injectedGetByIdFailure = false;
  await page.route('**/api/tasks/*', async (route, request) => {
    if (request.method() !== 'GET') return route.continue();
    if (!shouldFailNextTaskGet) return route.continue();

    const url = request.url();
    let pathname: string;
    try {
      pathname = new URL(url).pathname;
    } catch {
      return route.continue();
    }
    if (!pathname.startsWith('/api/tasks/')) return route.continue();
    const suffix = pathname.slice('/api/tasks/'.length);
    // Only fail task-by-id fetches (exclude other routes under /api/tasks/*).
    if (!suffix || suffix.includes('/')) return route.continue();
    if (injectedGetByIdFailure) return route.continue();

    injectedGetByIdFailure = true;
    return route.fulfill({
      status: 500,
      contentType: 'application/json',
      body: JSON.stringify({ success: false, message: 'e2e injected failure' }),
    });
  });

  await page.goto(`/projects/${project.id}/tasks`);

  // Ensure the tasks WS has established at least once so we can assert resync closes.
  await page.waitForFunction(() => {
    const opens: Array<{ url?: string }> = (window as any).__vkWsOpenEvents ?? [];
    return opens.some(
      (evt) =>
        typeof evt?.url === 'string' &&
        evt.url.includes('/api/tasks/stream/ws')
    );
  });

  const taskTitle = `E2E Seed ${seed} - ${makeName('task')}`;
  await page.evaluate((title) => {
    (window as any).__vkDropTaskTitle = title;
    (window as any).__vkDroppedTaskPatch = false;
  }, taskTitle);

  await page.locator(`#${uiIds.navbarCreateTask}`).click();
  await page.getByRole('menuitem', { name: 'Create new task' }).click();
  await page.locator(`#${uiIds.taskFormTitle}`).fill(taskTitle);

  // Disable auto-start so the create flow goes through `/api/tasks` (and thus
  // our test can inject a single `/api/tasks/:id` failure).
  const autoStartSwitch = page.locator('#autostart-switch');
  await expect(autoStartSwitch).toHaveAttribute('aria-checked', 'true');
  await autoStartSwitch.click();
  await expect(autoStartSwitch).toHaveAttribute('aria-checked', 'false');

  const createResponse = page.waitForResponse((response) => {
    if (response.request().method() !== 'POST') return false;
    const url = response.url();
    return url.includes('/api/tasks') && !url.includes('/create-and-start');
  });

  shouldFailNextTaskGet = true;
  await page.locator(`#${uiIds.taskFormSubmit}`).click();

  const createdJson = await (await createResponse).json();
  createdTaskId = createdJson?.data?.id ?? null;
  expect(createdTaskId).toBeTruthy();

  const todoColumn = page.locator('#kanban').getByTestId('kanban-column-todo');
  await expect(
    todoColumn.getByRole('heading', { name: taskTitle }).first()
  ).toBeVisible({ timeout: 60_000 });

  // The dropped WS patch + injected GET failure would previously yield no visible UI update.
  expect(injectedGetByIdFailure).toBeTruthy();

  // After optimistic state goes stale, the tasks stream should resync.
  await page.waitForFunction(() => {
    const events: Array<{ url?: string; code?: number; reason?: string }> =
      (window as any).__vkWsCloseEvents ?? [];
    return events.some(
      (evt) =>
        typeof evt?.url === 'string' &&
        evt.url.includes('/api/tasks/stream/ws') &&
        evt.code === 4000 &&
        typeof evt.reason === 'string' &&
        evt.reason.includes('resync:optimistic-stale')
    );
  });

  // Ensure the task remains visible throughout recovery.
  await expect(
    todoColumn.getByRole('heading', { name: taskTitle }).first()
  ).toBeVisible();
});
