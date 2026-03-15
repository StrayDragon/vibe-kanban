import { createRepoAndProject, createTask } from './helpers/setup';
import { expect, test } from './fixtures';

test('events SSE does not replay history; resume via after_seq replays missed invalidations', async ({
  page,
  seed,
  reposDir,
  makeName,
}) => {
  test.setTimeout(120_000);

  await page.addInitScript(() => {
    (window as any).__vkSseEvents = [];
    (window as any).__vkSseNextId = 1;

    const OriginalEventSource = EventSource;

    (window as any).EventSource = function EventSource(
      url: string,
      eventSourceInitDict?: EventSourceInit
    ) {
      const es =
        eventSourceInitDict === undefined
          ? new OriginalEventSource(url)
          : new OriginalEventSource(url, eventSourceInitDict);

      const id = (window as any).__vkSseNextId++;
      (es as any).__vkEsId = id;

      const eventTypes = ['json_patch', 'invalidate', 'invalidate_all'];
      for (const type of eventTypes) {
        es.addEventListener(type, (evt) => {
          (window as any).__vkSseEvents.push({
            esId: id,
            type,
            lastEventId: (evt as any).lastEventId ?? null,
            data: (evt as any).data ?? null,
          });
        });
      }

      return es;
    } as any;

    (window as any).EventSource.prototype = OriginalEventSource.prototype;
    (window as any).EventSource.CONNECTING = OriginalEventSource.CONNECTING;
    (window as any).EventSource.OPEN = OriginalEventSource.OPEN;
    (window as any).EventSource.CLOSED = OriginalEventSource.CLOSED;
  });

  const projectName = makeName('project');
  const repoFolderName = makeName('repo');
  const { project } = await createRepoAndProject(page.request, {
    name: projectName,
    reposDir,
    repoFolderName,
  });

  await page.goto(`/projects/${project.id}/tasks`);

  const esIdA: number = await page.evaluate(() => {
    (window as any).__vkCustomSse?.close();
    const es = new EventSource('/api/events');
    (window as any).__vkCustomSse = es;
    return (es as any).__vkEsId as number;
  });

  const task1Title = `E2E Seed ${seed} - ${makeName('sse-task-1')}`;
  const task1 = await createTask(page.request, {
    projectId: project.id,
    title: task1Title,
    status: 'todo',
  });

  await page.waitForFunction(
    ({ esId, taskId }) => {
      const events: Array<{ esId: number; type: string; data: unknown }> =
        (window as any).__vkSseEvents ?? [];
      return events.some((evt) => {
        if (evt.esId !== esId) return false;
        if (evt.type !== 'invalidate') return false;
        if (typeof evt.data !== 'string') return false;
        try {
          const parsed = JSON.parse(evt.data) as { taskIds?: string[] };
          return Array.isArray(parsed.taskIds) && parsed.taskIds.includes(taskId);
        } catch {
          return false;
        }
      });
    },
    { esId: esIdA, taskId: task1.id }
  );

  const seq1: number = await page.evaluate(
    ({ esId, taskId }) => {
      const events: Array<{
        esId: number;
        type: string;
        lastEventId: string | null;
        data: unknown;
      }> = (window as any).__vkSseEvents ?? [];
      const match = events.find((evt) => {
        if (evt.esId !== esId) return false;
        if (evt.type !== 'invalidate') return false;
        if (typeof evt.data !== 'string') return false;
        try {
          const parsed = JSON.parse(evt.data) as { taskIds?: string[] };
          return Array.isArray(parsed.taskIds) && parsed.taskIds.includes(taskId);
        } catch {
          return false;
        }
      });
      const raw = match?.lastEventId;
      const parsed = raw ? Number(raw) : NaN;
      return Number.isFinite(parsed) ? parsed : -1;
    },
    { esId: esIdA, taskId: task1.id }
  );
  expect(seq1).toBeGreaterThanOrEqual(0);

  await page.evaluate(() => {
    (window as any).__vkCustomSse?.close();
    (window as any).__vkCustomSse = null;
    (window as any).__vkSseEvents = [];
  });

  // New connection without resume must NOT replay history containing task1.
  const esIdB: number = await page.evaluate(() => {
    (window as any).__vkCustomSse?.close();
    const es = new EventSource('/api/events');
    (window as any).__vkCustomSse = es;
    return (es as any).__vkEsId as number;
  });

  await page.waitForTimeout(800);

  const replayedTask1 = await page.evaluate(
    ({ esId, taskId }) => {
      const events: Array<{ esId: number; type: string; data: unknown }> =
        (window as any).__vkSseEvents ?? [];
      return events.some((evt) => {
        if (evt.esId !== esId) return false;
        if (evt.type !== 'invalidate') return false;
        if (typeof evt.data !== 'string') return false;
        try {
          const parsed = JSON.parse(evt.data) as { taskIds?: string[] };
          return Array.isArray(parsed.taskIds) && parsed.taskIds.includes(taskId);
        } catch {
          return false;
        }
      });
    },
    { esId: esIdB, taskId: task1.id }
  );
  expect(replayedTask1).toBe(false);

  await page.evaluate(() => {
    (window as any).__vkCustomSse?.close();
    (window as any).__vkCustomSse = null;
    (window as any).__vkSseEvents = [];
  });

  // Create a second task while disconnected, then resume via after_seq and expect to see it.
  const task2Title = `E2E Seed ${seed} - ${makeName('sse-task-2')}`;
  const task2 = await createTask(page.request, {
    projectId: project.id,
    title: task2Title,
    status: 'todo',
  });

  // Give the outbox worker time to publish the patch into the global msg_store history.
  await page.waitForTimeout(800);

  const esIdC: number = await page.evaluate(({ afterSeq }) => {
    const es = new EventSource(`/api/events?after_seq=${encodeURIComponent(String(afterSeq))}`);
    (window as any).__vkCustomSse = es;
    return (es as any).__vkEsId as number;
  }, { afterSeq: seq1 });

  await page.waitForFunction(
    ({ esId, taskId }) => {
      const events: Array<{ esId: number; type: string; data: unknown }> =
        (window as any).__vkSseEvents ?? [];
      return events.some((evt) => {
        if (evt.esId !== esId) return false;
        if (evt.type !== 'invalidate') return false;
        if (typeof evt.data !== 'string') return false;
        try {
          const parsed = JSON.parse(evt.data) as { taskIds?: string[] };
          return Array.isArray(parsed.taskIds) && parsed.taskIds.includes(taskId);
        } catch {
          return false;
        }
      });
    },
    { esId: esIdC, taskId: task2.id }
  );

  await page.evaluate(() => {
    (window as any).__vkCustomSse?.close();
    (window as any).__vkCustomSse = null;
  });
});

