import { memo, useMemo } from 'react';
import {
  type DragEndEvent,
  KanbanBoard,
  KanbanCards,
  KanbanHeader,
  KanbanProvider,
} from '@/components/ui/shadcn-io/kanban';
import { TaskCard } from './TaskCard';
import type { TaskStatus, TaskWithAttemptStatus } from 'shared/types';
import { statusBoardColors, statusLabels } from '@/utils/statusLabels';
import { getTaskGroupId, isTaskGroupEntry } from '@/utils/taskGroup';
export type KanbanColumnItem = TaskWithAttemptStatus;

export type KanbanColumns = Record<TaskStatus, KanbanColumnItem[]>;

type GroupedTask = {
  task: TaskWithAttemptStatus;
  index: number;
};

type ColumnGroup =
  | { type: 'task'; task: TaskWithAttemptStatus; index: number }
  | { type: 'group'; groupId: string; title: string; tasks: GroupedTask[] };

interface TaskKanbanBoardProps {
  columns: KanbanColumns;
  onDragEnd: (event: DragEndEvent) => void;
  onViewTaskDetails: (task: TaskWithAttemptStatus) => void;
  selectedTaskId?: string;
  onCreateTask?: () => void;
  projectId: string;
}

function TaskKanbanBoard({
  columns,
  onDragEnd,
  onViewTaskDetails,
  selectedTaskId,
  onCreateTask,
  projectId,
}: TaskKanbanBoardProps) {
  const taskGroupTitles = useMemo(() => {
    const map = new Map<string, string>();

    Object.values(columns).forEach((items) => {
      items.forEach((task) => {
        if (!isTaskGroupEntry(task)) return;
        const groupId = getTaskGroupId(task);
        if (!groupId || map.has(groupId)) return;
        map.set(groupId, task.title || 'Task Group');
      });
    });

    return map;
  }, [columns]);

  const buildColumnGroups = useMemo(() => {
    const result: Record<TaskStatus, ColumnGroup[]> = {
      todo: [],
      inprogress: [],
      inreview: [],
      done: [],
      cancelled: [],
    };

    Object.entries(columns).forEach(([status, items]) => {
      const columnItems: ColumnGroup[] = [];
      const groups = new Map<string, Extract<ColumnGroup, { type: 'group' }>>();

      items.forEach((task, index) => {
        const groupId = getTaskGroupId(task);
        if (!groupId) {
          columnItems.push({ type: 'task', task, index });
          return;
        }

        let group = groups.get(groupId);
        if (!group) {
          group = {
            type: 'group',
            groupId,
            title: taskGroupTitles.get(groupId) ?? 'Task Group',
            tasks: [],
          };
          groups.set(groupId, group);
          columnItems.push(group);
        }

        group.tasks.push({ task, index });
      });

      result[status as TaskStatus] = columnItems;
    });

    return result;
  }, [columns, taskGroupTitles]);

  return (
    <KanbanProvider onDragEnd={onDragEnd}>
      {Object.entries(columns).map(([status]) => {
        const statusKey = status as TaskStatus;
        const items = buildColumnGroups[statusKey] ?? [];
        return (
          <KanbanBoard key={status} id={statusKey}>
            <KanbanHeader
              name={statusLabels[statusKey]}
              color={statusBoardColors[statusKey]}
              onAddTask={onCreateTask}
            />
            <KanbanCards>
              {items.map((item) => {
                if (item.type === 'task') {
                  return (
                    <TaskCard
                      key={item.task.id}
                      task={item.task}
                      index={item.index}
                      status={statusKey}
                      onViewDetails={onViewTaskDetails}
                      isOpen={selectedTaskId === item.task.id}
                      projectId={projectId}
                    />
                  );
                }

                const sortedTasks = [...item.tasks].sort((a, b) => {
                  const aIsEntry = isTaskGroupEntry(a.task);
                  const bIsEntry = isTaskGroupEntry(b.task);
                  if (aIsEntry === bIsEntry) {
                    return a.index - b.index;
                  }
                  return aIsEntry ? -1 : 1;
                });
                const subtaskCount = sortedTasks.filter(
                  (entry) => !isTaskGroupEntry(entry.task)
                ).length;

                return (
                  <div key={`group-${item.groupId}`} className="flex flex-col">
                    <div className="flex flex-col">
                      {sortedTasks.map(({ task, index }) => {
                        const isSubtask = !isTaskGroupEntry(task);
                        const groupSummary = !isSubtask
                          ? { subtaskCount }
                          : undefined;
                        const content = (
                          <TaskCard
                            key={task.id}
                            task={task}
                            index={index}
                            status={statusKey}
                            onViewDetails={onViewTaskDetails}
                            isOpen={selectedTaskId === task.id}
                            projectId={projectId}
                            groupSummary={groupSummary}
                          />
                        );

                        if (!isSubtask) {
                          return content;
                        }

                        return (
                          <div
                            key={task.id}
                            className="pl-3 border-l border-muted-foreground/20"
                          >
                            {content}
                          </div>
                        );
                      })}
                    </div>
                  </div>
                );
              })}
            </KanbanCards>
          </KanbanBoard>
        );
      })}
    </KanbanProvider>
  );
}

export default memo(TaskKanbanBoard);
