import { useMemo, useState } from 'react';
import { useMutation } from '@tanstack/react-query';
import { Copy, ExternalLink, Loader2, RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { toast } from '@/components/ui/toast';
import { useUserSystem } from '@/components/ConfigProvider';
import { useProjects } from '@/hooks/projects/useProjects';
import { configApi } from '@/lib/api';

function copyToClipboard(label: string, value: string) {
  void navigator.clipboard
    .writeText(value)
    .then(() => {
      toast({
        title: 'Copied',
        description: `${label} copied to clipboard.`,
      });
    })
    .catch((err) => {
      console.error('Failed to copy to clipboard:', err);
      toast({
        variant: 'destructive',
        title: 'Copy failed',
        description: `Could not copy ${label}.`,
      });
    });
}

function generateUuid(): string {
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) {
    return crypto.randomUUID();
  }

  // Fallback: RFC4122-ish random UUID (good enough for local config).
  const bytes = new Uint8Array(16);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = Math.floor(Math.random() * 256);
  }
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;

  const hex = Array.from(bytes, (b) => b.toString(16).padStart(2, '0'));
  return `${hex.slice(0, 4).join('')}-${hex.slice(4, 6).join('')}-${hex
    .slice(6, 8)
    .join('')}-${hex.slice(8, 10).join('')}-${hex.slice(10, 16).join('')}`;
}

function yamlString(value: string): string {
  return JSON.stringify(value);
}

