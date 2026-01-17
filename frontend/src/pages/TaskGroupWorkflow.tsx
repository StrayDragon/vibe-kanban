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
import { NewCard, NewCardContent, NewCardHeader } from '@/components/ui/new-card';
import { Loader } from '@/components/ui/loader';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { Textarea } from '@/components/ui/textarea';
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
import { useProjectTasks } from '@/hooks/useProjectTasks';
import { useTaskAttemptsWithSessions } from '@/hooks/useTaskAttempts';
import { useTaskGroup } from '@/hooks/useTaskGroup';
import { useDebouncedCallback } from '@/hooks/useDebouncedCallback';
import TaskAttemptPanel from '@/components/panels/TaskAttemptPanel';
import WYSIWYGEditor from '@/components/ui/wysiwyg';
import { CreateAttemptDialog } from '@/components/dialogs/tasks/CreateAttemptDialog';
import TaskGroupNode, {
  type TaskGroupFlowNode,
} from '@/components/task-groups/TaskGroupNode';
import { useUserSystem } from '@/components/ConfigProvider';
import { statusBoardColors, statusLabels } from '@/utils/statusLabels';
import { taskGroupsApi, tasksApi } from '@/lib/api';
import { paths } from '@/lib/paths';
import type {
  TaskGroupGraph,
  TaskGroupGraphEdge,
  TaskGroupGraphNode,
  TaskGroupNodeBaseStrategy,
  TaskGroupNodeKind,
} from '@/types/task-group';
import type { ExecutorProfileId, TaskStatus } from 'shared/types';

const TASK_STATUSES: TaskStatus[] = [
  'todo',
  'inprogress',
  'inreview',
  'done',
  'cancelled',
];

const getNodeTaskId = (node: TaskGroupGraphNode): string | undefined =>
  node.task_id ?? node.taskId;

const getNodeExecutorProfileId = (
  node: TaskGroupGraphNode
): ExecutorProfileId | null =>
  node.executor_profile_id ?? node.executorProfileId ?? null;

const getNodeBaseStrategy = (
  node: TaskGroupGraphNode
): TaskGroupNodeBaseStrategy =>
  node.base_strategy ?? node.baseStrategy ?? 'topology';

const getEdgeLabel = (edge: TaskGroupGraphEdge): string | undefined =>
  edge.data_flow ?? edge.dataFlow;

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

const normalizeGraph = (graph?: TaskGroupGraph | null): TaskGroupGraph => ({
  nodes: (graph?.nodes ?? []).map((node) => ({
    ...node,
    layout: { ...(node.layout ?? {}) },
  })),
  edges: (graph?.edges ?? []).map((edge) => ({ ...edge })),
});

type FlowNode = TaskGroupFlowNode;

type FlowEdge = Edge;