test('events SSE sends invalidate_all when resume is unavailable', async ({
  page,
  reposDir,
  makeName,
}) => {
  test.setTimeout(120_000);

  await page.addInitScript(() => {
    (window as any).__vkSseEvents = [];
    (window as any).__vkSseNextId = 1;

    const OriginalEventSource = EventSource;

    (window as any).EventSource = function EventSource(
      url: string,
      eventSourceInitDict?: EventSourceInit
    ) {
      const es =
        eventSourceInitDict === undefined
          ? new OriginalEventSource(url)
          : new OriginalEventSource(url, eventSourceInitDict);

      const id = (window as any).__vkSseNextId++;
      (es as any).__vkEsId = id;

      es.addEventListener('invalidate_all', (evt) => {
        (window as any).__vkSseEvents.push({
          esId: id,
          type: 'invalidate_all',
          lastEventId: (evt as any).lastEventId ?? null,
          data: (evt as any).data ?? null,
        });
      });

      return es;
    } as any;

    (window as any).EventSource.prototype = OriginalEventSource.prototype;
    (window as any).EventSource.CONNECTING = OriginalEventSource.CONNECTING;
    (window as any).EventSource.OPEN = OriginalEventSource.OPEN;
    (window as any).EventSource.CLOSED = OriginalEventSource.CLOSED;
  });

  const projectName = makeName('project');
  const repoFolderName = makeName('repo');
  const { project } = await createRepoAndProject(page.request, {
    name: projectName,
    reposDir,
    repoFolderName,
  });

  await page.goto(`/projects/${project.id}/tasks`);

  // Ask for a resume point far beyond watermark to force an explicit invalidate_all.
  const requestedAfterSeq = 9_999_999;
  const esId: number = await page.evaluate(({ requestedAfterSeq }) => {
    (window as any).__vkCustomSse?.close();
    const es = new EventSource(
      `/api/events?after_seq=${encodeURIComponent(String(requestedAfterSeq))}`
    );
    (window as any).__vkCustomSse = es;
    return (es as any).__vkEsId as number;
  }, { requestedAfterSeq });

  await page.waitForFunction(
    ({ esId }) => {
      const events: Array<{ esId: number; data: unknown }> =
        (window as any).__vkSseEvents ?? [];
      return events.some(
        (evt) =>
          evt.esId === esId && typeof evt.data === 'string' && evt.data.length > 0
      );
    },
    { esId }
  );

  const payload = await page.evaluate(({ esId }) => {
    const events: Array<{ esId: number; data: unknown }> =
      (window as any).__vkSseEvents ?? [];
    const match = events.find(
      (evt) => evt.esId === esId && typeof evt.data === 'string'
    );
    if (!match || typeof match.data !== 'string') return null;
    try {
      return JSON.parse(match.data) as { reason?: string };
    } catch {
      return null;
    }
  }, { esId });

  expect(payload?.reason).toBe('resume_unavailable');

  await page.evaluate(() => {
    (window as any).__vkCustomSse?.close();
    (window as any).__vkCustomSse = null;
  });
});
