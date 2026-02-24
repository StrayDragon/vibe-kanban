import { BrowserRouter } from 'react-router-dom';
import { I18nextProvider } from 'react-i18next';
import type { ComponentType, ReactNode } from 'react';

import i18n from '@/i18n';
import { UserSystemProvider } from '@/components/ConfigProvider';
import { ClickedElementsProvider } from '@/contexts/ClickedElementsProvider';
import { ProjectProvider } from '@/contexts/ProjectContext';
import { EventStreamProvider } from '@/contexts/EventStreamContext';

type ModalProviderProps = { children: ReactNode };

const DefaultModalProvider: ComponentType<ModalProviderProps> = ({
  children,
}) => <>{children}</>;

export function AppProviders({
  children,
  ModalProvider = DefaultModalProvider,
}: {
  children: ReactNode;
  ModalProvider?: ComponentType<ModalProviderProps>;
}) {
  return (
    <BrowserRouter>
      <UserSystemProvider>
        <ClickedElementsProvider>
          <ProjectProvider>
            <EventStreamProvider>
              <ModalProvider>
                <I18nextProvider i18n={i18n}>{children}</I18nextProvider>
              </ModalProvider>
            </EventStreamProvider>
          </ProjectProvider>
        </ClickedElementsProvider>
      </UserSystemProvider>
    </BrowserRouter>
  );
}
