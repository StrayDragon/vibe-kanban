import { create } from 'zustand';
import type { ExecutionProcess } from 'shared/types';

type OptimisticExecutionProcessesStore = {
  byAttemptId: Record<string, Record<string, ExecutionProcess>>;
  insert: (attemptId: string, process: ExecutionProcess) => void;
  remove: (attemptId: string, processId: string) => void;
  removeMany: (attemptId: string, processIds: string[]) => void;
};

export const useOptimisticExecutionProcessesStore =
  create<OptimisticExecutionProcessesStore>((set) => ({
    byAttemptId: {},

    insert: (attemptId, process) => {
      set((state) => ({
        byAttemptId: {
          ...state.byAttemptId,
          [attemptId]: {
            ...(state.byAttemptId[attemptId] ?? {}),
            [process.id]: process,
          },
        },
      }));
    },

    remove: (attemptId, processId) => {
      set((state) => {
        const attempt = state.byAttemptId[attemptId];
        if (!attempt?.[processId]) return state;
        const nextAttempt = { ...attempt };
        delete nextAttempt[processId];
        return {
          byAttemptId: {
            ...state.byAttemptId,
            [attemptId]: nextAttempt,
          },
        };
      });
    },

    removeMany: (attemptId, processIds) => {
      set((state) => {
        const attempt = state.byAttemptId[attemptId];
        if (!attempt) return state;
        let changed = false;
        const nextAttempt = { ...attempt };
        for (const id of processIds) {
          if (!nextAttempt[id]) continue;
          delete nextAttempt[id];
          changed = true;
        }
        if (!changed) return state;
        return {
          byAttemptId: {
            ...state.byAttemptId,
            [attemptId]: nextAttempt,
          },
        };
      });
    },
  }));
