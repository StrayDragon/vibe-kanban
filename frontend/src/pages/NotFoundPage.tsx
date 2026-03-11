import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';

import { NotFoundState } from '@/components/layout/NotFoundState';
import { paths } from '@/lib/paths';

export function NotFoundPage() {
  const navigate = useNavigate();
  const { t } = useTranslation('common');

  return (
    <NotFoundState
      primaryAction={{
        label: t('notFound.backToTasks', 'Back to tasks'),
        onClick: () => navigate(paths.overview()),
      }}
      secondaryAction={{
        label: t('buttons.back', 'Back'),
        onClick: () => navigate(-1),
      }}
    />
  );
}
