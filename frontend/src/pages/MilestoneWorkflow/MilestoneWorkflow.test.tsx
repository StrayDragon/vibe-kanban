import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

const {
  milestoneState,
  projectTasksState,
  milestonesUpdateMock,
  refetchMilestoneMock,
} = vi.hoisted(() => ({
  milestoneState: { current: null as unknown },
  projectTasksState: {
    current: { tasks: [], tasksById: {}, isLoading: false } as unknown,
  },
  milestonesUpdateMock: vi.fn().mockResolvedValue({}),
  refetchMilestoneMock: vi.fn().mockResolvedValue({}),
}));

vi.mock('@xyflow/react', async () => {
  const React = await import('react');

  const ReactFlowProvider = ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  );

  const ReactFlow = ({
    nodes = [],
    onSelectionChange,
    children,
  }: {
    nodes?: Array<{ id: string }>;
    onSelectionChange?: (event: { nodes?: Array<{ id: string }> }) => void;
    children?: React.ReactNode;
  }) => {
    return (
      <div>
        <div>
          {nodes.map((node) => (
            <button
              key={node.id}
              type="button"
              data-testid={`node-${node.id}`}
              onClick={() => onSelectionChange?.({ nodes: [node] })}
            >
              {node.id}
            </button>
          ))}
        </div>
        {children}
      </div>
    );
  };

  const useNodesState = <T,>(initialNodes: T[]) => {
    const [nodes, setNodes] = React.useState(initialNodes);
    return [nodes, setNodes, () => {}] as const;
  };

  const useEdgesState = <T,>(initialEdges: T[]) => {
    const [edges, setEdges] = React.useState(initialEdges);
    return [edges, setEdges, () => {}] as const;
  };

  return {
    Background: () => null,
    Controls: () => null,
    MiniMap: () => null,
    MarkerType: { ArrowClosed: 'ArrowClosed' },
    ReactFlow,
    ReactFlowProvider,
    addEdge: (_params: unknown, edges: unknown[]) => edges,
    applyEdgeChanges: (_changes: unknown, edges: unknown[]) => edges,
    applyNodeChanges: (_changes: unknown, nodes: unknown[]) => nodes,
    useEdgesState,
    useNodesState,
  };
});

vi.mock('react-router-dom', async () => {
  const actual =
    await vi.importActual<typeof import('react-router-dom')>(
      'react-router-dom'
    );
  return {
    ...actual,
    useNavigate: () => vi.fn(),
    useParams: () => ({ projectId: 'project-1', milestoneId: 'ms-1' }),
  };
});

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (_key: string, fallback?: string) => fallback ?? _key,
  }),
}));

vi.mock('@/hooks/milestones/useMilestone', () => ({
  useMilestone: () => ({
    data: milestoneState.current,
    isLoading: false,
    refetch: refetchMilestoneMock,
  }),
}));

vi.mock('@/hooks/projects/useProjectTasks', () => ({
  useProjectTasks: () => projectTasksState.current,
}));

vi.mock('@/hooks/task-attempts/useTaskAttempts', () => ({
  useTaskAttemptsWithSessions: () => ({ data: [], isLoading: false }),
}));

vi.mock('@/contexts/ProjectContext', () => ({
  useProject: () => ({
    projectId: 'project-1',
    project: { name: 'Project' },
  }),
}));

vi.mock('@/components/ConfigProvider', () => ({
  useUserSystem: () => ({
    profiles: [],
    config: { executor_profile: null },
  }),
}));

vi.mock('@/lib/api', () => ({
  milestonesApi: { update: milestonesUpdateMock },
  tasksApi: { create: vi.fn(), update: vi.fn() },
}));

import { MilestoneWorkflow } from './MilestoneWorkflow';

