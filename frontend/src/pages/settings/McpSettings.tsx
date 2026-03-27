import { Copy } from 'lucide-react';
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
import { useCopyToClipboard } from '@/hooks/utils/useCopyToClipboard';

const MCP_SNIPPET_JSON = `{
  "mcpServers": {
    "vibe_kanban": {
      "command": "mcp_task_server",
      "args": []
    }
  }
}
`;

const MCP_SNIPPET_CODEX_TOML = `[mcp_servers.vibe_kanban]
command = "mcp_task_server"
args = []
`;

export function McpSettings() {
  const { t } = useTranslation(['settings', 'common']);
  const copyToClipboard = useCopyToClipboard();

  return (
    <div className="space-y-6">
      <Alert>
        <AlertTitle>{t('settings.mcp.title', 'MCP')}</AlertTitle>
        <AlertDescription>
          {t(
            'settings.mcp.readOnlyDescription',
            'MCP configuration is managed in your executor config files on disk. Vibe Kanban does not read or display those files in the UI.'
          )}
        </AlertDescription>
      </Alert>

      <Card>
        <CardHeader>
          <CardTitle>
            {t('settings.mcp.howToTitle', 'How to configure Vibe Kanban MCP')}
          </CardTitle>
          <CardDescription>
            {t(
              'settings.mcp.howToDescription',
              'Add the following server entry to your executor MCP config.'
            )}
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <div className="text-sm font-medium">
              {t('settings.mcp.jsonSnippet', 'JSON snippet (most agents)')}
            </div>
            <div className="rounded-md border border-border/60 bg-muted/40 p-3">
              <pre className="text-xs whitespace-pre-wrap">
                {MCP_SNIPPET_JSON}
              </pre>
            </div>
            <Button
              variant="secondary"
              onClick={() =>
                copyToClipboard('MCP JSON snippet', MCP_SNIPPET_JSON)
              }
            >
              <Copy className="mr-2 h-4 w-4" />
              {t('common:buttons.copy', 'Copy')}
            </Button>
          </div>

          <div className="space-y-2">
            <div className="text-sm font-medium">
              {t('settings.mcp.codexSnippet', 'Codex snippet (TOML)')}
            </div>
            <div className="rounded-md border border-border/60 bg-muted/40 p-3">
              <pre className="text-xs whitespace-pre-wrap">
                {MCP_SNIPPET_CODEX_TOML}
              </pre>
            </div>
            <Button
              variant="secondary"
              onClick={() =>
                copyToClipboard(
                  'MCP Codex TOML snippet',
                  MCP_SNIPPET_CODEX_TOML
                )
              }
            >
              <Copy className="mr-2 h-4 w-4" />
              {t('common:buttons.copy', 'Copy')}
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
