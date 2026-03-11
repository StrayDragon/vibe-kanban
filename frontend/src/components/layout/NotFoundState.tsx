import { FileSearch, Home } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';

type Action = {
  label: string;
  onClick: () => void;
};

interface NotFoundStateProps {
  title?: string;
  description?: string;
  primaryAction: Action;
  secondaryAction?: Action;
}

export function NotFoundState({
  title,
  description,
  primaryAction,
  secondaryAction,
}: NotFoundStateProps) {
  const { t } = useTranslation('common');

  return (
    <div className="flex min-h-[50vh] items-center justify-center p-6">
      <Card className="w-full max-w-xl border-dashed">
        <CardContent className="flex flex-col items-center gap-5 px-8 py-12 text-center">
          <div className="flex h-14 w-14 items-center justify-center rounded-full border bg-muted/50">
            <FileSearch className="h-7 w-7 text-muted-foreground" />
          </div>
          <div className="space-y-2">
            <div className="text-xs font-medium uppercase tracking-[0.3em] text-muted-foreground">
              {t('notFound.code', '404')}
            </div>
            <h1 className="text-2xl font-semibold">
              {title ?? t('notFound.title', 'Not found')}
            </h1>
            <p className="mx-auto max-w-md text-sm text-muted-foreground">
              {description ??
                t(
                  'notFound.description',
                  'The page or resource you requested could not be found.'
                )}
            </p>
          </div>
          <div className="flex flex-wrap items-center justify-center gap-3">
            <Button onClick={primaryAction.onClick}>
              <Home className="mr-2 h-4 w-4" />
              {primaryAction.label}
            </Button>
            {secondaryAction ? (
              <Button variant="outline" onClick={secondaryAction.onClick}>
                {secondaryAction.label}
              </Button>
            ) : null}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