type NewNodeMode = 'existing' | 'new';

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
  const { tasks, tasksById, isLoading: isTasksLoading } = useProjectTasks(
    projectId ?? ''
  );

  const [graphDraft, setGraphDraft] = useState<TaskGroupGraph | null>(null);
  const [graphError, setGraphError] = useState<string | null>(null);
  const [isPersistingGraph, setIsPersistingGraph] = useState(false);
  const [selectedEdgeId, setSelectedEdgeId] = useState<string | null>(null);

  useEffect(() => {
    if (!taskGroup) {
      setGraphDraft(null);
      return;
    }
    setGraphDraft(normalizeGraph(taskGroup.graph ?? taskGroup.graph_json));
    setGraphError(null);
  }, [taskGroup]);

  const graph = useMemo(
    () => graphDraft ?? taskGroup?.graph ?? taskGroup?.graph_json ?? null,
    [graphDraft, taskGroup?.graph, taskGroup?.graph_json]
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
  const [isUpdatingStatus, setIsUpdatingStatus] = useState(false);
  const [statusValue, setStatusValue] = useState<TaskStatus | null>(null);
  const [baselineValue, setBaselineValue] = useState('');
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
      return graphNodes.map((node, index) => {
        const prevNode = prevMap.get(node.id);
        const taskId = getNodeTaskId(node);
        const task = taskId ? tasksById[taskId] : undefined;
        const layout = node.layout ?? {};
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
            kind: node.kind ?? 'task',
            taskId,
            phase: node.phase,
            executorProfileId: getNodeExecutorProfileId(node),
            baseStrategy: getNodeBaseStrategy(node),
            requiresApproval:
              node.requires_approval ??
              node.requiresApproval ??
              (node.kind ?? 'task') === 'checkpoint',
          },
        };
      });
    });
  }, [graph, graphEdges, graphNodes, setEdges, setNodes, tasksById]);

  useEffect(() => {
    if (selectedNodeId && !graphNodesById.has(selectedNodeId)) {
      setSelectedNodeId(null);
    }
  }, [graphNodesById, selectedNodeId]);

  useEffect(() => {
    if (selectedEdgeId && !graphEdges.find((edge) => edge.id === selectedEdgeId)) {
      setSelectedEdgeId(null);
    }
  }, [graphEdges, selectedEdgeId]);

  useEffect(() => {
    if (!selectedEdgeId) {
      setSelectedEdgeLabel('');
      return;
    }
    const edge = graphEdges.find((edge) => edge.id === selectedEdgeId);
    setSelectedEdgeLabel(edge ? getEdgeLabel(edge) ?? '' : '');
  }, [graphEdges, selectedEdgeId]);

  useEffect(() => {
    if (selectedEdgeId) return;
    if (!selectedNodeId && nodes.length > 0 && !hasAutoSelectedRef.current) {
      hasAutoSelectedRef.current = true;
      setSelectedNodeId(nodes[0].id);
    }
  }, [nodes, selectedEdgeId, selectedNodeId]);

  const selectedGraphNode = selectedNodeId
    ? graphNodesById.get(selectedNodeId)
    : null;
  const selectedTaskId = selectedGraphNode
    ? getNodeTaskId(selectedGraphNode)
    : undefined;
  const selectedTask = selectedTaskId ? tasksById[selectedTaskId] : undefined;

  const {
    data: selectedAttempts = [],
    isLoading: isAttemptsLoading,
  } = useTaskAttemptsWithSessions(selectedTask?.id, {
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
    ? graphEdges.find((edge) => edge.id === selectedEdgeId) ?? null
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

  const selectedKind = selectedGraphNode?.kind ?? 'task';
  const selectedExecutorProfile = selectedGraphNode
    ? getNodeExecutorProfileId(selectedGraphNode)
    : null;
  const selectedBaseStrategy = selectedGraphNode
    ? getNodeBaseStrategy(selectedGraphNode)
    : 'topology';
  const requiresApproval =
    selectedGraphNode?.requires_approval ??
    selectedGraphNode?.requiresApproval ??
    selectedKind === 'checkpoint';
  const isCheckpoint = selectedKind === 'checkpoint';
  const isNodeReady = blockedBy.length === 0;
  const isPendingApproval = Boolean(
    selectedGraphNode && requiresApproval && isNodeReady && selectedTask?.status !== 'done'
  );

  const suggestedStatus =
    taskGroup?.suggested_status ?? taskGroup?.suggestedStatus ?? null;
  const currentStatus = statusValue ?? taskGroup?.status ?? 'todo';

  const persistGraph = useCallback(
    async (nextGraph: TaskGroupGraph) => {
      if (!taskGroup) return;
      setIsPersistingGraph(true);
      setGraphError(null);
      try {
        await taskGroupsApi.update(taskGroup.id, { graph: nextGraph });
        await refetchTaskGroup();
      } catch (error) {
        console.error('Failed to update task group graph:', error);
        setGraphError('Failed to save workflow changes.');
        await refetchTaskGroup();
      } finally {
        setIsPersistingGraph(false);
      }
    },
    [refetchTaskGroup, taskGroup]
  );

  const { debounced: persistGraphDebounced } = useDebouncedCallback(
    persistGraph,
    600
  );

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

  const handleNodesChange = useCallback(
    (changes: NodeChange<FlowNode>[]) => {
      const nextNodes = applyNodeChanges(changes, nodesRef.current);
      setNodes(nextNodes);
      nodesRef.current = nextNodes;
      const removedIds = changes
        .filter((change) => change.type === 'remove')
        .map((change) => change.id);
      const hasPositionChange = changes.some(
        (change) => change.type === 'position' && !change.dragging
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
            nextNodes.map((node) => [node.id, node.position])
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
    [setNodes, updateGraphDraft]
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
        console.error('Failed to update task group status:', error);
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

  const updateNode = useCallback(
    (nodeId: string, updater: (node: TaskGroupGraphNode) => TaskGroupGraphNode) => {
      updateGraphDraft((prev) => ({
        ...prev,
        nodes: prev.nodes.map((node) =>
          node.id === nodeId ? updater(node) : node
        ),
      }));
    },
    [updateGraphDraft]
  );

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
                data_flow: value.trim().length ? value : undefined,
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
      updateGraphDraft((prev) => ({
        ...prev,
        nodes: prev.nodes.filter((node) => node.id !== nodeId),
        edges: prev.edges.filter(
          (edge) => edge.from !== nodeId && edge.to !== nodeId
        ),
      }));
      setSelectedNodeId(null);
    },
    [updateGraphDraft]
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

  return (
    <div className="min-h-full h-full flex flex-col">
      <NewCardHeader
        actions={
          <div className="flex flex-wrap items-center gap-3">
            <div className="flex items-center gap-2">
              <Label className="text-xs text-muted-foreground">Baseline</Label>
              <Input
                value={baselineValue}
                onChange={(event) => setBaselineValue(event.target.value)}
                onBlur={handleBaselineSave}
                onKeyDown={handleBaselineKeyDown}
                className="h-8 w-[160px]"
                placeholder={t('taskFormDialog.baselinePlaceholder', 'e.g. main')}
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
                  setSelectedNodeId(null);
                  setSelectedEdgeId(null);
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
            <NewCardHeader>
              <div className="text-sm font-semibold">
                {selectedEdge ? 'Edge details' : 'Node details'}
              </div>
            </NewCardHeader>
            <NewCardContent className="flex-1 min-h-0">
              {selectedEdge ? (
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
                  <Button size="sm" variant="outline" onClick={handleRemoveEdge}>
                    <Trash2 className="h-4 w-4 mr-1" />
                    Remove edge
                  </Button>
                </div>
              ) : !selectedGraphNode ? (
                <div className="p-4 text-sm text-muted-foreground">
                  Select a node or edge to view details.
                </div>
              ) : !selectedTask ? (
                <div className="p-4 text-sm text-muted-foreground">
                  {isTasksLoading
                    ? 'Loading task details...'
                    : 'Linked task data is unavailable.'}
                </div>
              ) : (
                <div className="h-full min-h-0 flex flex-col">
                  <div className="p-4 border-b space-y-3">
                    <div className="flex items-start justify-between gap-2">
                      <div className="min-w-0">
                        <div className="text-sm font-semibold truncate">
                          {selectedTask.title || 'Task'}
                        </div>
                        <div className="text-xs text-muted-foreground">
                          {selectedKind.replace(/^[a-z]/, (char) =>
                            char.toUpperCase()
                          )}
                          {typeof selectedGraphNode.phase === 'number'
                            ? ` · Phase ${selectedGraphNode.phase}`
                            : ''}
                        </div>
                      </div>
                      <Badge variant="outline" className="text-[10px]">
                        {statusLabels[selectedTask.status]}
                      </Badge>
                    </div>

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

                    <div className="flex flex-wrap gap-2">
                      {!isCheckpoint && (
                        <Button
                          size="sm"
                          onClick={handleStartNode}
                          disabled={!isNodeReady}
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
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() => handleRemoveNode(selectedGraphNode.id)}
                      >
                        <Trash2 className="h-4 w-4 mr-1" />
                        Remove node
                      </Button>
                    </div>
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
                                  : node.requires_approval ??
                                    node.requiresApproval ??
                                    false,
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
                          ? 'Topology uses the most recent completed predecessor and falls back to the task group baseline.'
                          : 'Baseline starts from the task group baseline branch.'}
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

                  <div className="flex-1 min-h-0">
                    {isAttemptsLoading ? (
                      <div className="p-4 text-sm text-muted-foreground">
                        Loading attempts...
                      </div>
                    ) : latestAttempt ? (
                      <ExecutionProcessesProvider attemptId={latestAttempt.id}>
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
            <Button onClick={handleAddNode} disabled={!canAddNode || isAddingNode}>
              {isAddingNode ? 'Adding...' : 'Add node'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