describe('MilestoneWorkflow', () => {
  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  it('persists node instructions and clears blank instructions', async () => {
    milestonesUpdateMock.mockClear();
    refetchMilestoneMock.mockClear();
    vi.useFakeTimers();

    const flushMicrotasks = async () => {
      await Promise.resolve();
      await Promise.resolve();
    };

    const taskA = {
      id: 'task-1',
      title: 'Task A',
      description: null,
      status: 'todo',
      task_kind: 'default',
      milestone_id: null,
      milestone_node_id: null,
    };

    milestoneState.current = {
      id: 'ms-1',
      title: 'Workflow',
      description: null,
      status: 'todo',
      baseline_ref: 'main',
      graph: {
        nodes: [
          {
            id: 'node-a',
            task_id: taskA.id,
            kind: 'task',
            phase: 0,
            executor_profile_id: null,
            base_strategy: 'topology',
            instructions: null,
            requires_approval: false,
            layout: { x: 0, y: 0 },
          },
        ],
        edges: [],
      },
    };

    projectTasksState.current = {
      tasks: [taskA],
      tasksById: { [taskA.id]: taskA },
      isLoading: false,
    };

    render(<MilestoneWorkflow />);

    fireEvent.click(screen.getByTestId('node-node-a'));
    fireEvent.click(screen.getByRole('button', { name: 'Details' }));

    const textarea = screen.getByPlaceholderText(
      'Optional node-specific guidance'
    );
    fireEvent.change(textarea, { target: { value: 'Do the thing' } });

    expect((textarea as HTMLTextAreaElement).value).toBe('Do the thing');
    await vi.advanceTimersByTimeAsync(700);
    await flushMicrotasks();
    expect(milestonesUpdateMock).toHaveBeenCalledTimes(1);

    const firstCall = milestonesUpdateMock.mock.calls[0];
    expect(firstCall?.[0]).toBe('ms-1');
    expect(firstCall?.[1]?.graph?.nodes?.[0]?.instructions).toBe(
      'Do the thing'
    );

    fireEvent.change(textarea, { target: { value: '   ' } });
    await vi.advanceTimersByTimeAsync(700);
    await flushMicrotasks();
    expect(milestonesUpdateMock).toHaveBeenCalledTimes(2);

    const lastCall =
      milestonesUpdateMock.mock.calls[
        milestonesUpdateMock.mock.calls.length - 1
      ];
    expect(lastCall?.[1]?.graph?.nodes?.[0]?.instructions).toBeNull();
  });

  it('preserves local draft when refreshed server data arrives', async () => {
    const taskA = {
      id: 'task-1',
      title: 'Task A',
      description: null,
      status: 'todo',
      task_kind: 'default',
      milestone_id: null,
      milestone_node_id: null,
    };

    projectTasksState.current = {
      tasks: [taskA],
      tasksById: { [taskA.id]: taskA },
      isLoading: false,
    };

    milestoneState.current = {
      id: 'ms-1',
      title: 'Workflow',
      description: null,
      status: 'todo',
      baseline_ref: 'main',
      graph: {
        nodes: [
          {
            id: 'node-a',
            task_id: taskA.id,
            kind: 'task',
            phase: 0,
            executor_profile_id: null,
            base_strategy: 'topology',
            instructions: null,
            requires_approval: false,
            layout: { x: 0, y: 0 },
          },
        ],
        edges: [],
      },
      updated_at: '2026-02-25T00:00:00.000Z',
    };

    const view = render(<MilestoneWorkflow />);

    fireEvent.click(screen.getByTestId('node-node-a'));
    fireEvent.click(screen.getByRole('button', { name: 'Details' }));

    const textarea = screen.getByPlaceholderText(
      'Optional node-specific guidance'
    );
    fireEvent.change(textarea, { target: { value: 'Draft instructions' } });

    milestoneState.current = {
      ...(milestoneState.current as Record<string, unknown>),
      updated_at: '2026-02-25T00:01:00.000Z',
    };

    view.rerender(<MilestoneWorkflow />);

    await waitFor(() => {
      const el = screen.getByPlaceholderText(
        'Optional node-specific guidance'
      ) as HTMLTextAreaElement;
      expect(el.value).toBe('Draft instructions');
    });
  });
});