export function ProjectSettings() {
  const { t } = useTranslation(['settings', 'common']);
  const { reloadSystem } = useUserSystem();
  const { projects, isLoading, error, isConnected } = useProjects();
  const [snippetProjectId, setSnippetProjectId] = useState(() => generateUuid());
  const [snippetName, setSnippetName] = useState('');
  const [snippetRepoPaths, setSnippetRepoPaths] = useState('');

  const reloadMutation = useMutation({
    mutationFn: configApi.reloadConfig,
    onSuccess: async () => {
      await reloadSystem();
      toast({
        title: 'Config reloaded',
        description: 'Reload succeeded.',
      });
    },
    onError: (err) => {
      console.error('Config reload failed:', err);
      toast({
        variant: 'destructive',
        title: 'Reload failed',
        description:
          err instanceof Error ? err.message : 'Config reload failed.',
      });
    },
  });

  const openProjectsYaml = async () => {
    try {
      const result = await configApi.openConfigTarget('projects_yaml');
      if (result.url) {
        window.open(result.url, '_blank', 'noopener,noreferrer');
      }
    } catch (err) {
      console.error('Failed to open projects.yaml:', err);
      toast({
        variant: 'destructive',
        title: 'Open failed',
        description:
          err instanceof Error ? err.message : 'Failed to open projects.yaml.',
      });
    }
  };

  const snippet = useMemo(() => {
    const name = snippetName.trim() || 'my-project';
    const repoPaths = snippetRepoPaths
      .split('\n')
      .map((line) => line.trim())
      .filter((line) => line.length > 0);

    const repoLines =
      repoPaths.length > 0
        ? repoPaths.map((path) => `    - path: ${yamlString(path)}`)
        : [`    - path: ${yamlString('/abs/path/to/repo')}`];

    return [
      '# Paste this under `projects:` in projects.yaml (create the file if missing)',
      `- id: ${snippetProjectId}`,
      `  name: ${yamlString(name)}`,
      '  repos:',
      ...repoLines,
      '',
    ].join('\n');
  }, [snippetName, snippetProjectId, snippetRepoPaths]);

  return (
    <div className="space-y-6">
      <Alert>
        <AlertTitle>
          {t('settings.projects.readOnlyTitle', 'Projects are file-configured')}
        </AlertTitle>
        <AlertDescription>
          {t(
            'settings.projects.readOnlyDescription',
            'Projects and repos are configured in projects.yaml (or projects.d/*.yaml). Editing via UI is disabled.'
          )}
        </AlertDescription>
      </Alert>

      <Card>
        <CardHeader>
          <CardTitle>{t('settings.projects.title', 'Projects')}</CardTitle>
          <CardDescription>
            {t(
              'settings.projects.description',
              'Edit projects.yaml on disk, then reload to apply changes.'
            )}
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-wrap items-center gap-2">
          <Button variant="outline" onClick={() => void openProjectsYaml()}>
            <ExternalLink className="mr-2 h-4 w-4" />
            {t('settings.config.open', 'Open projects.yaml')}
          </Button>
          <Button
            onClick={() => reloadMutation.mutate()}
            disabled={reloadMutation.isPending}
          >
            {reloadMutation.isPending ? (
              <>
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                {t('settings.config.reloading', 'Reloading')}
              </>
            ) : (
              <>
                <RefreshCw className="mr-2 h-4 w-4" />
                {t('settings.config.reload', 'Reload')}
              </>
            )}
          </Button>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>
            {t('settings.projects.snippetTitle', 'YAML snippet generator')}
          </CardTitle>
          <CardDescription>
            {t(
              'settings.projects.snippetDescription',
              'Generate a minimal projects entry, then paste it into projects.yaml and reload.'
            )}
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-2">
            <Label htmlFor="projectSnippetName">
              {t('settings.projects.snippetName', 'Project name')}
            </Label>
            <Input
              id="projectSnippetName"
              value={snippetName}
              placeholder="my-project"
              onChange={(e) => setSnippetName(e.target.value)}
            />
          </div>

          <div className="grid gap-2">
            <Label htmlFor="projectSnippetRepos">
              {t('settings.projects.snippetRepos', 'Repo paths (one per line)')}
            </Label>
            <Textarea
              id="projectSnippetRepos"
              value={snippetRepoPaths}
              placeholder="/abs/path/to/repo"
              rows={3}
              onChange={(e) => setSnippetRepoPaths(e.target.value)}
            />
            <div className="text-xs text-muted-foreground">
              {t(
                'settings.projects.snippetReposHint',
                'Repo paths must be absolute. VK will reject relative paths on reload.'
              )}
            </div>
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <Button
              variant="outline"
              onClick={() => setSnippetProjectId(generateUuid())}
            >
              {t('settings.projects.snippetNewId', 'New id')}
            </Button>
            <Button
              variant="secondary"
              onClick={() => copyToClipboard('YAML snippet', snippet)}
            >
              <Copy className="mr-2 h-4 w-4" />
              {t('settings.projects.snippetCopy', 'Copy snippet')}
            </Button>
          </div>

          <div className="rounded-md border border-border/60 bg-muted/40 p-3">
            <pre className="text-xs font-mono whitespace-pre-wrap break-words">
              {snippet}
            </pre>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>
            {t('settings.projects.configured', 'Configured projects')}
          </CardTitle>
          <CardDescription>
            {t(
              'settings.projects.configuredDescription',
              'Read-only view of projects currently loaded from projects.yaml / projects.d.'
            )}{' '}
            <span className="text-xs text-muted-foreground">
              {isConnected
                ? t('settings.projects.connected', 'Live')
                : t('settings.projects.disconnected', 'Disconnected')}
            </span>
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          {error && (
            <Alert variant="destructive">
              <AlertTitle>{t('common:error', 'Error')}</AlertTitle>
              <AlertDescription>{error.message}</AlertDescription>
            </Alert>
          )}

          {!error && isLoading && (
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <Loader2 className="h-4 w-4 animate-spin" />
              {t('settings.projects.loading', 'Loading')}
            </div>
          )}

          {!error && !isLoading && projects.length === 0 && (
            <div className="text-sm text-muted-foreground">
              {t(
                'settings.projects.empty',
                'No projects configured. Add a projects entry to projects.yaml and reload.'
              )}
            </div>
          )}

          {!error &&
            !isLoading &&
            projects.map((project) => (
              <div
                key={project.id}
                className="flex flex-col gap-1 rounded-md border border-border/60 p-3"
              >
                <div className="text-sm font-medium">{project.name}</div>
                <div className="text-xs text-muted-foreground font-mono break-all">
                  {project.id}
                </div>
              </div>
            ))}
        </CardContent>
      </Card>
    </div>
  );
}
