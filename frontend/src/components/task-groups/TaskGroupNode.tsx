import { memo } from 'react';
import { Handle, Position, type Node, type NodeProps } from '@xyflow/react';
import { cn } from '@/lib/utils';
import { ProfileVariantBadge } from '@/components/common/ProfileVariantBadge';
import { statusBoardColors, statusLabels } from '@/utils/statusLabels';
import type { TaskStatus, ExecutorProfileId } from 'shared/types';
import type { TaskGroupNodeBaseStrategy, TaskGroupNodeKind } from '@/types/task-group';

export type TaskGroupNodeData = {
  title: string;
  status?: TaskStatus;
  kind: TaskGroupNodeKind;
  taskId?: string;
  phase?: number;
  executorProfileId?: ExecutorProfileId | null;
  baseStrategy?: TaskGroupNodeBaseStrategy;
  requiresApproval?: boolean;
};

export type TaskGroupFlowNode = Node<TaskGroupNodeData, 'taskGroup'>;

const KIND_LABELS: Record<TaskGroupNodeKind, string> = {
  task: 'Task',
  checkpoint: 'Checkpoint',
  merge: 'Merge',
};

const BASE_STRATEGY_LABELS: Record<TaskGroupNodeBaseStrategy, string> = {
  topology: 'Topology',
  baseline: 'Baseline',
};

const TaskGroupNode = ({ data, selected }: NodeProps<TaskGroupFlowNode>) => {
  const statusColor = data.status ? statusBoardColors[data.status] : null;

  return (
    <div
      className={cn(
        'rounded-lg border bg-background px-3 py-2 shadow-sm min-w-[180px] max-w-[240px]',
        selected && 'ring-2 ring-primary/40 border-primary/50'
      )}
    >
      <Handle type="target" position={Position.Left} className="!h-2 !w-2" />
      <Handle type="source" position={Position.Right} className="!h-2 !w-2" />

      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <div className="text-sm font-semibold truncate">
            {data.title || 'Untitled'}
          </div>
          <div className="text-[10px] uppercase tracking-[0.08em] text-muted-foreground">
            {KIND_LABELS[data.kind]}
            {typeof data.phase === 'number' && ` · Phase ${data.phase}`}
          </div>
        </div>
        {data.status && (
          <span className="text-[10px] uppercase tracking-[0.06em] text-muted-foreground">
            <span
              className="inline-block h-2 w-2 rounded-full mr-1 align-middle"
              style={{
                backgroundColor: `hsl(var(${statusColor}))`,
              }}
            />
            {statusLabels[data.status]}
          </span>
        )}
      </div>

      {data.executorProfileId && (
        <ProfileVariantBadge
          profileVariant={data.executorProfileId}
          className="mt-2 block text-[11px] truncate"
        />
      )}

      {data.baseStrategy && (
        <div className="mt-1 text-[10px] uppercase tracking-[0.08em] text-muted-foreground">
          {BASE_STRATEGY_LABELS[data.baseStrategy]} base
        </div>
      )}

      {data.requiresApproval && (
        <div className="mt-2 text-[10px] text-muted-foreground uppercase tracking-[0.08em]">
          Approval required
        </div>
      )}
    </div>
  );
};

export default memo(TaskGroupNode);
