import { render } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { createSemanticHook } from '@/keyboard/useSemanticKey';
import { Action, Scope } from '@/keyboard/registry';

const { useHotkeysMock } = vi.hoisted(() => ({
  useHotkeysMock: vi.fn(),
}));

vi.mock('react-hotkeys-hook', () => ({
  useHotkeys: (...args: unknown[]) => useHotkeysMock(...args),
}));

describe('createSemanticHook', () => {
  it('evaluates `when` function and passes result to useHotkeys.enabled', () => {
    useHotkeysMock.mockClear();
    const useKey = createSemanticHook(Action.OPEN_DETAILS);
    const handler = vi.fn();

    function Test() {
      useKey(handler, { scope: Scope.KANBAN, when: () => false });
      return null;
    }

    render(<Test />);

    expect(useHotkeysMock).toHaveBeenCalled();
    const options = useHotkeysMock.mock.calls[0]?.[2] as { enabled?: boolean };
    expect(options.enabled).toBe(false);
  });

  it('supports boolean `when` and keeps IME composition from firing handler', () => {
    useHotkeysMock.mockClear();
    const useKey = createSemanticHook(Action.OPEN_DETAILS);
    const handler = vi.fn();

    function Test() {
      useKey(handler, { scope: Scope.KANBAN, when: true });
      return null;
    }

    render(<Test />);

    const callback = useHotkeysMock.mock.calls[0]?.[1] as (e: unknown) => void;
    callback({ isComposing: true });
    expect(handler).not.toHaveBeenCalled();
  });
});
