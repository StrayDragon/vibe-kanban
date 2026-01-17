import '@xyflow/react/dist/style.css';

import {
  Background,
  Controls,
  MiniMap,
  ReactFlow,
  ReactFlowProvider,
  useEdgesState,
  useNodesState,
  MarkerType,
  type Edge,
  type NodeChange,
  type NodeTypes,
} from '@xyflow/react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { NewCard, NewCardContent, NewCardHeader } from '@/components/ui/new-card';
import { Loader } from '@/components/ui/loader';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
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
import TaskGroupNode, {
  type TaskGroupFlowNode,
} from '@/components/task-groups/TaskGroupNode';
import { statusBoardColors, statusLabels } from '@/utils/statusLabels';
import { taskGroupsApi } from '@/lib/api';
import { paths } from '@/lib/paths';
import type {
  TaskGroupGraphEdge,
  TaskGroupGraphNode,
} from '@/types/task-group';
import type { TaskStatus } from 'shared/types';

const TASK_STATUSES: TaskStatus[] = [
  'todo',
  'inprogress',
  'inreview',
  'done',
  'cancelled',
];

const getNodeTaskId = (node: TaskGroupGraphNode): string | undefined =>
  node.task_id ?? node.taskId;

const getNodeAgentRole = (node: TaskGroupGraphNode): string | undefined =>
  node.agent_role ?? node.agentRole;

const getNodeCostEstimate = (node: TaskGroupGraphNode): string | undefined =>
  node.cost_estimate ?? node.costEstimate;

const getEdgeLabel = (edge: TaskGroupGraphEdge): string | undefined =>
  edge.data_flow ?? edge.dataFlow;

const fallbackPosition = (index: number) => ({
  x: (index % 4) * 260,
  y: Math.floor(index / 4) * 180,
});

type FlowNode = TaskGroupFlowNode;

type FlowEdge = Edge;

