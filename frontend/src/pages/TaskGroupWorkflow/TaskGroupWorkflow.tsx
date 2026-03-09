import '@xyflow/react/dist/style.css';

import {
  Background,
  Controls,
  MiniMap,
  ReactFlow,
  ReactFlowProvider,
  addEdge,
  applyEdgeChanges,
  applyNodeChanges,
  useEdgesState,
  useNodesState,
  MarkerType,
  type Connection,
  type Edge,
  type EdgeChange,
  type NodeChange,
  type NodeTypes,
} from '@xyflow/react';
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
} from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { Check, Play, Plus, Trash2 } from 'lucide-react';
import {
  NewCard,
  NewCardContent,
  NewCardHeader,
} from '@/components/ui/new-card';
import { Loader } from '@/components/ui/loader';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { Textarea } from '@/components/ui/textarea';
import { toast } from '@/components/ui/toast';
import { ExecutorProfileSelector } from '@/components/settings';
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from '@/components/ui/breadcrumb';
import { ExecutionProcessesProvider } from '@/contexts/ExecutionProcessesContext';
import { ClickedElementsProvider } from '@/contexts/ClickedElementsProvider';
import { ReviewProvider } from '@/contexts/ReviewProvider';
import { useProject } from '@/contexts/ProjectContext';
import { useProjectTasks } from '@/hooks/projects/useProjectTasks';
import { useTaskAttemptsWithSessions } from '@/hooks/task-attempts/useTaskAttempts';
import { useTaskGroup } from '@/hooks/task-groups/useTaskGroup';
import { useDebouncedCallback } from '@/hooks/utils/useDebouncedCallback';
import TaskAttemptPanel from '@/components/panels/TaskAttemptPanel';
import WYSIWYGEditor from '@/components/ui/wysiwyg';
import { CreateAttemptDialog } from '@/components/dialogs/tasks/CreateAttemptDialog';
import TaskGroupNode, {
  type TaskGroupFlowNode,
} from '@/components/task-groups/TaskGroupNode';
import { useUserSystem } from '@/components/ConfigProvider';
import { statusBoardColors, statusLabels } from '@/utils/statusLabels';
import { getTaskGroupId, isTaskGroupEntry } from '@/utils/taskGroup';
import { taskGroupsApi, tasksApi } from '@/lib/api';
import { paths } from '@/lib/paths';
import type {
  TaskGroupGraph,
  TaskGroupGraphEdge,
  TaskGroupGraphNode,
  TaskGroupNodeBaseStrategy,
  TaskGroupNodeKind,
} from '@/types/task-group';
import type {
  ExecutorProfileId,
  MilestoneAutomationMode,
  TaskStatus,
} from 'shared/types';

const TASK_STATUSES: TaskStatus[] = [
  'todo',
  'inprogress',
  'inreview',
  'done',
  'cancelled',
];

const getNodeTaskId = (node: TaskGroupGraphNode): string | undefined =>
  node.task_id;

const getNodeExecutorProfileId = (
  node: TaskGroupGraphNode
): ExecutorProfileId | null =>
  node.executor_profile_id;

const getNodeBaseStrategy = (
  node: TaskGroupGraphNode
): TaskGroupNodeBaseStrategy =>
  node.base_strategy;

const getEdgeLabel = (edge: TaskGroupGraphEdge): string | undefined =>
  edge.data_flow ?? undefined;

const fallbackPosition = (index: number) => ({
  x: (index % 4) * 260,
  y: Math.floor(index / 4) * 180,
});

const createId = (prefix: string) => {
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) {
    return `${prefix}-${crypto.randomUUID()}`;
  }
  return `${prefix}-${Date.now().toString(36)}-${Math.random().toString(16).slice(2)}`;
};

const executorProfileKey = (profile: ExecutorProfileId | null): string => {
  if (!profile) return 'null';
  return `${profile.executor}::${profile.variant ?? ''}`;
};

const normalizeGraph = (graph?: TaskGroupGraph | null): TaskGroupGraph => ({
  nodes: (graph?.nodes ?? []).map((node) => ({ ...node })),
  edges: (graph?.edges ?? []).map((edge) => ({ ...edge })),
});

const graphKey = (graph: TaskGroupGraph): string => {
  const nodes = [...(graph.nodes ?? [])]
    .map((node) => ({
      id: node.id,
      taskId: getNodeTaskId(node) ?? null,
      kind: node.kind ?? 'task',
      phase: typeof node.phase === 'number' ? node.phase : null,
      executorProfileId: getNodeExecutorProfileId(node),
      baseStrategy: getNodeBaseStrategy(node),
      instructions: node.instructions ?? null,
      requiresApproval: node.requires_approval ?? null,
      layoutX: node.layout?.x ?? null,
      layoutY: node.layout?.y ?? null,
    }))
    .sort((a, b) => a.id.localeCompare(b.id));

  const edges = [...(graph.edges ?? [])]
    .map((edge) => ({
      id: edge.id,
      from: edge.from,
      to: edge.to,
      label: getEdgeLabel(edge) ?? null,
    }))
    .sort((a, b) => a.id.localeCompare(b.id));

  return JSON.stringify({ nodes, edges });
};

type FlowNode = TaskGroupFlowNode;

type FlowEdge = Edge;

type NewNodeMode = 'existing' | 'new';
type PanelView = 'chat' | 'details';

