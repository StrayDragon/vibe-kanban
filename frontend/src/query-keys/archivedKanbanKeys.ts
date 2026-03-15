export const archivedKanbanKeys = {
  all: ['archived-kanbans'] as const,
  byProject: (projectId: string | undefined) =>
    ['archived-kanbans', projectId] as const,
  byId: (archiveId: string | undefined) =>
    ['archived-kanban', archiveId] as const,
};
