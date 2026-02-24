import { memo } from 'react';
import { Handle, Position, type Node, type NodeProps } from '@xyflow/react';
import { cn } from '@/lib/utils';
import { ProfileVariantBadge } from '@/components/common/ProfileVariantBadge';
import { Input } from '@/components/ui/input';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { statusBoardColors, statusLabels } from '@/utils/statusLabels';
import type { TaskStatus, ExecutorProfileId } from 'shared/types';
import type {
  TaskGroupNodeBaseStrategy,
  TaskGroupNodeKind,
} from '@/types/task-group';

export type TaskGroupNodeInlineUpdate = {
  kind?: TaskGroupNodeKind;
  phase?: number;
};

export type TaskGroupNodeData = {
  title: string;
  status?: TaskStatus;
  kind: TaskGroupNodeKind;
  taskId?: string;
  phase?: number;
  executorProfileId?: ExecutorProfileId | null;
  baseStrategy?: TaskGroupNodeBaseStrategy;
  requiresApproval?: boolean;
  isMaster?: boolean;
  onUpdate?: (update: TaskGroupNodeInlineUpdate) => void;
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
  const isMaster = Boolean(data.isMaster);

  return (
    <div
      className={cn(
        'rounded-lg border bg-background px-3 py-2 shadow-sm min-w-[180px] max-w-[260px]',
        selected && 'ring-2 ring-primary/40 border-primary/50',
        isMaster && 'border-primary/40 bg-primary/5'
      )}
    >
      {!isMaster && (
        <>
          <Handle
            type="target"
            position={Position.Left}
            className="!h-2 !w-2"
          />
          <Handle
            type="source"
            position={Position.Right}
            className="!h-2 !w-2"
          />
        </>
      )}

      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <div className="text-sm font-semibold truncate">
            {data.title || 'Untitled'}
          </div>
          <div className="text-[10px] uppercase tracking-[0.08em] text-muted-foreground">
            {isMaster ? 'Primary' : KIND_LABELS[data.kind]}
            {!isMaster &&
              typeof data.phase === 'number' &&
              ` · Phase ${data.phase}`}
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

      {selected && data.onUpdate && !isMaster && (
        <div className="mt-2 space-y-2 text-[10px] nodrag">
          <div className="grid grid-cols-2 gap-2">
            <div className="space-y-1">
              <div className="uppercase tracking-[0.08em] text-muted-foreground">
                Kind
              </div>
              <Select
                value={data.kind}
                onValueChange={(value) =>
                  data.onUpdate?.({ kind: value as TaskGroupNodeKind })
                }
              >
                <SelectTrigger className="h-6 text-[11px]">
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
              <div className="uppercase tracking-[0.08em] text-muted-foreground">
                Phase
              </div>
              <Input
                type="number"
                className="h-6 text-[11px]"
                value={data.phase ?? 0}
                onChange={(event) => {
                  const value = Number.parseInt(event.target.value, 10);
                  data.onUpdate?.({
                    phase: Number.isFinite(value) ? value : 0,
                  });
                }}
              />
            </div>
          </div>
        </div>
      )}

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
