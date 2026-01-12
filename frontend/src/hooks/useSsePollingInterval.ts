import { useDocumentVisibility } from '@/hooks/useDocumentVisibility';
import { useEventStream } from '@/contexts/EventStreamContext';

export function useSsePollingInterval(fallbackIntervalMs: number) {
  const { isConnected } = useEventStream();
  const isVisible = useDocumentVisibility();

  if (!isVisible || isConnected) {
    return false;
  }

  return fallbackIntervalMs;
}
