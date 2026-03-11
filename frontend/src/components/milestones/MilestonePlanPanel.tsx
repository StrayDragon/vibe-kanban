import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Textarea } from '@/components/ui/textarea';
import { toast } from '@/components/ui/toast';
import { ConfirmDialog } from '@/components/dialogs';
import { CreateAttemptDialog } from '@/components/dialogs/tasks/CreateAttemptDialog';
import { ExecutionProcessesProvider } from '@/contexts/ExecutionProcessesContext';
import { ClickedElementsProvider } from '@/contexts/ClickedElementsProvider';
import { ReviewProvider } from '@/contexts/ReviewProvider';
import TaskAttemptPanel from '@/components/panels/TaskAttemptPanel';

import { useTaskAttemptsWithSessions } from '@/hooks/task-attempts/useTaskAttempts';
import { milestonesApi, sessionsApi } from '@/lib/api';

import type { WorkspaceWithSession } from '@/types/attempt';
import type {
  Milestone,
  MilestonePlanApplyResponse,
  MilestonePlanPreviewResponse,
  MilestonePlanV1,
  TaskWithAttemptStatus,
} from 'shared/types';

type ParseResult =
  | {
      ok: true;
      plan: MilestonePlanV1;
      extractedJson: string | null;
    }
  | {
      ok: false;
      error: string;
      extractedJson: string | null;
    };

function extractFencedJson(input: string): string | null {
  const patterns = [
    /```milestone-plan-v1\s*([\s\S]*?)```/i,
    /```milestone-plan\s*([\s\S]*?)```/i,
    /```json\s*([\s\S]*?)```/i,
  ];
  for (const pattern of patterns) {
    const match = input.match(pattern);
    if (!match) continue;
    const candidate = match[1]?.trim();
    if (candidate) return candidate;
  }
  return null;
}

function parsePlanText(raw: string): ParseResult | null {
  const trimmed = raw.trim();
  if (!trimmed) return null;

  let jsonText = trimmed;
  let extractedJson: string | null = null;

  if (!trimmed.startsWith('{') && !trimmed.startsWith('[')) {
    extractedJson = extractFencedJson(trimmed);
    if (extractedJson) {
      jsonText = extractedJson;
    }
  }

  try {
    const parsed = JSON.parse(jsonText) as MilestonePlanV1;
    if (!parsed || typeof parsed !== 'object') {
      return { ok: false, error: 'Plan payload must be a JSON object.', extractedJson };
    }
    return { ok: true, plan: parsed, extractedJson };
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    return { ok: false, error: `Invalid JSON: ${msg}`, extractedJson };
  }
}

function prettyJson(value: unknown): string {
  return `${JSON.stringify(value, null, 2)}\n`;
}

function autoWireEdges(plan: MilestonePlanV1): MilestonePlanV1 {
  const nodes = [...(plan.nodes ?? [])].slice();
  nodes.sort((a, b) => {
    const ap = typeof a.phase === 'number' ? a.phase : 0;
    const bp = typeof b.phase === 'number' ? b.phase : 0;
    if (ap !== bp) return ap - bp;
    return String(a.id ?? '').localeCompare(String(b.id ?? ''));
  });

  const edges = [];
  for (let i = 0; i < nodes.length - 1; i += 1) {
    const from = String(nodes[i]?.id ?? '').trim();
    const to = String(nodes[i + 1]?.id ?? '').trim();
    if (!from || !to) continue;
    edges.push({ from, to, data_flow: null });
  }

  return { ...plan, edges };
}