export function TaskGroupWorkflow() {
  const { t } = useTranslation('tasks');
  const navigate = useNavigate();
  const { projectId: routeProjectId, taskGroupId } = useParams<{
    projectId: string;
    taskGroupId: string;
  }>();
  const { projectId: contextProjectId, project } = useProject();
  const projectId = routeProjectId ?? contextProjectId;

  const {
    data: taskGroup,
    isLoading: isTaskGroupLoading,
    refetch: refetchTaskGroup,
  } = useTaskGroup(taskGroupId, { enabled: !!taskGroupId });
  const { tasksById, isLoading: isTasksLoading } = useProjectTasks(
    projectId ?? ''
  );

  const graph = useMemo(
    () => taskGroup?.graph ?? taskGroup?.graph_json ?? null,
    [taskGroup?.graph, taskGroup?.graph_json]
  );
  const graphNodes = useMemo(() => graph?.nodes ?? [], [graph]);
  const graphEdges = useMemo(() => graph?.edges ?? [], [graph]);

  const graphNodesById = useMemo(() => {
    const map = new Map<string, TaskGroupGraphNode>();
    graphNodes.forEach((node) => map.set(node.id, node));
    return map;
  }, [graphNodes]);

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

  const [nodes, setNodes, onNodesChange] = useNodesState<FlowNode>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<FlowEdge>([]);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [isUpdatingStatus, setIsUpdatingStatus] = useState(false);
  const [statusValue, setStatusValue] = useState<TaskStatus | null>(null);
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
            agentRole: getNodeAgentRole(node),
            costEstimate: getNodeCostEstimate(node),
            artifacts: node.artifacts ?? [],
          },
        };
      });
    });
  }, [graphEdges, graphNodes, setEdges, setNodes, taskGroup, tasksById]);

  useEffect(() => {
    if (selectedNodeId && !graphNodesById.has(selectedNodeId)) {
      setSelectedNodeId(null);
    }
  }, [graphNodesById, selectedNodeId]);

  useEffect(() => {
    if (!selectedNodeId && nodes.length > 0 && !hasAutoSelectedRef.current) {
      hasAutoSelectedRef.current = true;
      setSelectedNodeId(nodes[0].id);
    }
  }, [nodes, selectedNodeId]);

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

  const suggestedStatus =
    taskGroup?.suggested_status ?? taskGroup?.suggestedStatus ?? null;
  const currentStatus = statusValue ?? taskGroup?.status ?? 'todo';

  const persistLayout = useCallback(async () => {
    if (!taskGroup) return;
    const sourceGraph = taskGroup.graph ?? taskGroup.graph_json;
    if (!sourceGraph) return;
    const positions = new Map<string, FlowNode['position']>(
      nodesRef.current.map((node) => [node.id, node.position])
    );
    const updatedGraph = {
      ...sourceGraph,
      nodes: sourceGraph.nodes.map((node) => {
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

    try {
      await taskGroupsApi.update(taskGroup.id, { graph: updatedGraph });
    } catch (error) {
      console.error('Failed to persist task group layout:', error);
    }
  }, [taskGroup]);

  const { debounced: persistLayoutDebounced } = useDebouncedCallback(
    persistLayout,
    600
  );

  const handleNodesChange = useCallback(
    (changes: NodeChange<FlowNode>[]) => {
      onNodesChange(changes);
      const hasPositionChange = changes.some(
        (change) => change.type === 'position' && !change.dragging
      );
      if (hasPositionChange) {
        persistLayoutDebounced();
      }
    },
    [onNodesChange, persistLayoutDebounced]
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

  return (
    <div className="min-h-full h-full flex flex-col">
      <NewCardHeader
        actions={
          <div className="flex flex-wrap items-center gap-3">
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

      <div className="flex-1 min-h-0 p-4 flex flex-col lg:flex-row gap-4">
        <div className="flex-1 min-h-[320px] rounded-lg border bg-card overflow-hidden">
          <ReactFlowProvider>
            <div className="h-full w-full">
              <ReactFlow
                nodes={nodes}
                edges={edges}
                nodeTypes={nodeTypes}
                onNodesChange={handleNodesChange}
                onEdgesChange={onEdgesChange}
                onSelectionChange={({ nodes: selectedNodes }) =>
                  setSelectedNodeId(selectedNodes?.[0]?.id ?? null)
                }
                onPaneClick={() => setSelectedNodeId(null)}
                fitView
                minZoom={0.2}
                maxZoom={1.4}
                defaultEdgeOptions={defaultEdgeOptions}
                nodesConnectable={false}
                edgesReconnectable={false}
                zoomOnDoubleClick={false}
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
              <div className="text-sm font-semibold">Node details</div>
            </NewCardHeader>
            <NewCardContent className="flex-1 min-h-0">
              {!selectedGraphNode ? (
                <div className="p-4 text-sm text-muted-foreground">
                  Select a node to view its task details.
                </div>
              ) : !selectedTask ? (
                <div className="p-4 text-sm text-muted-foreground">
                  {isTasksLoading
                    ? 'Loading task details...'
                    : 'Linked task data is unavailable.'}
                </div>
              ) : (
                <div className="h-full min-h-0 flex flex-col">
                  <div className="p-4 border-b space-y-2">
                    <div className="flex items-start justify-between gap-2">
                      <div className="min-w-0">
                        <div className="text-sm font-semibold truncate">
                          {selectedTask.title || 'Task'}
                        </div>
                        <div className="text-xs text-muted-foreground">
                          {(selectedGraphNode.kind ?? 'task')
                            .replace(/^[a-z]/, (char) => char.toUpperCase())}
                          {typeof selectedGraphNode.phase === 'number'
                            ? ` · Phase ${selectedGraphNode.phase}`
                            : ''}
                        </div>
                      </div>
                      <Badge variant="outline" className="text-[10px]">
                        {statusLabels[selectedTask.status]}
                      </Badge>
                    </div>
                    {getNodeAgentRole(selectedGraphNode) && (
                      <div className="text-xs text-muted-foreground">
                        Role: {getNodeAgentRole(selectedGraphNode)}
                      </div>
                    )}
                    {getNodeCostEstimate(selectedGraphNode) && (
                      <div className="text-xs text-muted-foreground">
                        Estimate: {getNodeCostEstimate(selectedGraphNode)}
                      </div>
                    )}
                    {selectedGraphNode.artifacts?.length ? (
                      <div className="text-xs text-muted-foreground">
                        Artifacts: {selectedGraphNode.artifacts.join(', ')}
                      </div>
                    ) : null}
                    {selectedGraphNode.instructions && (
                      <div className="text-xs text-muted-foreground whitespace-pre-wrap">
                        {selectedGraphNode.instructions}
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
                                  <div className="flex-1 min-h-0">
                                    {logs}
                                  </div>
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
    </div>
  );
}
