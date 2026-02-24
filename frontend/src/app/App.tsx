import type { ComponentType, ReactNode } from 'react';

import { AppProviders } from '@app/AppProviders';
import { AppRouter } from '@app/AppRouter';

type ModalProviderProps = { children: ReactNode };

export default function App({
  ModalProvider,
}: {
  ModalProvider?: ComponentType<ModalProviderProps>;
}) {
  return (
    <AppProviders ModalProvider={ModalProvider}>
      <AppRouter />
    </AppProviders>
  );
}
