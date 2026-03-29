import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App.tsx';
import './styles/index.css';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { HotkeysProvider } from 'react-hotkeys-hook';
// Import modal type definitions
import './types/modals';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 1000 * 60 * 5, // 5 minutes
      refetchOnWindowFocus: false,
    },
  },
});

const DevOnlyRoot = import.meta.env.DEV
  ? React.lazy(() => import('./dev/DevOnlyRoot'))
  : null;

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <HotkeysProvider initiallyActiveScopes={['*', 'global', 'kanban']}>
        {DevOnlyRoot ? (
          <React.Suspense fallback={null}>
            <DevOnlyRoot />
          </React.Suspense>
        ) : null}
        <App />
      </HotkeysProvider>
      {/*<TanStackDevtools plugins={[FormDevtoolsPlugin()]} />*/}
      {/* <ReactQueryDevtools initialIsOpen={false} /> */}
    </QueryClientProvider>
  </React.StrictMode>
);
