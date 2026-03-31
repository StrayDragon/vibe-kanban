import { Link, useLocation } from 'react-router-dom';
import { useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import {
  ChevronDown,
  Kanban,
  List,
  Settings,
  Archive,
  MessageCircleQuestion,
  Menu,
  Plus,
} from 'lucide-react';
import { Logo } from '@/components/Logo';
import { SearchBar } from '@/components/SearchBar';
import { TaskReviewInbox } from '@/components/tasks/TaskReviewInbox';
import { useSearch } from '@/contexts/SearchContext';
import { openTaskForm } from '@/lib/openTaskForm';
import { useProject } from '@/contexts/ProjectContext';
import { useNavigateWithSearch } from '@/hooks';
import { ArchiveKanbanDialog } from '@/components/dialogs';
import { AddProjectRepositoryDialog } from '@/components/dialogs';
import { paths } from '@/lib/paths';
import { uiIds } from '@/lib/uiIds';
import type { ProjectPublic } from 'shared/types';

const EXTERNAL_LINKS = [
  {
    labelKey: 'navigation.support',
    icon: MessageCircleQuestion,
    href: 'https://github.com/BloopAI/vibe-kanban/issues',
  },
];

function NavDivider() {
  return (
    <div
      className="mx-2 h-6 w-px bg-border/60"
      role="separator"
      aria-orientation="vertical"
    />
  );
}

export function Navbar() {
  const { t } = useTranslation('projects');
  const { t: tTasks } = useTranslation('tasks');
  const { t: tCommon } = useTranslation('common');
  const location = useLocation();
  const {
    projectId,
    project,
    projects,
    isLoading: projectsLoading,
  } = useProject();
  const navigateWithSearch = useNavigateWithSearch();
  const { query, setQuery, active, clear, registerInputRef, reviewInbox } =
    useSearch();
  const isOverviewRoute = location.pathname.startsWith('/tasks');
  const isProjectTasksRoute = /^\/projects\/[^/]+\/tasks/.test(
    location.pathname
  );
  const isProjectArchiveDetailRoute =
    /^\/projects\/[^/]+\/archives\/[^/]+\/?$/.test(location.pathname);

  const showCreateTask = Boolean(
    projectId && project && !isOverviewRoute && !isProjectArchiveDetailRoute
  );
  const showProjectActions = showCreateTask;
  const hasProjects = projects.length > 0;
  const kanbanPath = projectId
    ? paths.projectTasks(projectId)
    : hasProjects
      ? paths.projectTasks(projects[0].id)
      : '/tasks';
  const archivesPath = projectId
    ? paths.projectArchives(projectId)
    : hasProjects
      ? paths.projectArchives(projects[0].id)
      : '/tasks';
  const projectNameCounts = useMemo(() => {
    const counts = new Map<string, number>();
    projects.forEach((item) => {
      counts.set(item.name, (counts.get(item.name) ?? 0) + 1);
    });
    return counts;
  }, [projects]);

  const formatProjectLabel = useCallback(
    (item: ProjectPublic | null | undefined): string => {
      if (!item) return '';
      const needsDisambiguation = (projectNameCounts.get(item.name) ?? 0) > 1;
      if (!needsDisambiguation) return item.name;
      return `${item.name} · ${item.id.slice(0, 8)}`;
    },
    [projectNameCounts]
  );

  const unknownProjectLabel = projectId
    ? `Unknown project · ${projectId.slice(0, 8)}`
    : null;
  const switcherLabel =
    (project ? formatProjectLabel(project) : null) ??
    unknownProjectLabel ??
    (projectsLoading ? t('loading') : t('switcher.placeholder'));

  const setSearchBarRef = useCallback(
    (node: HTMLInputElement | null) => {
      registerInputRef(node);
    },
    [registerInputRef]
  );

  const handleCreateTask = () => {
    if (projectId) {
      openTaskForm({ mode: 'create', projectId });
    }
  };

  const handleArchiveKanban = async () => {
    if (!projectId) return;
    try {
      const result = await ArchiveKanbanDialog.show({ projectId });
      if (result?.archiveId) {
        navigateWithSearch(paths.projectArchive(projectId, result.archiveId));
      }
    } finally {
      ArchiveKanbanDialog.hide();
    }
  };

  const handleAddRepositoryByPath = async () => {
    if (!projectId) return;
    try {
      await AddProjectRepositoryDialog.show({
        projectId,
        projectName: project?.name,
      });
    } finally {
      AddProjectRepositoryDialog.hide();
    }
  };

  const renderProjectOption = (item: ProjectPublic) => (
    // Disambiguate same-name projects by showing a stable identifier.
    <DropdownMenuRadioItem
      key={item.id}
      value={item.id}
      className="gap-2 pr-1"
      onSelect={(event) => {
        const target = event.target as HTMLElement | null;
        if (target?.closest('[data-project-delete]')) {
          event.preventDefault();
        }
      }}
    >
      <span className="min-w-0 flex-1">
        <span className="block truncate">{item.name}</span>
        {(projectNameCounts.get(item.name) ?? 0) > 1 && (
          <span className="block truncate text-xs font-mono text-muted-foreground">
            {item.id}
          </span>
        )}
      </span>
    </DropdownMenuRadioItem>
  );

  return (
    <div className="border-b bg-background">
      <div className="w-full px-3">
        <div className="flex items-center h-12 py-2">
          <div className="flex-1 flex items-center">
            <Link to="/tasks">
              <Logo />
            </Link>
          </div>

          <div className="hidden sm:flex items-center gap-2">
            {isProjectTasksRoute && (
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button
                    variant="outline"
                    size="sm"
                    className="h-8 min-w-[140px] max-w-[220px] justify-between gap-2 bg-muted px-2"
                    aria-label={t('switcher.label')}
                  >
                    <span className="truncate">{switcherLabel}</span>
                    <ChevronDown className="h-4 w-4 opacity-70" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="start" className="w-56">
                  <DropdownMenuLabel>{t('switcher.label')}</DropdownMenuLabel>
                  <DropdownMenuSeparator />
                  {projectsLoading && projects.length === 0 ? (
                    <DropdownMenuItem disabled>{t('loading')}</DropdownMenuItem>
                  ) : projects.length === 0 ? (
                    <DropdownMenuItem disabled>
                      {t('switcher.empty')}
                    </DropdownMenuItem>
                  ) : (
                    <DropdownMenuRadioGroup
                      value={projectId ?? ''}
                      onValueChange={(value) => {
                        if (!value || value === projectId) return;
                        navigateWithSearch(paths.projectTasks(value));
                      }}
                    >
                      {projects.map(renderProjectOption)}
                    </DropdownMenuRadioGroup>
                  )}
                  <DropdownMenuSeparator />
                  <DropdownMenuItem
                    disabled={!projectId || !project}
                    onSelect={(event) => {
                      event.preventDefault();
                      void handleAddRepositoryByPath();
                    }}
                  >
                    {t('switcher.addRepository')}
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            )}
            <SearchBar
              ref={setSearchBarRef}
              className="shrink-0"
              value={query}
              onChange={setQuery}
              disabled={!active}
              onClear={clear}
              project={project || null}
            />
          </div>

          <div className="flex flex-1 items-center justify-end gap-1">
            {showProjectActions ? (
              <>
                <div className="flex items-center gap-1">
                  {isProjectTasksRoute && projectId && (
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-9 w-9"
                      onClick={() => void handleArchiveKanban()}
                      aria-label={tTasks('archives.archiveButton')}
                      title={tTasks('archives.archiveButton')}
                    >
                      <Archive className="h-4 w-4" />
                    </Button>
                  )}
                  {showCreateTask && (
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-9 w-9"
                      aria-label={tTasks('actions.createTask')}
                      title={tTasks('actions.createTask')}
                      id={uiIds.navbarCreateTask}
                      onClick={handleCreateTask}
                    >
                      <Plus className="h-4 w-4" />
                    </Button>
                  )}
                </div>
                <NavDivider />
              </>
            ) : null}

            <div className="flex items-center gap-1">
              {reviewInbox && reviewInbox.tasks.length > 0 && (
                <TaskReviewInbox
                  tasks={reviewInbox.tasks}
                  onSelectTask={reviewInbox.onSelectTask}
                  projectNames={reviewInbox.projectNames}
                  className="h-9 w-9"
                />
              )}

              <Button
                variant="ghost"
                size="icon"
                className="h-9 w-9"
                asChild
                aria-label={tCommon('navigation.settings')}
              >
                <Link to="/settings">
                  <Settings className="h-4 w-4" />
                </Link>
              </Button>

              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-9 w-9"
                    aria-label={tCommon('navigation.mainNavigation')}
                  >
                    <Menu className="h-4 w-4" />
                  </Button>
                </DropdownMenuTrigger>

                <DropdownMenuContent align="end">
                  <DropdownMenuItem
                    asChild
                    className={isOverviewRoute ? 'bg-accent' : ''}
                  >
                    <Link to="/tasks">
                      <List className="mr-2 h-4 w-4" />
                      {tTasks('overview.title')}
                    </Link>
                  </DropdownMenuItem>
                  {hasProjects ? (
                    <DropdownMenuItem
                      asChild
                      className={
                        location.pathname.startsWith('/projects')
                          ? 'bg-accent'
                          : ''
                      }
                    >
                      <Link to={kanbanPath}>
                        <Kanban className="mr-2 h-4 w-4" />
                        {tTasks('navigation.kanbans')}
                      </Link>
                    </DropdownMenuItem>
                  ) : (
                    <DropdownMenuItem disabled>
                      <Kanban className="mr-2 h-4 w-4" />
                      {tTasks('navigation.kanbans')}
                    </DropdownMenuItem>
                  )}
                  {hasProjects ? (
                    <DropdownMenuItem
                      asChild
                      className={
                        location.pathname.includes('/archives')
                          ? 'bg-accent'
                          : ''
                      }
                    >
                      <Link to={archivesPath}>
                        <Archive className="mr-2 h-4 w-4" />
                        {tTasks('archives.title')}
                      </Link>
                    </DropdownMenuItem>
                  ) : (
                    <DropdownMenuItem disabled>
                      <Archive className="mr-2 h-4 w-4" />
                      {tTasks('archives.title')}
                    </DropdownMenuItem>
                  )}
                  {isProjectTasksRoute && (
                    <>
                      <DropdownMenuSeparator className="sm:hidden" />
                      <DropdownMenuLabel className="sm:hidden">
                        {t('switcher.label')}
                      </DropdownMenuLabel>
                      {projectsLoading && projects.length === 0 ? (
                        <DropdownMenuItem className="sm:hidden" disabled>
                          {t('loading')}
                        </DropdownMenuItem>
                      ) : projects.length === 0 ? (
                        <DropdownMenuItem className="sm:hidden" disabled>
                          {t('switcher.empty')}
                        </DropdownMenuItem>
                      ) : (
                        <DropdownMenuRadioGroup
                          className="sm:hidden"
                          value={projectId ?? ''}
                          onValueChange={(value) => {
                            if (!value || value === projectId) return;
                            navigateWithSearch(paths.projectTasks(value));
                          }}
                        >
                          {projects.map(renderProjectOption)}
                        </DropdownMenuRadioGroup>
                      )}
                      <DropdownMenuSeparator className="sm:hidden" />
                      <DropdownMenuItem
                        className="sm:hidden"
                        disabled={!projectId || !project}
                        onSelect={(event) => {
                          event.preventDefault();
                          void handleAddRepositoryByPath();
                        }}
                      >
                        {t('switcher.addRepository')}
                      </DropdownMenuItem>
                    </>
                  )}

                  <DropdownMenuSeparator />

                  {EXTERNAL_LINKS.map((item) => {
                    const Icon = item.icon;
                    const label = tCommon(item.labelKey);
                    return (
                      <DropdownMenuItem key={item.href} asChild>
                        <a
                          href={item.href}
                          target="_blank"
                          rel="noopener noreferrer"
                        >
                          <Icon className="mr-2 h-4 w-4" />
                          {label}
                        </a>
                      </DropdownMenuItem>
                    );
                  })}
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
