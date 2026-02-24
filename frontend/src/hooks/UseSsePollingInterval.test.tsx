import { renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { useSsePollingInterval } from './utils/useSsePollingInterval';

let isConnected = false;
let isVisible = true;

vi.mock('@/contexts/EventStreamContext', () => ({
  useEventStream: () => ({ isConnected, error: null }),
}));

vi.mock('@/hooks/utils/useDocumentVisibility', () => ({
  useDocumentVisibility: () => isVisible,
}));

describe('useSsePollingInterval', () => {
  it('disables polling when SSE is connected', () => {
    isConnected = true;
    isVisible = true;
    const { result } = renderHook(() => useSsePollingInterval(5000));
    expect(result.current).toBe(false);
  });

  it('returns fallback interval when disconnected and visible', () => {
    isConnected = false;
    isVisible = true;
    const { result } = renderHook(() => useSsePollingInterval(5000));
    expect(result.current).toBe(5000);
  });

  it('disables polling when document is hidden', () => {
    isConnected = false;
    isVisible = false;
    const { result } = renderHook(() => useSsePollingInterval(5000));
    expect(result.current).toBe(false);
  });
});