export function MilestonePlanPanel({
  milestone,
  entryTask,
  onMilestoneUpdated,
}: {
  milestone: Milestone;
  entryTask: TaskWithAttemptStatus | null;
  onMilestoneUpdated: (updated: Milestone) => void;
}) {
  const { t } = useTranslation('tasks');

  const [planText, setPlanText] = useState('');
  const parsed = useMemo(() => parsePlanText(planText), [planText]);

  const [preview, setPreview] = useState<MilestonePlanPreviewResponse | null>(
    null
  );
  const [isPreviewing, setIsPreviewing] = useState(false);
  const [isDetecting, setIsDetecting] = useState(false);

  const [isApplying, setIsApplying] = useState(false);
  const [applyKey, setApplyKey] = useState<string | null>(null);

  useEffect(() => {
    setPreview(null);
    setApplyKey(null);
  }, [planText]);

  const canPreview = Boolean(parsed && parsed.ok);
  const canApply = Boolean(preview && parsed && parsed.ok);

  const handleAutoWire = useCallback(() => {
    if (!parsed || !parsed.ok) return;
    if ((parsed.plan.edges?.length ?? 0) > 0) {
      toast({
        variant: 'destructive',
        title: t('common:states.error', 'Error'),
        description: 'Edges already exist in this plan.',
      });
      return;
    }
    const next = autoWireEdges(parsed.plan);
    setPlanText(prettyJson(next));
  }, [parsed, t]);

  const handlePreview = useCallback(async () => {
    if (!parsed || !parsed.ok) return;
    setIsPreviewing(true);
    try {
      const result = await milestonesApi.previewPlan(milestone.id, parsed.plan);
      setPreview(result);
    } catch (err) {
      console.error('Failed to preview plan:', err);
      toast({
        variant: 'destructive',
        title: t('common:states.error', 'Error'),
        description: err instanceof Error ? err.message : 'Failed to preview plan',
      });
    } finally {
      setIsPreviewing(false);
    }
  }, [milestone.id, parsed, t]);

  const handleApply = useCallback(async () => {
    if (!parsed || !parsed.ok) return;
    if (!preview) return;

    const confirmed = await ConfirmDialog.show({
      title: 'Apply milestone plan?',
      message:
        'This will create any missing tasks and replace the milestone graph in a single transaction.',
      confirmText: 'Apply plan',
      cancelText: 'Cancel',
    });
    if (confirmed !== 'confirmed') return;

    const idempotencyKey = applyKey ?? crypto.randomUUID();
    if (!applyKey) setApplyKey(idempotencyKey);

    setIsApplying(true);
    try {
      const result: MilestonePlanApplyResponse = await milestonesApi.applyPlan(
        milestone.id,
        parsed.plan,
        { idempotencyKey }
      );
      onMilestoneUpdated(result.milestone);
      toast({
        title: 'Plan applied',
        description: `Created ${result.created_tasks.length} task(s).`,
      });
      setPlanText(prettyJson(parsed.plan));
    } catch (err) {
      console.error('Failed to apply plan:', err);
      toast({
        variant: 'destructive',
        title: t('common:states.error', 'Error'),
        description: err instanceof Error ? err.message : 'Failed to apply plan',
      });
    } finally {
      setIsApplying(false);
    }
  }, [applyKey, milestone.id, onMilestoneUpdated, parsed, preview, t]);

  const handleStartGuideAttempt = useCallback(() => {
    if (!entryTask?.id) return;
    CreateAttemptDialog.show({
      taskId: entryTask.id,
      promptPreset: 'milestone_planning',
      onCreated: () => {
        toast({
          title: 'Guide attempt started',
        });
      },
    });
  }, [entryTask?.id]);

  const { data: guideAttempts = [], isLoading: isGuideAttemptsLoading } =
    useTaskAttemptsWithSessions(entryTask?.id, {
      enabled: Boolean(entryTask?.id),
    });

  const latestGuideAttempt: WorkspaceWithSession | undefined = useMemo(() => {
    if (!guideAttempts.length) return undefined;
    return [...guideAttempts].sort((a, b) => {
      const diff =
        new Date(b.created_at).getTime() - new Date(a.created_at).getTime();
      if (diff !== 0) return diff;
      return a.id.localeCompare(b.id);
    })[0];
  }, [guideAttempts]);

  const handleLoadFromGuide = useCallback(async () => {
    const sessionId = latestGuideAttempt?.session?.id;
    if (!sessionId) return;

    setIsDetecting(true);
    try {
      const page = await sessionsApi.getMessages(sessionId, { limit: 200 });
      const entries = page.entries ?? [];

      for (let i = entries.length - 1; i >= 0; i -= 1) {
        const entry = entries[i];
        const candidate = entry?.summary ?? entry?.prompt ?? '';
        const parsedCandidate = parsePlanText(candidate);
        if (parsedCandidate && parsedCandidate.ok) {
          setPlanText(prettyJson(parsedCandidate.plan));
          toast({
            title: 'Plan loaded',
            description: `Detected a plan from guide turn ${entry.turn_id}.`,
          });
          return;
        }
      }

      toast({
        variant: 'destructive',
        title: t('common:states.error', 'Error'),
        description:
          'No milestone plan block detected in the latest guide output.',
      });
    } catch (err) {
      console.error('Failed to load plan from guide output:', err);
      toast({
        variant: 'destructive',
        title: t('common:states.error', 'Error'),
        description: err instanceof Error ? err.message : 'Failed to load plan',
      });
    } finally {
      setIsDetecting(false);
    }
  }, [latestGuideAttempt?.session?.id, t]);

  return (
    <div className="h-full min-h-0 flex flex-col gap-3">
      <div className="rounded-lg border bg-background/60 p-3 space-y-3">
        <div className="flex items-center justify-between gap-2">
          <div className="text-sm font-semibold">Plan</div>
          <div className="flex items-center gap-2">
            <Button
              size="xs"
              variant="outline"
              onClick={handleLoadFromGuide}
              disabled={!latestGuideAttempt?.session?.id || isDetecting}
              title="Extract the latest plan block from the guide attempt output"
            >
              {isDetecting ? 'Loading…' : 'Load from guide'}
            </Button>
            <Button
              size="xs"
              variant="outline"
              onClick={handleAutoWire}
              disabled={!canPreview || isPreviewing || isApplying}
              title="Generate a simple linear topology when edges are empty"
            >
              Auto-wire
            </Button>
            <Button
              size="xs"
              variant="outline"
              onClick={handlePreview}
              disabled={!canPreview || isPreviewing || isApplying}
            >
              {isPreviewing ? 'Previewing…' : 'Preview'}
            </Button>
            <Button
              size="xs"
              onClick={handleApply}
              disabled={!canApply || isApplying}
            >
              {isApplying ? 'Applying…' : 'Apply'}
            </Button>
          </div>
        </div>

        <Textarea
          value={planText}
          onChange={(e) => setPlanText(e.target.value)}
          rows={10}
          placeholder={
            'Paste a MilestonePlanV1 JSON payload, or paste agent output containing a fenced plan block.'
          }
          className="text-xs font-mono"
        />

        {parsed && !parsed.ok && (
          <div className="text-xs text-destructive">{parsed.error}</div>
        )}
        {parsed && parsed.ok && (
          <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
            <Badge variant="outline" className="text-[11px]">
              schema_version: {parsed.plan.schema_version}
            </Badge>
            <span>nodes: {parsed.plan.nodes?.length ?? 0}</span>
            <span>edges: {parsed.plan.edges?.length ?? 0}</span>
            {parsed.extractedJson && <span>extracted from fenced block</span>}
          </div>
        )}

        {preview && (
          <div className="pt-2 border-t space-y-2 text-xs">
            {preview.metadata_changes.length > 0 && (
              <div className="space-y-1">
                <div className="font-medium">Metadata changes</div>
                <div className="space-y-1">
                  {preview.metadata_changes.map((c) => (
                    <div key={`${c.field}:${c.from ?? ''}:${c.to ?? ''}`}>
                      <span className="font-mono">{c.field}</span>: {c.from ?? '∅'} →{' '}
                      {c.to ?? '∅'}
                    </div>
                  ))}
                </div>
              </div>
            )}

            {preview.tasks_to_create.length > 0 && (
              <div className="space-y-1">
                <div className="font-medium">
                  Tasks to create ({preview.tasks_to_create.length})
                </div>
                <div className="space-y-1">
                  {preview.tasks_to_create.map((t) => (
                    <div key={t.node_id}>
                      <span className="font-mono">{t.node_id}</span>: {t.title}
                    </div>
                  ))}
                </div>
              </div>
            )}

            {preview.task_links.length > 0 && (
              <div className="space-y-1">
                <div className="font-medium">
                  Tasks to link ({preview.task_links.length})
                </div>
                <div className="space-y-1">
                  {preview.task_links.map((link) => (
                    <div key={link.node_id}>
                      <span className="font-mono">{link.node_id}</span>: {link.task_id}
                    </div>
                  ))}
                </div>
              </div>
            )}

            <div className="flex flex-wrap gap-2">
              <Badge variant="outline" className="text-[11px]">
                nodes: +{preview.node_diff.added.length} −{preview.node_diff.removed.length}
              </Badge>
              <Badge variant="outline" className="text-[11px]">
                edges: +{preview.edge_diff.added.length} −{preview.edge_diff.removed.length}
              </Badge>
            </div>

            {(preview.node_diff.added.length > 0 ||
              preview.node_diff.removed.length > 0) && (
              <div className="grid grid-cols-2 gap-2">
                <div className="space-y-1">
                  <div className="font-medium">Added nodes</div>
                  {preview.node_diff.added.length === 0 ? (
                    <div className="text-muted-foreground">None</div>
                  ) : (
                    <div className="space-y-1">
                      {preview.node_diff.added.map((id) => (
                        <div key={id} className="font-mono">
                          {id}
                        </div>
                      ))}
                    </div>
                  )}
                </div>
                <div className="space-y-1">
                  <div className="font-medium">Removed nodes</div>
                  {preview.node_diff.removed.length === 0 ? (
                    <div className="text-muted-foreground">None</div>
                  ) : (
                    <div className="space-y-1">
                      {preview.node_diff.removed.map((id) => (
                        <div key={id} className="font-mono">
                          {id}
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              </div>
            )}
          </div>
        )}
      </div>

      <div className="flex-1 min-h-0 rounded-lg border bg-background/60 overflow-hidden">
        <div className="p-3 border-b flex items-center justify-between gap-2">
          <div className="text-sm font-semibold">Guide</div>
          <Button
            size="xs"
            variant="outline"
            onClick={handleStartGuideAttempt}
            disabled={!entryTask?.id}
          >
            Start guide attempt
          </Button>
        </div>

        {!entryTask ? (
          <div className="p-3 text-sm text-muted-foreground">
            No milestone entry task found.
          </div>
        ) : isGuideAttemptsLoading ? (
          <div className="p-3 text-sm text-muted-foreground">
            Loading guide attempts…
          </div>
        ) : !latestGuideAttempt ? (
          <div className="p-3 text-sm text-muted-foreground">
            No guide attempts yet for this milestone.
          </div>
        ) : (
          <ExecutionProcessesProvider attemptId={latestGuideAttempt.id}>
            <ClickedElementsProvider attempt={latestGuideAttempt}>
              <ReviewProvider attemptId={latestGuideAttempt.id}>
                <TaskAttemptPanel attempt={latestGuideAttempt} task={entryTask}>
                  {({ logs, followUp }) => (
                    <div className="h-full min-h-0 flex flex-col">
                      <div className="flex-1 min-h-0">{logs}</div>
                      <div className="min-h-0 max-h-[45%] border-t overflow-hidden bg-background">
                        <div className="h-full min-h-0">{followUp}</div>
                      </div>
                    </div>
                  )}
                </TaskAttemptPanel>
              </ReviewProvider>
            </ClickedElementsProvider>
          </ExecutionProcessesProvider>
        )}
      </div>
    </div>
  );
}
