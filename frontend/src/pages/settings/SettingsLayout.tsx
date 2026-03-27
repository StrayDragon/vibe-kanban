import { useTranslation } from 'react-i18next';
import { X } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useEffect } from 'react';
import { useHotkeysContext } from 'react-hotkeys-hook';
import { useKeyExit } from '@/keyboard/hooks';
import { Scope } from '@/keyboard/registry';
import { usePreviousPath } from '@/hooks/utils/usePreviousPath';
import { GeneralSettings } from '@/pages/settings/GeneralSettings';
import { ProjectSettings } from '@/pages/settings/ProjectSettings';
import { McpSettings } from '@/pages/settings/McpSettings';
import { useLocation } from 'react-router-dom';

export function SettingsLayout() {
  const { t } = useTranslation('settings');
  const { enableScope, disableScope } = useHotkeysContext();
  const goToPreviousPath = usePreviousPath();
  const { hash } = useLocation();

  // Enable SETTINGS scope when component mounts
  useEffect(() => {
    enableScope(Scope.SETTINGS);
    return () => {
      disableScope(Scope.SETTINGS);
    };
  }, [enableScope, disableScope]);

  useEffect(() => {
    if (!hash) return;
    const id = hash.replace(/^#/, '');
    if (!id) return;

    requestAnimationFrame(() => {
      document.getElementById(id)?.scrollIntoView({ block: 'start' });
    });
  }, [hash]);

  // Register ESC keyboard shortcut
  useKeyExit(goToPreviousPath, { scope: Scope.SETTINGS });

  return (
    <div className="h-full overflow-auto">
      <div className="container mx-auto px-4 py-8">
        {/* Header with title and close button */}
        <div className="flex items-center justify-between sticky top-0 bg-background z-10 py-4 -mx-4 px-4">
          <h1 className="text-2xl font-semibold">
            {t('settings.layout.nav.title')}
          </h1>
          <Button
            variant="ghost"
            onClick={goToPreviousPath}
            className="h-8 px-2 rounded-none border border-foreground/20 hover:border-foreground/30 transition-all hover:opacity-100 focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 flex items-center gap-1.5"
          >
            <X className="h-4 w-4" />
            <span className="text-xs font-medium">ESC</span>
          </Button>
        </div>
        <div className="space-y-10">
          <section id="config" className="scroll-mt-24">
            <GeneralSettings />
          </section>
          <section id="projects" className="scroll-mt-24">
            <ProjectSettings />
          </section>
          <section id="mcp" className="scroll-mt-24">
            <McpSettings />
          </section>
        </div>
      </div>
    </div>
  );
}
