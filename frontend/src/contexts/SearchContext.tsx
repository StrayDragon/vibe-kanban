import {
  createContext,
  useContext,
  useState,
  useEffect,
  useRef,
  useCallback,
  ReactNode,
} from 'react';
import { useLocation, useParams } from 'react-router-dom';
import type { TaskWithAttemptStatus } from 'shared/types';

import {
  type OrchestrationLane,
  normalizeOrchestrationFilters,
} from '@/utils/automation';

export interface ReviewInboxRegistration {
  tasks: TaskWithAttemptStatus[];
  onSelectTask: (task: TaskWithAttemptStatus) => void;
  projectNames?: Record<string, string>;
}

interface SearchState {
  query: string;
  setQuery: (query: string) => void;
  active: boolean;
  clear: () => void;
  focusInput: () => void;
  registerInputRef: (ref: HTMLInputElement | null) => void;
  orchestrationFilters: OrchestrationLane[];
  setOrchestrationFilters: (filters: OrchestrationLane[]) => void;
  toggleOrchestrationFilter: (filter: OrchestrationLane) => void;
  clearOrchestrationFilters: () => void;
  reviewInbox: ReviewInboxRegistration | null;
  setReviewInbox: (registration: ReviewInboxRegistration | null) => void;
  clearReviewInbox: () => void;
}

const SearchContext = createContext<SearchState | null>(null);

interface SearchProviderProps {
  children: ReactNode;
}

export function SearchProvider({ children }: SearchProviderProps) {
  const [query, setQuery] = useState('');
  const [orchestrationFilters, setOrchestrationFiltersState] = useState<
    OrchestrationLane[]
  >([]);
  const [reviewInbox, setReviewInboxState] =
    useState<ReviewInboxRegistration | null>(null);
  const location = useLocation();
  const { projectId } = useParams<{ projectId: string }>();
  const inputRef = useRef<HTMLInputElement | null>(null);

  const isProjectTasksRoute = /^\/projects\/[^/]+\/tasks/.test(
    location.pathname
  );
  const isOverviewTasksRoute = /^\/tasks/.test(location.pathname);
  const isTasksRoute = isProjectTasksRoute || isOverviewTasksRoute;

  useEffect(() => {
    if (
      !isTasksRoute &&
      (query !== '' || orchestrationFilters.length > 0 || reviewInbox !== null)
    ) {
      setQuery('');
      setOrchestrationFiltersState([]);
      setReviewInboxState(null);
    }
  }, [isTasksRoute, orchestrationFilters.length, query, reviewInbox]);

  useEffect(() => {
    if (isProjectTasksRoute) {
      setQuery('');
      setOrchestrationFiltersState([]);
    }
  }, [projectId, isProjectTasksRoute]);

  const clear = () => setQuery('');

  const focusInput = () => {
    if (inputRef.current && isTasksRoute) {
      inputRef.current.focus();
    }
  };

  const registerInputRef = useCallback((ref: HTMLInputElement | null) => {
    inputRef.current = ref;
  }, []);

  const setOrchestrationFilters = useCallback(
    (filters: OrchestrationLane[]) => {
      setOrchestrationFiltersState(normalizeOrchestrationFilters(filters));
    },
    []
  );

  const toggleOrchestrationFilter = useCallback((filter: OrchestrationLane) => {
    setOrchestrationFiltersState((prev) => {
      if (prev.includes(filter)) {
        return prev.filter((item) => item !== filter);
      }

      return normalizeOrchestrationFilters([...prev, filter]);
    });
  }, []);

  const clearOrchestrationFilters = useCallback(() => {
    setOrchestrationFiltersState([]);
  }, []);

  const setReviewInbox = useCallback((registration: ReviewInboxRegistration | null) => {
    setReviewInboxState(registration);
  }, []);

  const clearReviewInbox = useCallback(() => {
    setReviewInboxState(null);
  }, []);

  const value: SearchState = {
    query,
    setQuery,
    active: isTasksRoute,
    clear,
    focusInput,
    registerInputRef,
    orchestrationFilters,
    setOrchestrationFilters,
    toggleOrchestrationFilter,
    clearOrchestrationFilters,
    reviewInbox,
    setReviewInbox,
    clearReviewInbox,
  };

  return (
    <SearchContext.Provider value={value}>{children}</SearchContext.Provider>
  );
}

export function useSearch(): SearchState {
  const context = useContext(SearchContext);
  if (!context) {
    throw new Error('useSearch must be used within a SearchProvider');
  }
  return context;
}
