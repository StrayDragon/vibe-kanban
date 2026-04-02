import type { ReactNode } from 'react';
import { cn } from '@/lib/utils';
import { Table } from '@/components/ui/table';

export function SettingsTable({
  children,
  className,
  wrapperClassName,
  tableClassName,
}: {
  children: ReactNode;
  className?: string;
  wrapperClassName?: string;
  tableClassName?: string;
}) {
  return (
    <div
      className={cn(
        'rounded-md border border-border/60 overflow-hidden',
        className
      )}
    >
      <div className={cn('overflow-x-auto', wrapperClassName)}>
        <Table className={cn('min-w-0 md:min-w-[720px]', tableClassName)}>
          {children}
        </Table>
      </div>
    </div>
  );
}
