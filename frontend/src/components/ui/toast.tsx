import { create } from 'zustand';
import { X } from 'lucide-react';

import { cn } from '@/lib/utils';

export type ToastVariant = 'default' | 'destructive';

export type ToastInput = {
  title: string;
  description?: string;
  variant?: ToastVariant;
  durationMs?: number;
};

type ToastItem = {
  id: string;
  title: string;
  description?: string;
  variant: ToastVariant;
};

let toastCounter = 0;
const nextToastId = () => `toast-${Date.now()}-${toastCounter++}`;

type ToastStore = {
  toasts: ToastItem[];
  push: (input: ToastInput) => string;
  dismiss: (id: string) => void;
  clear: () => void;
};

const useToastStore = create<ToastStore>((set, get) => ({
  toasts: [],
  push: (input) => {
    const id = nextToastId();
    const variant = input.variant ?? 'default';
    const durationMs =
      input.durationMs ?? (variant === 'destructive' ? 8000 : 4500);
    const item: ToastItem = {
      id,
      title: input.title,
      description: input.description,
      variant,
    };

    set((state) => ({ toasts: [...state.toasts, item] }));

    window.setTimeout(() => {
      get().dismiss(id);
    }, durationMs);

    return id;
  },
  dismiss: (id) =>
    set((state) => ({ toasts: state.toasts.filter((t) => t.id !== id) })),
  clear: () => set({ toasts: [] }),
}));

export function toast(input: ToastInput): string {
  return useToastStore.getState().push(input);
}

export function ToastViewport() {
  const toasts = useToastStore((s) => s.toasts);
  const dismiss = useToastStore((s) => s.dismiss);

  return (
    <div className="pointer-events-none fixed bottom-4 right-4 z-[100] flex w-[360px] max-w-[calc(100vw-2rem)] flex-col gap-2">
      {toasts.map((t) => (
        <div
          key={t.id}
          className={cn(
            'pointer-events-auto rounded-md border bg-background/95 p-3 shadow-lg backdrop-blur',
            t.variant === 'destructive' && 'border-destructive/40'
          )}
          role="status"
          aria-live="polite"
        >
          <div className="flex items-start gap-3">
            <div className="min-w-0 flex-1">
              <div
                className={cn(
                  'text-sm font-semibold',
                  t.variant === 'destructive' && 'text-destructive'
                )}
              >
                {t.title}
              </div>
              {t.description && (
                <div className="mt-1 text-xs text-muted-foreground">
                  {t.description}
                </div>
              )}
            </div>
            <button
              type="button"
              onClick={() => dismiss(t.id)}
              className="inline-flex h-7 w-7 items-center justify-center rounded hover:bg-muted"
              aria-label="Dismiss"
            >
              <X className="h-4 w-4" />
            </button>
          </div>
        </div>
      ))}
    </div>
  );
}
