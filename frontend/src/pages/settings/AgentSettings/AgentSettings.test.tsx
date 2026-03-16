import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import type { CodexProtocolCompatibility } from 'shared/types';

const { userSystemState, codexCompatState } = vi.hoisted(() => ({
  userSystemState: { current: null as Record<string, unknown> | null },
  codexCompatState: { current: null as CodexProtocolCompatibility | null },
}));

vi.mock('lodash', () => ({
  cloneDeep: <T,>(value: T): T => {
    const cloner = (
      globalThis as unknown as {
        structuredClone?: (v: unknown) => unknown;
      }
    ).structuredClone;
    if (typeof cloner === 'function') {
      return cloner(value) as T;
    }
    return JSON.parse(JSON.stringify(value)) as T;
  },
  isEqual: () => false,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('@/hooks/config/useProfiles', () => ({
  useProfiles: () => ({
    profilesContent: '',
    profilesPath: '/tmp/profiles.json',
    isLoading: false,
    isSaving: false,
    error: null,
    save: vi.fn(),
    refetch: vi.fn(),
  }),
}));

vi.mock('@/components/ConfigProvider', () => ({
  useUserSystem: () => userSystemState.current,
}));

vi.mock('@/hooks/config/useAgentAvailability', () => ({
  useAgentAvailability: () => ({ type: 'not_found' }),
}));

vi.mock('@/hooks/config/useCodexCompatibility', () => ({
  useCodexCompatibility: (
    _variant: string | null | undefined,
    enabled = true
  ) => {
    if (!enabled) {
      return {
        compatibility: null,
        isChecking: false,
        error: null,
        refresh: vi.fn(),
        check: vi.fn(),
      };
    }
    return {
      compatibility: codexCompatState.current,
      isChecking: false,
      error: null,
      refresh: vi.fn(),
      check: vi.fn(),
    };
  },
}));

vi.mock('@/components/ui/json-editor', async () => {
  const React = await import('react');
  return {
    JSONEditor: () => React.createElement('div', {}, 'JSONEditor'),
  };
});

vi.mock('@/components/ExecutorConfigForm', async () => {
  const React = await import('react');
  return {
    ExecutorConfigForm: () =>
      React.createElement('div', {}, 'ExecutorConfigForm'),
  };
});

vi.mock('@/components/dialogs/settings/CreateConfigurationDialog', async () => {
  const React = await import('react');
  return {
    CreateConfigurationDialog: () => React.createElement('div'),
  };
});

vi.mock('@/components/dialogs/settings/DeleteConfigurationDialog', async () => {
  const React = await import('react');
  return {
    DeleteConfigurationDialog: () => React.createElement('div'),
  };
});

vi.mock('@/components/dialogs/settings/SyncLlmanDialog', async () => {
  const React = await import('react');
  return {
    SyncLlmanDialog: () => React.createElement('div'),
  };
});

vi.mock('@/components/AgentAvailabilityIndicator', async () => {
  const React = await import('react');
  return {
    AgentAvailabilityIndicator: () =>
      React.createElement('div', {}, 'AgentAvailabilityIndicator'),
  };
});

import { AgentSettings } from './AgentSettings';

function setCompatStatus(status: CodexProtocolCompatibility['status']) {
  codexCompatState.current = {
    status,
    expected_v2_schema_sha256: 'expected',
    runtime_v2_schema_sha256: status === 'not_installed' ? null : 'runtime',
    codex_cli_version: '0.0.0-test',
    base_command: 'codex',
    message: status === 'compatible' ? null : 'blocked',
  };
}

function getSelectOption(label: string): HTMLElement {
  const nodes = screen.getAllByText(label);
  for (const node of nodes) {
    const option = node.closest('[role="option"]');
    if (option) return option as HTMLElement;
  }
  throw new Error(`Could not find select option: ${label}`);
}

describe('AgentSettings Codex compatibility gate', () => {
  it.each(['incompatible', 'unknown'] as const)(
    'disables selecting CODEX when compatibility status is %s',
    async (status) => {
      setCompatStatus(status);
      userSystemState.current = {
        config: {
          executor_profile: { executor: 'CLAUDE_CODE', variant: null },
        },
        updateAndSaveConfig: vi.fn().mockResolvedValue(true),
        profiles: {
          CLAUDE_CODE: { DEFAULT: {} },
          CODEX: { DEFAULT: {}, HIGH: {} },
        },
        reloadSystem: vi.fn(),
        agentCommandResolutions: {},
      };

      render(<AgentSettings />);

      const trigger = document.getElementById('executor');
      expect(trigger).toBeTruthy();
      fireEvent.click(trigger as HTMLElement);

      await screen.findByText('CODEX');
      const codexOption = getSelectOption('CODEX');
      expect(codexOption.getAttribute('data-disabled')).not.toBeNull();
    }
  );

  it.each(['incompatible', 'unknown'] as const)(
    'disables saving when CODEX is selected and status is %s, even when dirty',
    async (status) => {
      setCompatStatus(status);
      const updateAndSaveConfig = vi.fn().mockResolvedValue(true);
      userSystemState.current = {
        config: {
          executor_profile: { executor: 'CODEX', variant: null },
        },
        updateAndSaveConfig,
        profiles: {
          CLAUDE_CODE: { DEFAULT: {} },
          CODEX: { DEFAULT: {}, HIGH: {} },
        },
        reloadSystem: vi.fn(),
        agentCommandResolutions: {},
      };

      render(<AgentSettings />);

      const saveButton = screen.getByRole('button', {
        name: 'common:buttons.save',
      }) as HTMLButtonElement;
      expect(saveButton.disabled).toBe(true);

      fireEvent.click(saveButton);
      expect(updateAndSaveConfig).not.toHaveBeenCalled();
    }
  );

  it('allows saving when CODEX is selected and status is compatible', async () => {
    setCompatStatus('compatible');
    const updateAndSaveConfig = vi.fn().mockResolvedValue(true);
    userSystemState.current = {
      config: {
        executor_profile: { executor: 'CODEX', variant: null },
      },
      updateAndSaveConfig,
      profiles: {
        CLAUDE_CODE: { DEFAULT: {} },
        CODEX: { DEFAULT: {}, HIGH: {} },
      },
      reloadSystem: vi.fn(),
      agentCommandResolutions: {},
    };

    render(<AgentSettings />);

    const saveButton = screen.getByRole('button', {
      name: 'common:buttons.save',
    }) as HTMLButtonElement;
    expect(saveButton.disabled).toBe(false);

    fireEvent.click(saveButton);
    expect(updateAndSaveConfig).toHaveBeenCalledTimes(1);
  });
});
