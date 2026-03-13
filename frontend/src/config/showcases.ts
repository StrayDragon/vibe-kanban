import { ShowcaseConfig } from '@/types/showcase';

export const showcases = {
  taskPanel: {
    id: 'task-panel-onboarding',
    stages: [
      {
        titleKey: 'showcases.taskPanel.companion.title',
        descriptionKey: 'showcases.taskPanel.companion.description',
        media: {
          type: 'image',
          src: '/vibe-kanban-screenshot-overview.png',
        },
      },
      {
        titleKey: 'showcases.taskPanel.installation.title',
        descriptionKey: 'showcases.taskPanel.installation.description',
        media: {
          type: 'image',
          src: '/vibe-kanban-screenshot-overview.png',
        },
      },
      {
        titleKey: 'showcases.taskPanel.codeReview.title',
        descriptionKey: 'showcases.taskPanel.codeReview.description',
        media: {
          type: 'image',
          src: '/vibe-kanban-screenshot-overview.png',
        },
      },
      {
        titleKey: 'showcases.taskPanel.pullRequest.title',
        descriptionKey: 'showcases.taskPanel.pullRequest.description',
        media: {
          type: 'image',
          src: '/vibe-kanban-screenshot-overview.png',
        },
      },
      {
        titleKey: 'showcases.taskPanel.tags.title',
        descriptionKey: 'showcases.taskPanel.tags.description',
        media: {
          type: 'image',
          src: '/vibe-kanban-screenshot-overview.png',
        },
      },
    ],
  } satisfies ShowcaseConfig,
} as const;
