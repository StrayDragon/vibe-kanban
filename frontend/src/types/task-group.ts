import type {
  ExecutorProfileId,
  MilestoneAutomationMode,
  TaskStatus,
  TaskWithAttemptStatus,
} from 'shared/types';

export type TaskGroupNodeKind = 'task' | 'checkpoint' | 'merge';

export type TaskGroupNodeBaseStrategy = 'topology' | 'baseline';

export type TaskGroupGraphNode = {
  id: string;
  task_id: string;
  kind: TaskGroupNodeKind;
  phase: number;
  executor_profile_id: ExecutorProfileId | null;
  base_strategy: TaskGroupNodeBaseStrategy;
  instructions: string | null;
  requires_approval: boolean | null;
  layout: { x: number; y: number };
  status?: TaskStatus | null;
};

export type TaskGroupGraphEdge = {
  id: string;
  from: string;
  to: string;
  data_flow: string | null;
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
  objective: string | null;
  definition_of_done: string | null;
  default_executor_profile_id: ExecutorProfileId | null;
  automation_mode: MilestoneAutomationMode;
  run_next_step_requested_at: string | null;
  status: TaskStatus;
  suggested_status: TaskStatus;
  baseline_ref: string;
  schema_version: number;
  graph: TaskGroupGraph;
  created_at: string;
  updated_at: string;
};

export type UpdateTaskGroup = {
  title?: string | null;
  description?: string | null;
  objective?: string | null;
  definition_of_done?: string | null;
  default_executor_profile_id?: ExecutorProfileId | null;
  automation_mode?: MilestoneAutomationMode | null;
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
