import { Bot, ListFilter, ShieldCheck } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { ProjectExecutionMode, TaskWithAttemptStatus } from 'shared/types';
import { Badge } from '@/components/ui/badge';
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group';
import {
  type OrchestrationFilter,
  getProjectExecutionModeLabel,
  summarizeTaskOrchestration,
} from '@/utils/automation';
import { cn } from '@/lib/utils';

interface TaskOrchestrationSummaryStripProps {
  tasks: TaskWithAttemptStatus[];
  filter?: OrchestrationFilter;
  onFilterChange?: (filter: OrchestrationFilter) => void;
  title: string;
  subtitle: string;
  projectExecutionMode?: ProjectExecutionMode;
  className?: string;
  showFilters?: boolean;
}

const FILTERS: Array<{
  value: OrchestrationFilter;
  countKey: keyof ReturnType<typeof summarizeTaskOrchestration>;
}> = [
  { value: 'all', countKey: 'all' },
  { value: 'manual', countKey: 'manual' },
  { value: 'managed', countKey: 'managed' },
  { value: 'needs_review', countKey: 'needs_review' },
  { value: 'blocked', countKey: 'blocked' },
];

export function TaskOrchestrationSummaryStrip({
  tasks,
  filter,
  onFilterChange,
  title,
  subtitle,
  projectExecutionMode,
  className,
  showFilters = true,
}: TaskOrchestrationSummaryStripProps) {
  const { t } = useTranslation('tasks');
  const summary = summarizeTaskOrchestration(tasks);

  const filterLabel = (value: OrchestrationFilter) => {
    switch (value) {
      case 'manual':
        return t('orchestration.filters.manual');
      case 'managed':
        return t('orchestration.filters.managed');
      case 'needs_review':
        return t('orchestration.filters.needsReview');
      case 'blocked':
        return t('orchestration.filters.blocked');
      case 'all':
      default:
        return t('orchestration.filters.all');
    }
  };

  return (
    <div
      className={cn(
        'rounded-xl border bg-card/95 px-4 py-4 shadow-sm',
        className
      )}
    >
      <div className="space-y-2">
        <div className="flex flex-wrap items-center gap-2">
          <div className="flex items-center gap-2">
            <ShieldCheck className="h-4 w-4 text-muted-foreground" />
            <h2 className="text-sm font-semibold tracking-tight">{title}</h2>
          </div>
          {projectExecutionMode && (
            <Badge variant="outline" className="gap-1.5">
              <Bot className="h-3.5 w-3.5" />
              {getProjectExecutionModeLabel(projectExecutionMode)}
            </Badge>
          )}
        </div>
        <p className="text-xs text-muted-foreground">{subtitle}</p>
      </div>

      {showFilters && filter && onFilterChange && (
        <div className="mt-4 flex flex-col gap-2 sm:flex-row sm:flex-wrap sm:items-center">
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <ListFilter className="h-3.5 w-3.5" />
            {t('orchestration.filterLabel', { defaultValue: 'Filter' })}
          </div>
          <ToggleGroup
            type="single"
            value={filter}
            onValueChange={(value) => {
              if (!value) return;
              onFilterChange(value as OrchestrationFilter);
            }}
            className="flex flex-wrap justify-start gap-2"
          >
            {FILTERS.map(({ value, countKey }) => (
              <ToggleGroupItem
                key={value}
                value={value}
                className="inline-flex shrink-0 items-center gap-1.5 rounded-full border border-border/70 px-3 text-xs whitespace-nowrap text-foreground/80 hover:bg-muted/70 data-[state=on]:border-primary data-[state=on]:bg-primary data-[state=on]:text-primary-foreground"
                style={{
                  width: 'fit-content',
                  minWidth: 'fit-content',
                  height: '2rem',
                }}
                aria-label={t('orchestration.filterAria', {
                  label: filterLabel(value),
                  defaultValue: `Filter ${filterLabel(value)} tasks`,
                })}
              >
                <span>{filterLabel(value)}</span>
                <span className="rounded-full bg-muted px-1.5 py-0.5 text-[10px] leading-none text-foreground/70 data-[state=on]:bg-primary-foreground/15 data-[state=on]:text-primary-foreground">
                  {summary[countKey]}
                </span>
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
        </div>
      )}
    </div>
  );
}