export function TaskGroupWorkflow() {
  const { t } = useTranslation('tasks');
  const navigate = useNavigate();
  const { projectId: routeProjectId, taskGroupId } = useParams<{
    projectId: string;
    taskGroupId: string;
  }>();
  const { projectId: contextProjectId, project } = useProject();
  const { profiles, config } = useUserSystem();
  const projectId = routeProjectId ?? contextProjectId;

  const {
    data: taskGroup,
    isLoading: isTaskGroupLoading,
    refetch: refetchTaskGroup,
  } = useTaskGroup(taskGroupId, { enabled: !!taskGroupId });
  const {
    tasks,
    tasksById,
    isLoading: isTasksLoading,
  } = useProjectTasks(projectId ?? '');

  const [graphDraft, setGraphDraft] = useState<TaskGroupGraph | null>(null);
  const [graphError, setGraphError] = useState<string | null>(null);
  const [isPersistingGraph, setIsPersistingGraph] = useState(false);
  const [selectedEdgeId, setSelectedEdgeId] = useState<string | null>(null);

  const serverGraph = useMemo(() => {
    if (!taskGroup) return null;
    return normalizeGraph(taskGroup.graph);
  }, [taskGroup]);

  const serverGraphKey = useMemo(() => {
    if (!serverGraph) return null;
    return graphKey(serverGraph);
  }, [serverGraph]);

  const graphDraftKey = useMemo(() => {
    if (!graphDraft) return null;
    return graphKey(graphDraft);
  }, [graphDraft]);

  const isGraphDraftDirty = useMemo(() => {
    if (!graphDraftKey || !serverGraphKey) return false;
    return graphDraftKey !== serverGraphKey;
  }, [graphDraftKey, serverGraphKey]);

  const lastTaskGroupIdRef = useRef<string | null>(null);
  useEffect(() => {
    if (!taskGroup) {
      lastTaskGroupIdRef.current = null;
      setGraphDraft(null);
      return;
    }
    const nextGraph = normalizeGraph(taskGroup.graph);
    const nextKey = graphKey(nextGraph);
    setGraphError(null);

    setGraphDraft((prev) => {
      const isNewTaskGroup = lastTaskGroupIdRef.current !== taskGroup.id;
      lastTaskGroupIdRef.current = taskGroup.id;

      if (isNewTaskGroup || !prev) return nextGraph;

      const prevKey = graphKey(prev);
      if (prevKey !== nextKey) {
        return prev;
      }
      return nextGraph;
    });
  }, [taskGroup]);

  const graph = useMemo(
    () => graphDraft ?? taskGroup?.graph ?? null,
    [graphDraft, taskGroup?.graph]
  );
  const graphNodes = useMemo(() => graph?.nodes ?? [], [graph]);
  const graphEdges = useMemo(() => graph?.edges ?? [], [graph]);

  const graphNodesById = useMemo(() => {
    const map = new Map<string, TaskGroupGraphNode>();
    graphNodes.forEach((node) => map.set(node.id, node));
    return map;
  }, [graphNodes]);

  const availableTasks = useMemo(() => {
    return tasks.filter((task) => {
      if (task.task_kind === 'group') return false;
      if (task.task_group_id) return false;
      return true;
    });
  }, [tasks]);

  const entryTask = useMemo(() => {
    if (!taskGroup) return null;
    return (
      tasks.find(
        (task) =>
          isTaskGroupEntry(task) && getTaskGroupId(task) === taskGroup.id
      ) ?? null
    );
  }, [taskGroup, tasks]);

  const masterNodeId = useMemo(
    () => (taskGroup ? `task-group-${taskGroup.id}-primary` : null),
    [taskGroup]
  );

  const nodeTypes = useMemo<NodeTypes>(
    () => ({ taskGroup: TaskGroupNode }),
    []
  );

  const defaultEdgeOptions = useMemo(
    () => ({
      type: 'smoothstep',
      markerEnd: { type: MarkerType.ArrowClosed },
      style: { stroke: 'hsl(var(--border))' },
      labelStyle: { fontSize: 10 },
      labelBgStyle: {
        fill: 'hsl(var(--background))',
        fillOpacity: 0.9,
      },
    }),
    []
  );

  const [nodes, setNodes] = useNodesState<FlowNode>([]);
  const [edges, setEdges] = useEdgesState<FlowEdge>([]);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [selectedEdgeLabel, setSelectedEdgeLabel] = useState('');
  const [panelView, setPanelView] = useState<PanelView>('chat');
  const [isUpdatingStatus, setIsUpdatingStatus] = useState(false);
  const [statusValue, setStatusValue] = useState<TaskStatus | null>(null);
  const [baselineValue, setBaselineValue] = useState('');
  const [objectiveValue, setObjectiveValue] = useState('');
  const [definitionOfDoneValue, setDefinitionOfDoneValue] = useState('');
  const [defaultExecutorProfileValue, setDefaultExecutorProfileValue] =
    useState<ExecutorProfileId | null>(null);
  const [automationModeValue, setAutomationModeValue] =
    useState<MilestoneAutomationMode>('manual');
  const [isUpdatingMilestone, setIsUpdatingMilestone] = useState(false);
  const [isRequestingNextStep, setIsRequestingNextStep] = useState(false);
  const [isAddNodeOpen, setIsAddNodeOpen] = useState(false);
  const [newNodeMode, setNewNodeMode] = useState<NewNodeMode>('existing');
  const [newNodeTaskId, setNewNodeTaskId] = useState<string | null>(null);
  const [newNodeTitle, setNewNodeTitle] = useState('');
  const [newNodeDescription, setNewNodeDescription] = useState('');
  const [newNodeKind, setNewNodeKind] = useState<TaskGroupNodeKind>('task');
  const [newNodePhase, setNewNodePhase] = useState(0);
  const [isAddingNode, setIsAddingNode] = useState(false);
  const hasAutoSelectedRef = useRef(false);

  const nodesRef = useRef<FlowNode[]>([]);
  useEffect(() => {
    nodesRef.current = nodes;
  }, [nodes]);

  useEffect(() => {
    hasAutoSelectedRef.current = false;
  }, [taskGroup?.id]);

  useEffect(() => {
    if (!taskGroup) return;
    setStatusValue(taskGroup.status);
  }, [taskGroup]);

  useEffect(() => {
    if (!taskGroup) return;
    setBaselineValue(taskGroup.baseline_ref ?? '');
  }, [taskGroup]);

  const lastMilestoneIdRef = useRef<string | null>(null);
  const lastServerMilestoneMetaRef = useRef<{
    task_group_id: string;
    objective: string;
    definition_of_done: string;
    default_executor_profile_key: string;
    automation_mode: MilestoneAutomationMode;
  } | null>(null);

  useEffect(() => {
    if (!taskGroup) {
      lastMilestoneIdRef.current = null;
      lastServerMilestoneMetaRef.current = null;
      setObjectiveValue('');
      setDefinitionOfDoneValue('');
      setDefaultExecutorProfileValue(null);
      setAutomationModeValue('manual');
      return;
    }

    const serverObjective = taskGroup.objective ?? '';
    const serverDoD = taskGroup.definition_of_done ?? '';
    const serverDefaultExecutor = taskGroup.default_executor_profile_id ?? null;
    const serverDefaultExecutorKey = executorProfileKey(serverDefaultExecutor);
    const serverAutomation = taskGroup.automation_mode ?? 'manual';

    const isNewMilestone = lastMilestoneIdRef.current !== taskGroup.id;
    lastMilestoneIdRef.current = taskGroup.id;

    if (isNewMilestone || !lastServerMilestoneMetaRef.current) {
      setObjectiveValue(serverObjective);
      setDefinitionOfDoneValue(serverDoD);
      setDefaultExecutorProfileValue(serverDefaultExecutor);
      setAutomationModeValue(serverAutomation);
    } else {
      setObjectiveValue((prev) => {
        const lastServer = lastServerMilestoneMetaRef.current?.objective ?? '';
        return prev === lastServer ? serverObjective : prev;
      });
      setDefinitionOfDoneValue((prev) => {
        const lastServer =
          lastServerMilestoneMetaRef.current?.definition_of_done ?? '';
        return prev === lastServer ? serverDoD : prev;
      });
      setDefaultExecutorProfileValue((prev) => {
        const lastKey =
          lastServerMilestoneMetaRef.current?.default_executor_profile_key ??
          'null';
        return executorProfileKey(prev) === lastKey
          ? serverDefaultExecutor
          : prev;
      });
      setAutomationModeValue((prev) => {
        const lastServer =
          lastServerMilestoneMetaRef.current?.automation_mode ?? 'manual';
        return prev === lastServer ? serverAutomation : prev;
      });
    }

    lastServerMilestoneMetaRef.current = {
      task_group_id: taskGroup.id,
      objective: serverObjective,
      definition_of_done: serverDoD,
      default_executor_profile_key: serverDefaultExecutorKey,
      automation_mode: serverAutomation,
    };
  }, [taskGroup]);

  const persistGraph = useCallback(
    async (nextGraph: TaskGroupGraph) => {
      if (!taskGroup) return;
      setIsPersistingGraph(true);
      setGraphError(null);
      try {
        await taskGroupsApi.update(taskGroup.id, { graph: nextGraph });
        await refetchTaskGroup();
      } catch (error) {
        console.error('Failed to update milestone graph:', error);
        setGraphError('Failed to save workflow changes.');
        await refetchTaskGroup();
      } finally {
        setIsPersistingGraph(false);
      }
    },
    [refetchTaskGroup, taskGroup]
  );

  const {
    debounced: persistGraphDebounced,
    cancel: cancelPersistGraphDebounced,
  } = useDebouncedCallback(persistGraph, 600);

  const handleDiscardGraphDraft = useCallback(() => {
    if (!taskGroup) return;
    cancelPersistGraphDebounced();
    setGraphDraft(normalizeGraph(taskGroup.graph));
    setGraphError(null);
  }, [cancelPersistGraphDebounced, taskGroup]);

  const updateGraphDraft = useCallback(
    (updater: (prev: TaskGroupGraph) => TaskGroupGraph) => {
      setGraphDraft((prev) => {
        const base = prev ?? { nodes: [], edges: [] };
        const next = updater(base);
        persistGraphDebounced(next);
        return next;
      });
    },
    [persistGraphDebounced]
  );

  const updateNode = useCallback(
    (
      nodeId: string,
      updater: (node: TaskGroupGraphNode) => TaskGroupGraphNode
    ) => {
      updateGraphDraft((prev) => ({
        ...prev,
        nodes: prev.nodes.map((node) =>
          node.id === nodeId ? updater(node) : node
        ),
      }));
    },
    [updateGraphDraft]
  );

  const handleInlineNodeUpdate = useCallback(
    (nodeId: string, updates: { kind?: TaskGroupNodeKind; phase?: number }) => {
      updateNode(nodeId, (node) => {
        const next = { ...node };
        if (updates.kind) {
          next.kind = updates.kind;
          next.requires_approval =
            updates.kind === 'checkpoint'
              ? true
              : (node.requires_approval ?? false);
        }
        if (typeof updates.phase === 'number') {
          next.phase = updates.phase;
        }
        return next;
      });
    },
    [updateNode]
  );

  useEffect(() => {
    if (!graph) return;

    setEdges(
      graphEdges.map((edge) => ({
        id: edge.id,
        source: edge.from,
        target: edge.to,
        label: getEdgeLabel(edge),
      }))
    );

    setNodes((prev: FlowNode[]) => {
      const prevMap = new Map<string, FlowNode>(
        prev.map((node) => [node.id, node])
      );
      const nextNodes: FlowNode[] = graphNodes.map((node, index) => {
        const prevNode = prevMap.get(node.id);
        const taskId = getNodeTaskId(node);
        const task = taskId ? tasksById[taskId] : undefined;
        const layout = node.layout;
        const fallback = fallbackPosition(index);
        const position =
          prevNode?.position ??
          ({
            x: typeof layout.x === 'number' ? layout.x : fallback.x,
            y: typeof layout.y === 'number' ? layout.y : fallback.y,
          } as FlowNode['position']);

        return {
          id: node.id,
          type: 'taskGroup',
          position,
          data: {
            title: task?.title ?? node.id,
            status: task?.status,
            kind: node.kind,
            taskId,
            phase: node.phase,
            executorProfileId: getNodeExecutorProfileId(node),
            baseStrategy: getNodeBaseStrategy(node),
            requiresApproval: node.requires_approval ?? node.kind === 'checkpoint',
            onUpdate: (updates: { kind?: TaskGroupNodeKind; phase?: number }) =>
              handleInlineNodeUpdate(node.id, updates),
          },
        };
      });

      if (masterNodeId) {
        const positions = nextNodes.map((node) => node.position);
        const minX =
          positions.length > 0 ? Math.min(...positions.map((pos) => pos.x)) : 0;
        const minY =
          positions.length > 0 ? Math.min(...positions.map((pos) => pos.y)) : 0;
        const masterPosition = positions.length
          ? { x: minX - 260, y: minY }
          : { x: 0, y: 0 };

        const masterNode: FlowNode = {
          id: masterNodeId,
          type: 'taskGroup',
          position: masterPosition,
          draggable: false,
          connectable: false,
          data: {
            title: entryTask?.title || taskGroup?.title || 'Milestone',
            status: entryTask?.status ?? taskGroup?.status,
            kind: 'task',
            taskId: entryTask?.id,
            isMaster: true,
          },
        };

        return [masterNode, ...nextNodes];
      }

      return nextNodes;
    });
  }, [
    entryTask,
    graph,
    graphEdges,
    graphNodes,
    handleInlineNodeUpdate,
    masterNodeId,
    setEdges,
    setNodes,
    taskGroup,
    tasksById,
  ]);

  useEffect(() => {
    if (
      selectedNodeId &&
      !graphNodesById.has(selectedNodeId) &&
      selectedNodeId !== masterNodeId
    ) {
      setSelectedNodeId(null);
    }
  }, [graphNodesById, masterNodeId, selectedNodeId]);

  useEffect(() => {
    if (
      selectedEdgeId &&
      !graphEdges.find((edge) => edge.id === selectedEdgeId)
    ) {
      setSelectedEdgeId(null);
    }
  }, [graphEdges, selectedEdgeId]);

  useEffect(() => {
    if (!selectedEdgeId) {
      setSelectedEdgeLabel('');
      return;
    }
    const edge = graphEdges.find((edge) => edge.id === selectedEdgeId);
    setSelectedEdgeLabel(edge ? (getEdgeLabel(edge) ?? '') : '');
  }, [graphEdges, selectedEdgeId]);

  useEffect(() => {
    if (selectedEdgeId) {
      setPanelView('details');
      return;
    }
    if (selectedNodeId) {
      setPanelView('chat');
    }
  }, [selectedEdgeId, selectedNodeId]);

  useEffect(() => {
    if (selectedEdgeId) return;
    if (selectedNodeId) return;
    if (!masterNodeId) return;
    if (hasAutoSelectedRef.current) return;
    hasAutoSelectedRef.current = true;
    setSelectedNodeId(masterNodeId);
  }, [masterNodeId, selectedEdgeId, selectedNodeId]);

  const isMasterSelected = Boolean(
    masterNodeId && selectedNodeId === masterNodeId
  );
  const selectedGraphNode =
    selectedNodeId && !isMasterSelected
      ? graphNodesById.get(selectedNodeId)
      : null;
  const selectedTaskId = selectedGraphNode
    ? getNodeTaskId(selectedGraphNode)
    : undefined;
  const selectedTask = isMasterSelected
    ? (entryTask ?? undefined)
    : selectedTaskId
      ? tasksById[selectedTaskId]
      : undefined;

  const { data: selectedAttempts = [], isLoading: isAttemptsLoading } =
    useTaskAttemptsWithSessions(selectedTask?.id, {
      enabled: !!selectedTask?.id,
    });

  const latestAttempt = useMemo(() => {
    if (!selectedAttempts.length) return undefined;
    return [...selectedAttempts].sort((a, b) => {
      const diff =
        new Date(b.created_at).getTime() - new Date(a.created_at).getTime();
      if (diff !== 0) return diff;
      return a.id.localeCompare(b.id);
    })[0];
  }, [selectedAttempts]);

  const selectedEdge = selectedEdgeId
    ? (graphEdges.find((edge) => edge.id === selectedEdgeId) ?? null)
    : null;

  const nodePredecessors = useMemo(() => {
    const map = new Map<string, string[]>();
    graphEdges.forEach((edge) => {
      const from = edge.from?.trim();
      const to = edge.to?.trim();
      if (!from || !to) return;
      const list = map.get(to) ?? [];
      list.push(from);
      map.set(to, list);
    });
    return map;
  }, [graphEdges]);

  const blockedBy = useMemo(() => {
    if (!selectedGraphNode) return [];
    const predecessors = nodePredecessors.get(selectedGraphNode.id) ?? [];
    return predecessors
      .map((nodeId) => {
        const node = graphNodesById.get(nodeId);
        const taskId = node ? getNodeTaskId(node) : undefined;
        const task = taskId ? tasksById[taskId] : undefined;
        return {
          id: nodeId,
          label: task?.title ?? nodeId,
          status: task?.status ?? 'todo',
        };
      })
      .filter((item) => item.status !== 'done');
  }, [graphNodesById, nodePredecessors, selectedGraphNode, tasksById]);

  const selectedEdgeDetails = useMemo(() => {
    if (!selectedEdge) return null;
    const fromNode = graphNodesById.get(selectedEdge.from);
    const toNode = graphNodesById.get(selectedEdge.to);
    const fromTaskId = fromNode ? getNodeTaskId(fromNode) : undefined;
    const toTaskId = toNode ? getNodeTaskId(toNode) : undefined;
    const fromTask = fromTaskId ? tasksById[fromTaskId] : undefined;
    const toTask = toTaskId ? tasksById[toTaskId] : undefined;
    return {
      fromLabel: fromTask?.title ?? selectedEdge.from,
      toLabel: toTask?.title ?? selectedEdge.to,
    };
  }, [graphNodesById, selectedEdge, tasksById]);

  const milestoneProgress = useMemo(() => {
    const counts: Record<TaskStatus, number> = {
      todo: 0,
      inprogress: 0,
      inreview: 0,
      done: 0,
      cancelled: 0,
    };
    let missing = 0;
    let activeAttemptTaskTitle: string | null = null;

    graphNodes.forEach((node) => {
      const taskId = getNodeTaskId(node);
      if (!taskId) return;
      const task = tasksById[taskId];
      if (!task) {
        missing += 1;
        return;
      }
      counts[task.status] += 1;
      if (!activeAttemptTaskTitle && task.has_in_progress_attempt) {
        activeAttemptTaskTitle = task.title || task.id;
      }
    });

    const total = graphNodes.length;
    const done = counts.done;
    const percent =
      total > 0 ? Math.round((done / total) * 100) : counts.done > 0 ? 100 : 0;

    return { counts, total, done, percent, missing, activeAttemptTaskTitle };
  }, [graphNodes, tasksById]);

  const isGraphNodeSelected = Boolean(selectedGraphNode);
  const selectedKind = selectedGraphNode?.kind ?? 'task';
  const selectedExecutorProfile = selectedGraphNode
    ? getNodeExecutorProfileId(selectedGraphNode)
    : null;
  const selectedBaseStrategy = selectedGraphNode
    ? getNodeBaseStrategy(selectedGraphNode)
    : 'topology';
  const requiresApproval =
    selectedGraphNode?.requires_approval ?? selectedKind === 'checkpoint';
  const isCheckpoint = isGraphNodeSelected && selectedKind === 'checkpoint';
  const isNodeReady = isGraphNodeSelected ? blockedBy.length === 0 : true;
  const isPendingApproval = Boolean(
    selectedGraphNode &&
      requiresApproval &&
      isNodeReady &&
      selectedTask?.status !== 'done'
  );

  const suggestedStatus = taskGroup?.suggested_status ?? null;
  const currentStatus = statusValue ?? taskGroup?.status ?? 'todo';

  const handleNodesChange = useCallback(
    (changes: NodeChange<FlowNode>[]) => {
      const nextNodes = applyNodeChanges(changes, nodesRef.current);
      setNodes(nextNodes);
      nodesRef.current = nextNodes;
      const removedIds = changes
        .filter((change) => change.type === 'remove')
        .map((change) => change.id)
        .filter((id) => graphNodesById.has(id));
      const hasPositionChange = changes.some(
        (change) =>
          change.type === 'position' &&
          !change.dragging &&
          graphNodesById.has(change.id)
      );
      if (removedIds.length > 0) {
        updateGraphDraft((prev) => {
          const removed = new Set(removedIds);
          return {
            ...prev,
            nodes: prev.nodes.filter((node) => !removed.has(node.id)),
            edges: prev.edges.filter(
              (edge) => !removed.has(edge.from) && !removed.has(edge.to)
            ),
          };
        });
      }
      if (hasPositionChange) {
        updateGraphDraft((prev) => {
          const positions = new Map(
            nextNodes
              .filter((node) => graphNodesById.has(node.id))
              .map((node) => [node.id, node.position])
          );
          return {
            ...prev,
            nodes: prev.nodes.map((node) => {
              const position = positions.get(node.id);
              if (!position) return node;
              return {
                ...node,
                layout: {
                  ...(node.layout ?? {}),
                  x: position.x,
                  y: position.y,
                },
              };
            }),
          };
        });
      }
    },
    [graphNodesById, setNodes, updateGraphDraft]
  );

  const handleEdgesChange = useCallback(
    (changes: EdgeChange[]) => {
      setEdges((prev) => applyEdgeChanges(changes, prev));
      const removedIds = changes
        .filter((change) => change.type === 'remove')
        .map((change) => change.id);
      if (removedIds.length > 0) {
        updateGraphDraft((prev) => ({
          ...prev,
          edges: prev.edges.filter((edge) => !removedIds.includes(edge.id)),
        }));
      }
    },
    [setEdges, updateGraphDraft]
  );

  const handleConnect = useCallback(
    (connection: Connection) => {
      if (!connection.source || !connection.target) return;
      const edgeId = createId('edge');
      setEdges((prev) =>
        addEdge(
          {
            id: edgeId,
            source: connection.source!,
            target: connection.target!,
          },
          prev
        )
      );
      updateGraphDraft((prev) => ({
        ...prev,
        edges: [
          ...prev.edges,
          {
            id: edgeId,
            from: connection.source!,
            to: connection.target!,
            data_flow: null,
          },
        ],
      }));
      setSelectedNodeId(null);
      setSelectedEdgeId(edgeId);
    },
    [setEdges, updateGraphDraft]
  );

  const handleStatusChange = useCallback(
    async (nextStatus: TaskStatus) => {
      if (!taskGroup) return;
      if (nextStatus === currentStatus) return;
      const previousStatus = currentStatus;
      setStatusValue(nextStatus);
      setIsUpdatingStatus(true);
      try {
        await taskGroupsApi.update(taskGroup.id, { status: nextStatus });
        await refetchTaskGroup();
      } catch (error) {
        console.error('Failed to update milestone status:', error);
        setStatusValue(previousStatus);
      } finally {
        setIsUpdatingStatus(false);
      }
    },
    [currentStatus, refetchTaskGroup, taskGroup]
  );

  const handleApplySuggested = useCallback(async () => {
    if (!suggestedStatus) return;
    await handleStatusChange(suggestedStatus);
  }, [handleStatusChange, suggestedStatus]);

  const handleBaselineSave = useCallback(async () => {
    if (!taskGroup) return;
    const trimmed = baselineValue.trim();
    const current = taskGroup.baseline_ref ?? '';
    if (!trimmed || trimmed === current) {
      setBaselineValue(current);
      return;
    }
    try {
      await taskGroupsApi.update(taskGroup.id, { baseline_ref: trimmed });
      await refetchTaskGroup();
    } catch (error) {
      console.error('Failed to update baseline ref:', error);
      setBaselineValue(current);
    }
  }, [baselineValue, refetchTaskGroup, taskGroup]);

  const handleBaselineKeyDown = useCallback(
    (event: KeyboardEvent<HTMLInputElement>) => {
      if (event.key !== 'Enter') return;
      event.preventDefault();
      handleBaselineSave();
    },
    [handleBaselineSave]
  );

  const handleObjectiveSave = useCallback(async () => {
    if (!taskGroup) return;
    const trimmed = objectiveValue.trim();
    const nextValue = trimmed.length ? trimmed : null;
    const currentValue = taskGroup.objective ?? null;

    if (currentValue === nextValue) {
      setObjectiveValue(nextValue ?? '');
      return;
    }

    setIsUpdatingMilestone(true);
    try {
      await taskGroupsApi.update(taskGroup.id, { objective: nextValue });
      await refetchTaskGroup();
    } catch (error) {
      console.error('Failed to update milestone objective:', error);
      toast({
        variant: 'destructive',
        title: 'Failed to save objective',
        description: error instanceof Error ? error.message : undefined,
      });
      setObjectiveValue(taskGroup.objective ?? '');
    } finally {
      setIsUpdatingMilestone(false);
    }
  }, [objectiveValue, refetchTaskGroup, taskGroup]);

  const handleDefinitionOfDoneSave = useCallback(async () => {
    if (!taskGroup) return;
    const trimmed = definitionOfDoneValue.trim();
    const nextValue = trimmed.length ? trimmed : null;
    const currentValue = taskGroup.definition_of_done ?? null;

    if (currentValue === nextValue) {
      setDefinitionOfDoneValue(nextValue ?? '');
      return;
    }

    setIsUpdatingMilestone(true);
    try {
      await taskGroupsApi.update(taskGroup.id, {
        definition_of_done: nextValue,
      });
      await refetchTaskGroup();
    } catch (error) {
      console.error('Failed to update milestone definition of done:', error);
      toast({
        variant: 'destructive',
        title: 'Failed to save definition of done',
        description: error instanceof Error ? error.message : undefined,
      });
      setDefinitionOfDoneValue(taskGroup.definition_of_done ?? '');
    } finally {
      setIsUpdatingMilestone(false);
    }
  }, [definitionOfDoneValue, refetchTaskGroup, taskGroup]);

  const handleDefaultExecutorProfileSave = useCallback(
    async (profile: ExecutorProfileId | null) => {
      if (!taskGroup) return;
      const previous = defaultExecutorProfileValue;
      setDefaultExecutorProfileValue(profile);
      setIsUpdatingMilestone(true);
      try {
        await taskGroupsApi.update(taskGroup.id, {
          default_executor_profile_id: profile,
        });
        await refetchTaskGroup();
      } catch (error) {
        console.error(
          'Failed to update milestone default executor profile:',
          error
        );
        toast({
          variant: 'destructive',
          title: 'Failed to save default executor profile',
          description: error instanceof Error ? error.message : undefined,
        });
        setDefaultExecutorProfileValue(previous);
      } finally {
        setIsUpdatingMilestone(false);
      }
    },
    [defaultExecutorProfileValue, refetchTaskGroup, taskGroup]
  );

  const handleAutomationModeToggle = useCallback(
    async (checked: boolean) => {
      if (!taskGroup) return;
      const nextMode: MilestoneAutomationMode = checked ? 'auto' : 'manual';
      const previous = automationModeValue;
      if (previous === nextMode) return;
      setAutomationModeValue(nextMode);
      setIsUpdatingMilestone(true);
      try {
        await taskGroupsApi.update(taskGroup.id, { automation_mode: nextMode });
        await refetchTaskGroup();
      } catch (error) {
        console.error('Failed to update milestone automation mode:', error);
        toast({
          variant: 'destructive',
          title: 'Failed to update automation',
          description: error instanceof Error ? error.message : undefined,
        });
        setAutomationModeValue(previous);
      } finally {
        setIsUpdatingMilestone(false);
      }
    },
    [automationModeValue, refetchTaskGroup, taskGroup]
  );

  const handleRunNextStep = useCallback(async () => {
    if (!taskGroup) return;
    setIsRequestingNextStep(true);
    try {
      const result = await taskGroupsApi.runNextStep(taskGroup.id);
      const candidateTitle = result.candidate_task_id
        ? (tasksById[result.candidate_task_id]?.title ??
          result.candidate_task_id)
        : null;
      const details =
        result.message ??
        (candidateTitle ? `Candidate: ${candidateTitle}` : undefined);

      toast({
        title:
          result.status === 'queued'
            ? 'Queued next step'
            : result.status === 'queued_waiting_for_active_attempt'
              ? 'Queued next step (waiting)'
              : 'No eligible next step',
        description: details,
        variant: result.status === 'not_eligible' ? 'destructive' : 'default',
        durationMs: result.status === 'not_eligible' ? 6500 : 4500,
      });
      await refetchTaskGroup();
    } catch (error) {
      console.error('Failed to request next milestone step:', error);
      toast({
        variant: 'destructive',
        title: 'Failed to queue next step',
        description: error instanceof Error ? error.message : undefined,
      });
    } finally {
      setIsRequestingNextStep(false);
    }
  }, [refetchTaskGroup, taskGroup, tasksById]);

  const handleEdgeLabelChange = useCallback(
    (value: string) => {
      setSelectedEdgeLabel(value);
      if (!selectedEdgeId) return;
      updateGraphDraft((prev) => ({
        ...prev,
        edges: prev.edges.map((edge) =>
          edge.id === selectedEdgeId
            ? {
                ...edge,
                data_flow: value.trim().length ? value : null,
              }
            : edge
        ),
      }));
    },
    [selectedEdgeId, updateGraphDraft]
  );

  const handleRemoveEdge = useCallback(() => {
    if (!selectedEdgeId) return;
    updateGraphDraft((prev) => ({
      ...prev,
      edges: prev.edges.filter((edge) => edge.id !== selectedEdgeId),
    }));
    setSelectedEdgeId(null);
  }, [selectedEdgeId, updateGraphDraft]);

  const handleRemoveNode = useCallback(
    (nodeId: string) => {
      if (masterNodeId && nodeId === masterNodeId) return;
      updateGraphDraft((prev) => ({
        ...prev,
        nodes: prev.nodes.filter((node) => node.id !== nodeId),
        edges: prev.edges.filter(
          (edge) => edge.from !== nodeId && edge.to !== nodeId
        ),
      }));
      setSelectedNodeId(null);
    },
    [masterNodeId, updateGraphDraft]
  );

  const resetNewNodeForm = useCallback(() => {
    setNewNodeMode('existing');
    setNewNodeTaskId(null);
    setNewNodeTitle('');
    setNewNodeDescription('');
    setNewNodeKind('task');
    setNewNodePhase(0);
  }, []);

  const handleAddNode = useCallback(async () => {
    if (!projectId) return;
    const phase = Number.isFinite(newNodePhase) ? newNodePhase : 0;
    setIsAddingNode(true);
    try {
      let taskId = newNodeTaskId;
      if (newNodeMode === 'new') {
        const trimmedTitle = newNodeTitle.trim();
        if (!trimmedTitle.length) return;
        const created = await tasksApi.create({
          project_id: projectId,
          title: trimmedTitle,
          description: newNodeDescription.trim().length
            ? newNodeDescription.trim()
            : null,
          status: null,
          task_kind: null,
          task_group_id: null,
          task_group_node_id: null,
          parent_workspace_id: null,
          origin_task_id: null,
          created_by_kind: null,
          image_ids: null,
          shared_task_id: null,
        });
        taskId = created.id;
      }

      if (!taskId) return;

      const position = fallbackPosition(graphNodes.length);
      const nodeId = createId('node');
      updateGraphDraft((prev) => ({
        ...prev,
        nodes: [
          ...prev.nodes,
          {
            id: nodeId,
            task_id: taskId,
            kind: newNodeKind,
            phase,
            executor_profile_id: config?.executor_profile ?? null,
            base_strategy: 'topology',
            instructions: null,
            requires_approval: newNodeKind === 'checkpoint',
            layout: { x: position.x, y: position.y },
          },
        ],
      }));

      setSelectedNodeId(nodeId);
      setIsAddNodeOpen(false);
      resetNewNodeForm();
    } catch (error) {
      console.error('Failed to add node:', error);
    } finally {
      setIsAddingNode(false);
    }
  }, [
    config,
    graphNodes.length,
    newNodeDescription,
    newNodeKind,
    newNodeMode,
    newNodePhase,
    newNodeTaskId,
    newNodeTitle,
    projectId,
    resetNewNodeForm,
    updateGraphDraft,
  ]);

  const handleStartNode = useCallback(() => {
    if (!selectedTask) return;
    CreateAttemptDialog.show({ taskId: selectedTask.id });
  }, [selectedTask]);

  const handleApproveNode = useCallback(async () => {
    if (!selectedTask) return;
    try {
      await tasksApi.update(selectedTask.id, {
        title: selectedTask.title,
        description: selectedTask.description,
        status: 'done',
        parent_workspace_id: selectedTask.parent_workspace_id,
        image_ids: null,
      });
      await refetchTaskGroup();
    } catch (error) {
      console.error('Failed to approve checkpoint:', error);
    }
  }, [refetchTaskGroup, selectedTask]);

  if (isTaskGroupLoading) {
    return (
      <Loader message={t('loading', 'Loading...')} size={32} className="py-8" />
    );
  }

  if (!taskGroup || !projectId) {
    return (
      <div className="p-6 text-muted-foreground">
        {t('taskGroup.workflowMissing', 'Workflow not found.')}
      </div>
    );
  }

  const descriptionContent = selectedTask?.description || '';
  const projectName = project?.name ?? 'Project';
  const canAddNode =
    newNodeMode === 'existing'
      ? Boolean(newNodeTaskId)
      : newNodeTitle.trim().length > 0;
  const panelTitle =
    panelView === 'chat'
      ? 'Chat'
      : selectedEdge
        ? 'Edge details'
        : isMasterSelected
          ? 'Milestone'
          : 'Node details';
  const selectedNodeLabel = selectedTask?.title || 'Task';
  const selectedNodeMeta = isMasterSelected
    ? 'Primary'
    : `${selectedKind.replace(/^[a-z]/, (char) => char.toUpperCase())}${
        typeof selectedGraphNode?.phase === 'number'
          ? ` · Phase ${selectedGraphNode.phase}`
          : ''
      }`;
  const canStartAttempt =
    Boolean(selectedTask) &&
    !isCheckpoint &&
    (!isGraphNodeSelected || isNodeReady);

  return (
    <div className="min-h-full h-full flex flex-col">
      <NewCardHeader
        actions={
          <div className="flex flex-wrap items-center gap-3">
            {isGraphDraftDirty && (
              <div className="flex flex-wrap items-center gap-2">
                <Badge variant="outline" className="text-xs">
                  Unsaved changes
                </Badge>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={handleDiscardGraphDraft}
                  disabled={isPersistingGraph}
                >
                  Discard
                </Button>
              </div>
            )}
            <div className="flex items-center gap-2">
              <Label className="text-xs text-muted-foreground">Baseline</Label>
              <Input
                value={baselineValue}
                onChange={(event) => setBaselineValue(event.target.value)}
                onBlur={handleBaselineSave}
                onKeyDown={handleBaselineKeyDown}
                className="h-8 w-[160px]"
                placeholder={t(
                  'taskFormDialog.baselinePlaceholder',
                  'e.g. main'
                )}
              />
            </div>
            <div className="flex items-center gap-2">
              <span className="text-xs text-muted-foreground">Status</span>
              <Select
                value={currentStatus}
                onValueChange={(value) =>
                  handleStatusChange(value as TaskStatus)
                }
                disabled={isUpdatingStatus}
              >
                <SelectTrigger className="h-8 w-[150px]">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {TASK_STATUSES.map((status) => (
                    <SelectItem key={status} value={status}>
                      {statusLabels[status]}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            {suggestedStatus && suggestedStatus !== currentStatus && (
              <div className="flex items-center gap-2">
                <Badge variant="outline" className="text-xs">
                  Suggested: {statusLabels[suggestedStatus]}
                </Badge>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={handleApplySuggested}
                  disabled={isUpdatingStatus}
                >
                  Apply
                </Button>
              </div>
            )}

            <Button
              size="sm"
              variant="outline"
              onClick={() => setIsAddNodeOpen(true)}
            >
              <Plus className="h-4 w-4 mr-1" />
              Add node
            </Button>

            {isPersistingGraph && (
              <Badge variant="outline" className="text-xs">
                Saving...
              </Badge>
            )}
          </div>
        }
      >
        <div className="min-w-0">
          <Breadcrumb>
            <BreadcrumbList>
              <BreadcrumbItem>
                <BreadcrumbLink
                  className="cursor-pointer hover:underline"
                  onClick={() => navigate(paths.projectTasks(projectId))}
                >
                  {projectName}
                </BreadcrumbLink>
              </BreadcrumbItem>
              <BreadcrumbSeparator />
              <BreadcrumbItem>
                <BreadcrumbPage>{taskGroup.title}</BreadcrumbPage>
              </BreadcrumbItem>
            </BreadcrumbList>
          </Breadcrumb>
          {taskGroup.description && (
            <div className="text-xs text-muted-foreground mt-1 line-clamp-2">
              {taskGroup.description}
            </div>
          )}
        </div>
      </NewCardHeader>

      {graphError && (
        <div className="px-4 pt-2 text-xs text-destructive">{graphError}</div>
      )}

      <div className="flex-1 min-h-0 p-4 flex flex-col lg:flex-row gap-4">
        <div className="flex-1 min-h-[320px] rounded-lg border bg-card overflow-hidden">
          <ReactFlowProvider>
            <div className="h-full w-full">
              <ReactFlow
                nodes={nodes}
                edges={edges}
                nodeTypes={nodeTypes}
                onNodesChange={handleNodesChange}
                onEdgesChange={handleEdgesChange}
                onConnect={handleConnect}
                onSelectionChange={(event) => {
                  if (event.nodes?.length) {
                    setSelectedNodeId(event.nodes[0].id);
                    setSelectedEdgeId(null);
                    return;
                  }
                  if (event.edges?.length) {
                    setSelectedEdgeId(event.edges[0].id);
                    setSelectedNodeId(null);
                    return;
                  }
                  // ReactFlow can emit an empty selection change when focus shifts
                  // (e.g. clicking UI controls). Keep the current selection unless the
                  // user explicitly clears it via pane clicks.
                }}
                onPaneClick={() => {
                  setSelectedNodeId(null);
                  setSelectedEdgeId(null);
                }}
                fitView
                minZoom={0.2}
                maxZoom={1.4}
                defaultEdgeOptions={defaultEdgeOptions}
                nodesConnectable={true}
                edgesReconnectable={false}
                zoomOnDoubleClick={false}
                deleteKeyCode={['Backspace', 'Delete']}
              >
                <Background gap={24} size={1} />
                <Controls position="top-right" />
                <MiniMap
                  nodeColor={(node: FlowNode) => {
                    const status = node.data?.status;
                    if (!status) return 'hsl(var(--border))';
                    return `hsl(var(${statusBoardColors[status]}))`;
                  }}
                  maskColor="hsl(var(--background) / 0.85)"
                />
              </ReactFlow>
            </div>
          </ReactFlowProvider>
        </div>

        <div className="w-full lg:w-[420px] min-h-0">
          <NewCard className="h-full min-h-0 border rounded-lg bg-card overflow-hidden">
            <NewCardHeader
              actions={
                <div className="flex items-center gap-1">
                  <Button
                    size="xs"
                    variant={panelView === 'chat' ? 'default' : 'outline'}
                    onClick={() => setPanelView('chat')}
                    disabled={Boolean(selectedEdgeId)}
                  >
                    Chat
                  </Button>
                  <Button
                    size="xs"
                    variant={panelView === 'details' ? 'default' : 'outline'}
                    onClick={() => setPanelView('details')}
                  >
                    Details
                  </Button>
                </div>
              }
            >
              <div className="text-sm font-semibold">{panelTitle}</div>
            </NewCardHeader>
            <NewCardContent className="flex-1 min-h-0">
              {panelView === 'chat' ? (
                !selectedTask ? (
                  <div className="p-4 text-sm text-muted-foreground">
                    {isMasterSelected
                      ? isTasksLoading
                        ? 'Loading primary task...'
                        : 'Primary task data is unavailable.'
                      : 'Select a node to open chat.'}
                  </div>
                ) : (
                  <div className="h-full min-h-0 flex flex-col">
                    <div className="p-4 border-b space-y-3">
                      <div className="flex items-start justify-between gap-2">
                        <div className="min-w-0">
                          <div className="text-sm font-semibold truncate">
                            {selectedNodeLabel}
                          </div>
                          <div className="text-xs text-muted-foreground">
                            {selectedNodeMeta}
                          </div>
                        </div>
                        <Badge variant="outline" className="text-[10px]">
                          {statusLabels[selectedTask.status]}
                        </Badge>
                      </div>

                      {isGraphNodeSelected && (
                        <>
                          <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                            {isPendingApproval && (
                              <Badge variant="outline" className="text-[10px]">
                                Pending approval
                              </Badge>
                            )}
                            {isNodeReady && !isPendingApproval && (
                              <Badge variant="outline" className="text-[10px]">
                                Ready
                              </Badge>
                            )}
                            {!isNodeReady && (
                              <Badge variant="outline" className="text-[10px]">
                                Blocked
                              </Badge>
                            )}
                          </div>

                          {!isNodeReady && blockedBy.length > 0 && (
                            <div className="text-xs text-muted-foreground">
                              Blocked by:{' '}
                              {blockedBy
                                .map(
                                  (item) =>
                                    `${item.label} (${statusLabels[item.status]})`
                                )
                                .join(', ')}
                            </div>
                          )}
                        </>
                      )}

                      <div className="flex flex-wrap gap-2">
                        {canStartAttempt && (
                          <Button
                            size="sm"
                            onClick={handleStartNode}
                            disabled={isGraphNodeSelected && !isNodeReady}
                          >
                            <Play className="h-4 w-4 mr-1" />
                            Start attempt
                          </Button>
                        )}
                        {isPendingApproval && (
                          <Button
                            size="sm"
                            variant="outline"
                            onClick={handleApproveNode}
                          >
                            <Check className="h-4 w-4 mr-1" />
                            Approve
                          </Button>
                        )}
                      </div>
                    </div>

                    <div className="flex-1 min-h-0">
                      {isAttemptsLoading ? (
                        <div className="p-4 text-sm text-muted-foreground">
                          Loading attempts...
                        </div>
                      ) : latestAttempt ? (
                        <ExecutionProcessesProvider
                          attemptId={latestAttempt.id}
                        >
                          <ClickedElementsProvider attempt={latestAttempt}>
                            <ReviewProvider attemptId={latestAttempt.id}>
                              <TaskAttemptPanel
                                attempt={latestAttempt}
                                task={selectedTask}
                              >
                                {({ logs, followUp }) => (
                                  <div className="h-full min-h-0 flex flex-col">
                                    <div className="flex-1 min-h-0">{logs}</div>
                                    <div className="min-h-0 max-h-[45%] border-t overflow-hidden bg-background">
                                      <div className="h-full min-h-0">
                                        {followUp}
                                      </div>
                                    </div>
                                  </div>
                                )}
                              </TaskAttemptPanel>
                            </ReviewProvider>
                          </ClickedElementsProvider>
                        </ExecutionProcessesProvider>
                      ) : (
                        <div className="p-4 text-sm text-muted-foreground">
                          No attempts yet for this task.
                        </div>
                      )}
                    </div>
                  </div>
                )
              ) : selectedEdge ? (
                <div className="p-4 space-y-3">
                  <div className="text-xs text-muted-foreground">
                    {selectedEdgeDetails?.fromLabel ?? selectedEdge.from} →{' '}
                    {selectedEdgeDetails?.toLabel ?? selectedEdge.to}
                  </div>
                  <div className="space-y-1">
                    <Label className="text-xs">Data flow label</Label>
                    <Input
                      value={selectedEdgeLabel}
                      onChange={(event) =>
                        handleEdgeLabelChange(event.target.value)
                      }
                      placeholder="e.g. API contract"
                    />
                  </div>
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={handleRemoveEdge}
                  >
                    <Trash2 className="h-4 w-4 mr-1" />
                    Remove edge
                  </Button>
                </div>
              ) : !selectedGraphNode ? (
                isMasterSelected ? (
                  <div className="p-4 space-y-5">
                    <div className="space-y-2">
                      <div className="flex items-center justify-between gap-2">
                        <div className="text-xs text-muted-foreground">
                          Progress
                        </div>
                        <div className="text-xs font-medium">
                          {milestoneProgress.done}/{milestoneProgress.total}{' '}
                          done
                        </div>
                      </div>
                      <div className="h-2 rounded-full bg-muted overflow-hidden">
                        <div
                          className="h-2 bg-foreground/70"
                          style={{ width: `${milestoneProgress.percent}%` }}
                        />
                      </div>
                      <div className="flex flex-wrap gap-2 text-[11px] text-muted-foreground">
                        {(['inprogress', 'inreview', 'todo'] as TaskStatus[])
                          .map((status) => ({
                            status,
                            count: milestoneProgress.counts[status],
                          }))
                          .filter((item) => item.count > 0)
                          .map((item) => (
                            <span key={item.status}>
                              {statusLabels[item.status]}: {item.count}
                            </span>
                          ))}
                        {milestoneProgress.missing > 0 && (
                          <span>Missing: {milestoneProgress.missing}</span>
                        )}
                      </div>
                      {milestoneProgress.activeAttemptTaskTitle && (
                        <div className="text-[11px] text-muted-foreground">
                          Active attempt:{' '}
                          {milestoneProgress.activeAttemptTaskTitle}
                        </div>
                      )}
                    </div>

                    <div className="space-y-2">
                      <div className="flex items-center justify-between">
                        <Label className="text-xs">Automation</Label>
                        <Switch
                          checked={automationModeValue === 'auto'}
                          onCheckedChange={handleAutomationModeToggle}
                          disabled={isUpdatingMilestone}
                        />
                      </div>
                      <div className="text-[11px] text-muted-foreground">
                        When enabled, the scheduler advances eligible nodes one
                        at a time. When disabled, use "Run next step".
                      </div>
                      <div className="flex items-center gap-2">
                        <Button
                          size="sm"
                          onClick={handleRunNextStep}
                          disabled={isRequestingNextStep || isUpdatingMilestone}
                        >
                          <Play className="h-4 w-4 mr-1" />
                          Run next step
                        </Button>
                        {taskGroup.run_next_step_requested_at && (
                          <Badge variant="outline" className="text-[10px]">
                            Queued
                          </Badge>
                        )}
                      </div>
                    </div>

                    <div className="space-y-2">
                      <Label className="text-xs">Default executor profile</Label>
                      <ExecutorProfileSelector
                        profiles={profiles}
                        selectedProfile={defaultExecutorProfileValue}
                        onProfileSelect={(profile) =>
                          handleDefaultExecutorProfileSave(profile)
                        }
                        showLabel={false}
                        className="gap-2"
                        itemClassName="min-w-0"
                        disabled={isUpdatingMilestone}
                      />
                      <div className="flex items-center gap-2">
                        <Button
                          size="xs"
                          variant="outline"
                          onClick={() => handleDefaultExecutorProfileSave(null)}
                          disabled={
                            isUpdatingMilestone || !defaultExecutorProfileValue
                          }
                        >
                          Clear
                        </Button>
                        {!defaultExecutorProfileValue && (
                          <div className="text-[11px] text-muted-foreground">
                            Defaults to each node's executor profile (or your
                            current profile).
                          </div>
                        )}
                      </div>
                    </div>

                    <div className="space-y-2">
                      <Label className="text-xs">Objective</Label>
                      <Textarea
                        value={objectiveValue}
                        onChange={(event) =>
                          setObjectiveValue(event.target.value)
                        }
                        onBlur={handleObjectiveSave}
                        placeholder="What does success look like?"
                        className="min-h-[80px]"
                        disabled={isUpdatingMilestone}
                      />
                    </div>

                    <div className="space-y-2">
                      <Label className="text-xs">Definition of done</Label>
                      <Textarea
                        value={definitionOfDoneValue}
                        onChange={(event) =>
                          setDefinitionOfDoneValue(event.target.value)
                        }
                        onBlur={handleDefinitionOfDoneSave}
                        placeholder="Acceptance criteria and completion checklist"
                        className="min-h-[90px]"
                        disabled={isUpdatingMilestone}
                      />
                    </div>
                  </div>
                ) : (
                  <div className="p-4 text-sm text-muted-foreground">
                    Select a node or edge to view details.
                  </div>
                )
              ) : !selectedTask ? (
                <div className="p-4 text-sm text-muted-foreground">
                  {isTasksLoading
                    ? 'Loading task details...'
                    : 'Linked task data is unavailable.'}
                </div>
              ) : (
                <div className="h-full min-h-0 flex flex-col">
                  <div className="p-4 border-b flex items-center justify-between gap-2">
                    <div className="text-sm font-semibold truncate">
                      {selectedNodeLabel}
                    </div>
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => handleRemoveNode(selectedGraphNode.id)}
                    >
                      <Trash2 className="h-4 w-4 mr-1" />
                      Remove node
                    </Button>
                  </div>

                  <div className="p-4 border-b space-y-3">
                    <div className="grid grid-cols-2 gap-3">
                      <div className="space-y-1">
                        <Label className="text-xs">Kind</Label>
                        <Select
                          value={selectedKind}
                          onValueChange={(value) =>
                            updateNode(selectedGraphNode.id, (node) => ({
                              ...node,
                              kind: value as TaskGroupNodeKind,
                              requires_approval:
                                value === 'checkpoint'
                                  ? true
                                  : (node.requires_approval ?? false),
                            }))
                          }
                        >
                          <SelectTrigger className="h-8">
                            <SelectValue />
                          </SelectTrigger>
                          <SelectContent>
                            <SelectItem value="task">Task</SelectItem>
                            <SelectItem value="checkpoint">
                              Checkpoint
                            </SelectItem>
                            <SelectItem value="merge">Merge</SelectItem>
                          </SelectContent>
                        </Select>
                      </div>
                      <div className="space-y-1">
                        <Label className="text-xs">Phase</Label>
                        <Input
                          type="number"
                          value={selectedGraphNode.phase ?? 0}
                          onChange={(event) => {
                            const value = Number.parseInt(
                              event.target.value,
                              10
                            );
                            updateNode(selectedGraphNode.id, (node) => ({
                              ...node,
                              phase: Number.isFinite(value) ? value : 0,
                            }));
                          }}
                        />
                      </div>
                    </div>

                    <div className="space-y-2">
                      <Label className="text-xs">Executor profile</Label>
                      <ExecutorProfileSelector
                        profiles={profiles}
                        selectedProfile={
                          selectedExecutorProfile ??
                          config?.executor_profile ??
                          null
                        }
                        onProfileSelect={(profile) =>
                          updateNode(selectedGraphNode.id, (node) => ({
                            ...node,
                            executor_profile_id: profile,
                          }))
                        }
                        showLabel={false}
                        className="gap-2"
                        itemClassName="min-w-0"
                      />
                      {!selectedExecutorProfile && config?.executor_profile && (
                        <div className="text-[11px] text-muted-foreground">
                          Defaults to your current profile unless set.
                        </div>
                      )}
                    </div>

                    <div className="space-y-1">
                      <Label className="text-xs">Base strategy</Label>
                      <Select
                        value={selectedBaseStrategy}
                        onValueChange={(value) =>
                          updateNode(selectedGraphNode.id, (node) => ({
                            ...node,
                            base_strategy: value as TaskGroupNodeBaseStrategy,
                          }))
                        }
                      >
                        <SelectTrigger className="h-8">
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="topology">Topology</SelectItem>
                          <SelectItem value="baseline">Baseline</SelectItem>
                        </SelectContent>
                      </Select>
                      <div className="text-[11px] text-muted-foreground">
                        {selectedBaseStrategy === 'topology'
                          ? 'Topology uses the most recent completed predecessor and falls back to the milestone baseline.'
                          : 'Baseline starts from the milestone baseline branch.'}
                      </div>
                    </div>

                    <div className="space-y-1">
                      <Label className="text-xs">Instructions</Label>
                      <Textarea
                        value={selectedGraphNode.instructions ?? ''}
                        onChange={(event) =>
                          updateNode(selectedGraphNode.id, (node) => ({
                            ...node,
                            instructions: event.target.value.trim().length
                              ? event.target.value
                              : null,
                          }))
                        }
                        placeholder="Optional node-specific guidance"
                        className="min-h-[90px]"
                      />
                    </div>

                    {isCheckpoint ? (
                      <div className="text-xs text-muted-foreground">
                        Checkpoint nodes require manual approval.
                      </div>
                    ) : (
                      <div className="flex items-center justify-between">
                        <Label className="text-xs">Requires approval</Label>
                        <Switch
                          checked={Boolean(requiresApproval)}
                          onCheckedChange={(checked) =>
                            updateNode(selectedGraphNode.id, (node) => ({
                              ...node,
                              requires_approval: checked,
                            }))
                          }
                        />
                      </div>
                    )}

                    {descriptionContent && (
                      <WYSIWYGEditor value={descriptionContent} disabled />
                    )}
                  </div>
                </div>
              )}
            </NewCardContent>
          </NewCard>
        </div>
      </div>

      <Dialog
        open={isAddNodeOpen}
        onOpenChange={(open) => {
          setIsAddNodeOpen(open);
          if (!open) {
            resetNewNodeForm();
          }
        }}
      >
        <DialogContent className="sm:max-w-[520px]">
          <DialogHeader>
            <DialogTitle>Add node</DialogTitle>
          </DialogHeader>
          <div className="space-y-4 py-2">
            <div className="flex flex-wrap gap-2">
              <Button
                size="sm"
                variant={newNodeMode === 'existing' ? 'default' : 'outline'}
                onClick={() => {
                  setNewNodeMode('existing');
                  setNewNodeTaskId(null);
                }}
              >
                Existing task
              </Button>
              <Button
                size="sm"
                variant={newNodeMode === 'new' ? 'default' : 'outline'}
                onClick={() => setNewNodeMode('new')}
              >
                New task
              </Button>
            </div>

            {newNodeMode === 'existing' ? (
              <div className="space-y-2">
                <Label className="text-xs">Task</Label>
                {availableTasks.length === 0 ? (
                  <div className="text-xs text-muted-foreground">
                    No available tasks to link.
                  </div>
                ) : (
                  <Select
                    value={newNodeTaskId ?? ''}
                    onValueChange={(value) => setNewNodeTaskId(value)}
                  >
                    <SelectTrigger className="h-9">
                      <SelectValue placeholder="Select a task" />
                    </SelectTrigger>
                    <SelectContent>
                      {availableTasks.map((task) => (
                        <SelectItem key={task.id} value={task.id}>
                          {task.title || task.id}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                )}
              </div>
            ) : (
              <div className="space-y-2">
                <div className="space-y-1">
                  <Label className="text-xs">Title</Label>
                  <Input
                    value={newNodeTitle}
                    onChange={(event) => setNewNodeTitle(event.target.value)}
                    placeholder="Task title"
                  />
                </div>
                <div className="space-y-1">
                  <Label className="text-xs">Description</Label>
                  <Textarea
                    value={newNodeDescription}
                    onChange={(event) =>
                      setNewNodeDescription(event.target.value)
                    }
                    placeholder="Optional details"
                    className="min-h-[90px]"
                  />
                </div>
              </div>
            )}

            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1">
                <Label className="text-xs">Kind</Label>
                <Select
                  value={newNodeKind}
                  onValueChange={(value) =>
                    setNewNodeKind(value as TaskGroupNodeKind)
                  }
                >
                  <SelectTrigger className="h-9">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="task">Task</SelectItem>
                    <SelectItem value="checkpoint">Checkpoint</SelectItem>
                    <SelectItem value="merge">Merge</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div className="space-y-1">
                <Label className="text-xs">Phase</Label>
                <Input
                  type="number"
                  value={newNodePhase}
                  onChange={(event) => {
                    const value = Number.parseInt(event.target.value, 10);
                    setNewNodePhase(Number.isFinite(value) ? value : 0);
                  }}
                />
              </div>
            </div>
          </div>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setIsAddNodeOpen(false)}
              disabled={isAddingNode}
            >
              Cancel
            </Button>
            <Button
              onClick={handleAddNode}
              disabled={!canAddNode || isAddingNode}
            >
              {isAddingNode ? 'Adding...' : 'Add node'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
