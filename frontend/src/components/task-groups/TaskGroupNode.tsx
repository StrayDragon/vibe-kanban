import { memo } from 'react';
import { Handle, Position, type Node, type NodeProps } from '@xyflow/react';
import { cn } from '@/lib/utils';
import { statusBoardColors, statusLabels } from '@/utils/statusLabels';
import type { TaskStatus } from 'shared/types';
import type { TaskGroupNodeKind } from '@/types/task-group';

export type TaskGroupNodeData = {
  title: string;
  status?: TaskStatus;
  kind: TaskGroupNodeKind;
  taskId?: string;
  phase?: number;
  agentRole?: string;
  costEstimate?: string;
  artifacts?: string[];
};

export type TaskGroupFlowNode = Node<TaskGroupNodeData, 'taskGroup'>;

const KIND_LABELS: Record<TaskGroupNodeKind, string> = {
  task: 'Task',
  checkpoint: 'Checkpoint',
  merge: 'Merge',
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

      {(data.agentRole || data.costEstimate) && (
        <div className="mt-2 text-[11px] text-muted-foreground truncate">
          {data.agentRole ?? 'Unassigned'}
          {data.costEstimate ? ` · ${data.costEstimate}` : ''}
        </div>
      )}

      {data.artifacts && data.artifacts.length > 0 && (
        <div className="mt-2 text-[10px] text-muted-foreground truncate">
          {data.artifacts.join(', ')}
        </div>
      )}
    </div>
  );
};

export default memo(TaskGroupNode);
