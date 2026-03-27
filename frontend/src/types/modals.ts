import type {
  ConfirmDialogProps,
  DeleteTaskConfirmationDialogProps,
  TaskFormDialogProps,
} from '@/components/dialogs';

// Type definitions for nice-modal-react modal arguments
declare module '@ebay/nice-modal-react' {
  interface ModalArgs {
    // Generic modals
    confirm: ConfirmDialogProps;

    // App flow modals
    disclaimer: void;
    onboarding: void;

    // Task-related modals
    'task-form': TaskFormDialogProps;
    'delete-task-confirmation': DeleteTaskConfirmationDialogProps;
  }
}

export {};
