import { ListFilter, RotateCcw } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { cn } from '@/lib/utils';
import {
  ORCHESTRATION_LANES,
  type OrchestrationLane,
} from '@/utils/automation';

interface TaskOrchestrationFilterMenuProps {
  selectedFilters: OrchestrationLane[];
  onToggleFilter: (filter: OrchestrationLane) => void;
  onClearFilters: () => void;
  className?: string;
  compact?: boolean;
}

export function TaskOrchestrationFilterMenu({
  selectedFilters,
  onToggleFilter,
  onClearFilters,
  className,
  compact = false,
}: TaskOrchestrationFilterMenuProps) {
  const { t } = useTranslation(['tasks', 'common']);

  const filterLabel = (value: OrchestrationLane) => {
    switch (value) {
      case 'manual':
        return t('tasks:orchestration.filters.manual');
      case 'managed':
        return t('tasks:orchestration.filters.managed');
      case 'needs_review':
        return t('tasks:orchestration.filters.needsReview');
      case 'blocked':
        return t('tasks:orchestration.filters.blocked');
      default:
        return value;
    }
  };

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant={compact ? 'ghost' : 'outline'}
          size={compact ? 'icon' : 'sm'}
          className={cn(
            compact
              ? 'relative h-9 w-9'
              : 'h-8 shrink-0 gap-2 bg-muted px-2',
            selectedFilters.length > 0 && 'border-primary/40 text-foreground',
            className
          )}
          aria-label={t('tasks:orchestration.topbar.button')}
          title={t('tasks:orchestration.topbar.button')}
        >
          <ListFilter className="h-4 w-4" />
          {!compact && (
            <span className="text-xs font-medium">
              {t('tasks:orchestration.topbar.button')}
            </span>
          )}
          {selectedFilters.length > 0 && (
            <span
              className={cn(
                'inline-flex items-center justify-center rounded-full bg-primary text-primary-foreground text-[10px] font-semibold',
                compact ? 'absolute -right-1 -top-1 h-4 min-w-4 px-1' : 'h-4 min-w-4 px-1'
              )}
            >
              {selectedFilters.length}
            </span>
          )}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="w-56">
        <DropdownMenuLabel>
          {t('tasks:orchestration.topbar.menuTitle')}
        </DropdownMenuLabel>
        <DropdownMenuSeparator />
        <DropdownMenuItem
          onSelect={(event) => {
            event.preventDefault();
            onClearFilters();
          }}
          disabled={selectedFilters.length === 0}
        >
          <RotateCcw className="mr-2 h-4 w-4" />
          {t('common:reset')}
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        {ORCHESTRATION_LANES.map((filter) => (
          <DropdownMenuCheckboxItem
            key={filter}
            checked={selectedFilters.includes(filter)}
            onSelect={(event) => event.preventDefault()}
            onCheckedChange={() => onToggleFilter(filter)}
          >
            {filterLabel(filter)}
          </DropdownMenuCheckboxItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
