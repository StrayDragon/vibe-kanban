import { Link, useLocation } from 'react-router-dom';
import { useCallback } from 'react';
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
  BookOpen,
  MessageCircleQuestion,
  Menu,
  Plus,
} from 'lucide-react';
import { Logo } from '@/components/Logo';
import { SearchBar } from '@/components/SearchBar';
import { useSearch } from '@/contexts/SearchContext';
import { openTaskForm } from '@/lib/openTaskForm';
import { useProject } from '@/contexts/ProjectContext';
import { useOpenProjectInEditor } from '@/hooks/useOpenProjectInEditor';
import { OpenInIdeButton } from '@/components/ide/OpenInIdeButton';
import { useNavigateWithSearch, useProjectRepos } from '@/hooks';
import { ProjectFormDialog } from '@/components/dialogs/projects/ProjectFormDialog';
import { paths } from '@/lib/paths';

const EXTERNAL_LINKS = [
  {
    label: 'Docs',
    icon: BookOpen,
    href: 'https://vibekanban.com/docs',
  },
  {
    label: 'Support',
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
  const location = useLocation();
  const { projectId, project, projects, isLoading: projectsLoading } =
    useProject();
  const navigateWithSearch = useNavigateWithSearch();
  const { query, setQuery, active, clear, registerInputRef } = useSearch();
  const handleOpenInEditor = useOpenProjectInEditor(project || null);
  const isOverviewRoute = location.pathname.startsWith('/tasks');
  const isProjectTasksRoute = /^\/projects\/[^/]+\/tasks/.test(
    location.pathname
  );

  const { data: repos } = useProjectRepos(projectId);
  const isSingleRepoProject = repos?.length === 1;
  const showOpenInIde = Boolean(projectId && isSingleRepoProject);
  const showCreateTask = Boolean(projectId && !isOverviewRoute);
  const showProjectActions = showOpenInIde || showCreateTask;
  const hasProjects = projects.length > 0;
  const kanbanPath = projectId
    ? paths.projectTasks(projectId)
    : hasProjects
      ? paths.projectTasks(projects[0].id)
      : '/tasks';
  const switcherLabel =
    project?.name ??
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

  const handleOpenInIDE = () => {
    handleOpenInEditor();
  };

  const handleCreateProject = async () => {
    try {
      const result = await ProjectFormDialog.show({});
      if (result && result !== 'canceled') {
        navigateWithSearch(paths.projectTasks(result.id));
      }
    } catch (error) {
      // User cancelled - do nothing
    }
  };

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
                      {projects.map((item) => (
                        <DropdownMenuRadioItem key={item.id} value={item.id}>
                          {item.name}
                        </DropdownMenuRadioItem>
                      ))}
                    </DropdownMenuRadioGroup>
                  )}
                  <DropdownMenuSeparator />
                  <DropdownMenuItem onClick={handleCreateProject}>
                    <Plus className="h-4 w-4" />
                    {t('createProject')}
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
                  {showOpenInIde && (
                    <OpenInIdeButton
                      onClick={handleOpenInIDE}
                      className="h-9 w-9"
                    />
                  )}
                  {showCreateTask && (
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-9 w-9"
                      onClick={handleCreateTask}
                      aria-label="Create new task"
                    >
                      <Plus className="h-4 w-4" />
                    </Button>
                  )}
                </div>
                <NavDivider />
              </>
            ) : null}

            <div className="flex items-center gap-1">
              <Button
                variant="ghost"
                size="icon"
                className="h-9 w-9"
                asChild
                aria-label="Settings"
              >
                <Link
                  to={
                    projectId
                      ? `/settings/projects?projectId=${projectId}`
                      : '/settings'
                  }
                >
                  <Settings className="h-4 w-4" />
                </Link>
              </Button>

              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-9 w-9"
                    aria-label="Main navigation"
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
                      All Tasks
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
                        Kanbans
                      </Link>
                    </DropdownMenuItem>
                  ) : (
                    <DropdownMenuItem disabled>
                      <Kanban className="mr-2 h-4 w-4" />
                      Kanbans
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
                          {projects.map((item) => (
                            <DropdownMenuRadioItem key={item.id} value={item.id}>
                              {item.name}
                            </DropdownMenuRadioItem>
                          ))}
                        </DropdownMenuRadioGroup>
                      )}
                      <DropdownMenuItem
                        className="sm:hidden"
                        onClick={handleCreateProject}
                      >
                        <Plus className="h-4 w-4" />
                        {t('createProject')}
                      </DropdownMenuItem>
                    </>
                  )}

                  <DropdownMenuSeparator />

                  {EXTERNAL_LINKS.map((item) => {
                    const Icon = item.icon;
                    return (
                      <DropdownMenuItem key={item.href} asChild>
                        <a
                          href={item.href}
                          target="_blank"
                          rel="noopener noreferrer"
                        >
                          <Icon className="mr-2 h-4 w-4" />
                          {item.label}
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
