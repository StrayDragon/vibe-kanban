import { createRepoAndProject, createTask } from './helpers/setup';
import { expect, test } from './fixtures';

test('tasks WS resumes via after_seq after disconnect and replays missed patches', async ({
  page,
  seed,
  reposDir,
  makeName,
}) => {
  test.setTimeout(120_000);

  await page.addInitScript(() => {
    (window as any).__vkTasksWs = null;
    (window as any).__vkTasksWsOpenUrls = [];
    (window as any).__vkTasksWsCloseEvents = [];
    (window as any).__vkTasksWsLastSeq = null;

    const OriginalWebSocket = WebSocket;

    const originalClose = OriginalWebSocket.prototype.close;
    OriginalWebSocket.prototype.close = function close(code?: number, reason?: string) {
      (window as any).__vkTasksWsCloseEvents.push({
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
          (window as any).__vkTasksWs = ws;
          ws.addEventListener('open', () => {
            (window as any).__vkTasksWsOpenUrls.push(ws.url);
          });
          ws.addEventListener('message', (evt) => {
            try {
              const parsed = JSON.parse((evt as any).data);
              const seq = parsed?.seq;
              if (typeof seq === 'number' && Number.isFinite(seq)) {
                (window as any).__vkTasksWsLastSeq = seq;
              }
            } catch {
              // ignore
            }
          });
        }
      } catch {
        // ignore
      }

      return ws;
    } as any;

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

  await page.goto(`/projects/${project.id}/tasks`);

  // Wait for initial tasks WS open + at least one sequenced message.
  await page.waitForFunction(() => {
    const urls: string[] = (window as any).__vkTasksWsOpenUrls ?? [];
    return urls.some((url) => typeof url === 'string' && url.includes('/api/tasks/stream/ws'));
  });
  await page.waitForFunction(() => typeof (window as any).__vkTasksWsLastSeq === 'number');
  const initialSeq: number = await page.evaluate(() => (window as any).__vkTasksWsLastSeq as number);

  const initialUrls: string[] = await page.evaluate(() => (window as any).__vkTasksWsOpenUrls ?? []);
  expect(initialUrls.length).toBeGreaterThan(0);
  expect(initialUrls[0]).toContain('/api/tasks/stream/ws');
  expect(initialUrls[0]).not.toContain('after_seq=');

  // Disconnect the tasks WS and create a task via API while disconnected.
  await page.evaluate(() => {
    const ws: WebSocket | null = (window as any).__vkTasksWs;
    ws?.close(1000, 'e2e-disconnect');
  });

  await page.waitForFunction(() => {
    const events: Array<{ url?: string; reason?: string }> =
      (window as any).__vkTasksWsCloseEvents ?? [];
    return events.some(
      (evt) =>
        typeof evt?.url === 'string' &&
        evt.url.includes('/api/tasks/stream/ws') &&
        evt.reason === 'e2e-disconnect'
    );
  });

  const taskTitle = `E2E Seed ${seed} - ${makeName('resume-task')}`;
  await createTask(page.request, {
    projectId: project.id,
    title: taskTitle,
    status: 'todo',
  });

  // Verify the reconnect attempts include after_seq.
  await page.waitForFunction(() => {
    const urls: string[] = (window as any).__vkTasksWsOpenUrls ?? [];
    return urls.some((url) => typeof url === 'string' && url.includes('after_seq='));
  });

  // Ensure at least one new sequenced message arrived after reconnect.
  await page.waitForFunction(
    ({ initialSeq }) => {
      return (
        typeof (window as any).__vkTasksWsLastSeq === 'number' &&
        (window as any).__vkTasksWsLastSeq > initialSeq
      );
    },
    { initialSeq }
  );

  // UI converges without manual refresh.
  await expect(
    page.getByRole('heading', { name: taskTitle }).first()
  ).toBeVisible({ timeout: 30_000 });
});
