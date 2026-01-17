import type {
  ExecutorProfileId,
  TaskStatus,
  TaskWithAttemptStatus,
} from 'shared/types';

export type TaskGroupNodeKind = 'task' | 'checkpoint' | 'merge';

export type TaskGroupNodeBaseStrategy = 'topology' | 'baseline';

export type TaskGroupGraphNode = {
  id: string;
  task_id?: string;
  taskId?: string;
  kind?: TaskGroupNodeKind;
  phase?: number;
  executor_profile_id?: ExecutorProfileId | null;
  executorProfileId?: ExecutorProfileId | null;
  base_strategy?: TaskGroupNodeBaseStrategy;
  baseStrategy?: TaskGroupNodeBaseStrategy;
  instructions?: string | null;
  requires_approval?: boolean;
  requiresApproval?: boolean;
  layout?: { x?: number; y?: number };
};

export type TaskGroupGraphEdge = {
  id: string;
  from: string;
  to: string;
  type?: string;
  data_flow?: string;
  dataFlow?: string;
};

export type TaskGroupGraph = {
  schema_version?: number;
  nodes: TaskGroupGraphNode[];
  edges: TaskGroupGraphEdge[];
};

export type TaskGroup = {
  id: string;
  project_id: string;
  title: string;
  description: string | null;
  status: TaskStatus;
  suggested_status?: TaskStatus | null;
  suggestedStatus?: TaskStatus | null;
  baseline_ref?: string | null;
  schema_version: number;
  graph?: TaskGroupGraph;
  graph_json?: TaskGroupGraph;
  created_at: string;
  updated_at: string;
};

export type UpdateTaskGroup = {
  title?: string | null;
  description?: string | null;
  status?: TaskStatus;
  baseline_ref?: string | null;
  schema_version?: number | null;
  graph?: TaskGroupGraph;
};

export type TaskKind = 'default' | 'group';

export type TaskWithGroup = TaskWithAttemptStatus & {
  task_kind?: TaskKind;
  taskKind?: TaskKind;
  task_group_id?: string | null;
  taskGroupId?: string | null;
  task_group_node_id?: string | null;
  taskGroupNodeId?: string | null;
};
