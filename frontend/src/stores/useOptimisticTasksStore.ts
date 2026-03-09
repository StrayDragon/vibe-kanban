import { create } from 'zustand';
import type { TaskWithAttemptStatus } from 'shared/types';

type OptimisticMeta = {
  setAt: number;
  resyncAttempts: number;
  lastResyncAt: number | null;
};

type InsertEntry = {
  task: TaskWithAttemptStatus;
  meta: OptimisticMeta;
};

type OverrideEntry = {
  patch: Partial<TaskWithAttemptStatus>;
  meta: OptimisticMeta;
};

type TombstoneEntry = {
  meta: OptimisticMeta;
};

export type OptimisticTaskSnapshot = {
  insert?: InsertEntry;
  override?: OverrideEntry;
  tombstone?: TombstoneEntry;
};

const newMeta = (): OptimisticMeta => ({
  setAt: Date.now(),
  resyncAttempts: 0,
  lastResyncAt: null,
});

type OptimisticTasksStore = {
  inserts: Record<string, InsertEntry>;
  overrides: Record<string, OverrideEntry>;
  tombstones: Record<string, TombstoneEntry>;

  getSnapshot: (taskId: string) => OptimisticTaskSnapshot;
  restoreSnapshot: (taskId: string, snapshot: OptimisticTaskSnapshot) => void;

  insertTask: (task: TaskWithAttemptStatus) => void;
  clearInsert: (taskId: string) => void;

  setOverride: (
    taskId: string,
    patch: Partial<TaskWithAttemptStatus>,
    options?: { replace?: boolean }
  ) => void;
  clearOverride: (taskId: string) => void;

  tombstoneTask: (taskId: string) => void;
  clearTombstone: (taskId: string) => void;

  markResyncAttempt: (taskId: string) => void;
  reset: () => void;
};

export const useOptimisticTasksStore = create<OptimisticTasksStore>(
  (set, get) => ({
    inserts: {},
    overrides: {},
    tombstones: {},

    getSnapshot: (taskId) => {
      const state = get();
      return {
        insert: state.inserts[taskId],
        override: state.overrides[taskId],
        tombstone: state.tombstones[taskId],
      };
    },

    restoreSnapshot: (taskId, snapshot) => {
      set((state) => {
        const inserts = { ...state.inserts };
        const overrides = { ...state.overrides };
        const tombstones = { ...state.tombstones };

        if (snapshot.insert) {
          inserts[taskId] = snapshot.insert;
        } else {
          delete inserts[taskId];
        }

        if (snapshot.override) {
          overrides[taskId] = snapshot.override;
        } else {
          delete overrides[taskId];
        }

        if (snapshot.tombstone) {
          tombstones[taskId] = snapshot.tombstone;
        } else {
          delete tombstones[taskId];
        }

        return { inserts, overrides, tombstones };
      });
    },

    insertTask: (task) => {
      set((state) => ({
        inserts: {
          ...state.inserts,
          [task.id]: { task, meta: newMeta() },
        },
        // If we inserted a task, it is by definition not deleted.
        tombstones: (() => {
          const next = { ...state.tombstones };
          delete next[task.id];
          return next;
        })(),
      }));
    },

    clearInsert: (taskId) => {
      set((state) => {
        if (!state.inserts[taskId]) return state;
        const next = { ...state.inserts };
        delete next[taskId];
        return { inserts: next };
      });
    },

    setOverride: (taskId, patch, options) => {
      const replace = options?.replace ?? false;
      set((state) => {
        const existing = state.overrides[taskId];
        const nextPatch = replace
          ? patch
          : {
              ...(existing?.patch ?? {}),
              ...patch,
            };
        return {
          overrides: {
            ...state.overrides,
            [taskId]: { patch: nextPatch, meta: newMeta() },
          },
        };
      });
    },

    clearOverride: (taskId) => {
      set((state) => {
        if (!state.overrides[taskId]) return state;
        const next = { ...state.overrides };
        delete next[taskId];
        return { overrides: next };
      });
    },

    tombstoneTask: (taskId) => {
      set((state) => {
        const inserts = { ...state.inserts };
        const overrides = { ...state.overrides };
        const tombstones = { ...state.tombstones };

        delete inserts[taskId];
        delete overrides[taskId];
        tombstones[taskId] = { meta: newMeta() };

        return { inserts, overrides, tombstones };
      });
    },

    clearTombstone: (taskId) => {
      set((state) => {
        if (!state.tombstones[taskId]) return state;
        const next = { ...state.tombstones };
        delete next[taskId];
        return { tombstones: next };
      });
    },

    markResyncAttempt: (taskId) => {
      const now = Date.now();
      set((state) => {
        const inserts = { ...state.inserts };
        const overrides = { ...state.overrides };
        const tombstones = { ...state.tombstones };

        const updateMeta = (meta: OptimisticMeta): OptimisticMeta => ({
          ...meta,
          resyncAttempts: meta.resyncAttempts + 1,
          lastResyncAt: now,
        });

        if (inserts[taskId]) {
          inserts[taskId] = {
            ...inserts[taskId],
            meta: updateMeta(inserts[taskId].meta),
          };
        }
        if (overrides[taskId]) {
          overrides[taskId] = {
            ...overrides[taskId],
            meta: updateMeta(overrides[taskId].meta),
          };
        }
        if (tombstones[taskId]) {
          tombstones[taskId] = {
            ...tombstones[taskId],
            meta: updateMeta(tombstones[taskId].meta),
          };
        }

        return { inserts, overrides, tombstones };
      });
    },

    reset: () => set({ inserts: {}, overrides: {}, tombstones: {} }),
  })
);
