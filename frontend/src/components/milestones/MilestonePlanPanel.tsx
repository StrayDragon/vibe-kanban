import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
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
  MilestonePlanDetectionResult,
  MilestonePlanPreviewResponse,
  MilestonePlanV1,
  TaskWithAttemptStatus,
} from 'shared/types';

type ParseResult =
  | {
      ok: true;
      plan: MilestonePlanV1;
      extractedJson: string | null;
      extractedFrom: 'fenced' | 'embedded' | null;
    }
  | {
      ok: false;
      error: string;
      extractedJson: string | null;
      extractedFrom: 'fenced' | 'embedded' | null;
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

function extractEmbeddedJsonObject(input: string): string | null {
  const text = input;
  const firstBrace = text.indexOf('{');
  if (firstBrace === -1) return null;

  for (let start = firstBrace; start < text.length; start += 1) {
    if (text[start] !== '{') continue;

    let depth = 0;
    let inString = false;
    let escapeNext = false;

    for (let i = start; i < text.length; i += 1) {
      const ch = text[i];

      if (inString) {
        if (escapeNext) {
          escapeNext = false;
          continue;
        }
        if (ch === '\\') {
          escapeNext = true;
          continue;
        }
        if (ch === '"') {
          inString = false;
        }
        continue;
      }

      if (ch === '"') {
        inString = true;
        continue;
      }
      if (ch === '{') {
        depth += 1;
        continue;
      }
      if (ch === '}') {
        depth -= 1;
        if (depth === 0) {
          const candidate = text.slice(start, i + 1).trim();
          return candidate.length ? candidate : null;
        }
      }
    }
  }

  return null;
}

function parsePlanText(raw: string): ParseResult | null {
  const trimmed = raw.trim();
  if (!trimmed) return null;

  const candidates: Array<{
    json: string;
    extractedJson: string | null;
    extractedFrom: 'fenced' | 'embedded' | null;
  }> = [];

  if (trimmed.startsWith('{') || trimmed.startsWith('[')) {
    candidates.push({
      json: trimmed,
      extractedJson: null,
      extractedFrom: null,
    });
  }

  const fenced = extractFencedJson(trimmed);
  if (fenced) {
    candidates.push({
      json: fenced,
      extractedJson: fenced,
      extractedFrom: 'fenced',
    });
  }

  const embedded = extractEmbeddedJsonObject(trimmed);
  if (embedded) {
    candidates.push({
      json: embedded,
      extractedJson: embedded,
      extractedFrom: 'embedded',
    });
  }

  const seen = new Set<string>();
  let lastError: string | null = null;

  for (const candidate of candidates) {
    if (seen.has(candidate.json)) continue;
    seen.add(candidate.json);
    try {
      const parsed = JSON.parse(candidate.json) as MilestonePlanV1;
      if (!parsed || typeof parsed !== 'object') {
        lastError = 'Plan payload must be a JSON object.';
        continue;
      }
      return {
        ok: true,
        plan: parsed,
        extractedJson: candidate.extractedJson,
        extractedFrom: candidate.extractedFrom,
      };
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      lastError = `Invalid JSON: ${msg}`;
    }
  }

  return {
    ok: false,
    error: lastError ?? 'Invalid JSON',
    extractedJson: null,
    extractedFrom: null,
  };
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

  const [planDetection, setPlanDetection] =
    useState<MilestonePlanDetectionResult | null>(null);
  const [preview, setPreview] = useState<MilestonePlanPreviewResponse | null>(
    null
  );
  const [isPreviewing, setIsPreviewing] = useState(false);
  const [isDetecting, setIsDetecting] = useState(false);
  const [isAdvancedOpen, setIsAdvancedOpen] = useState(false);

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
        description:
          err instanceof Error ? err.message : 'Failed to preview plan',
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
        description:
          err instanceof Error ? err.message : 'Failed to apply plan',
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

  const handleDetectPlan = useCallback(
    async (opts?: { silent?: boolean }) => {
      const sessionId = latestGuideAttempt?.session?.id;
      if (!sessionId) {
        if (!opts?.silent) {
          toast({
            variant: 'destructive',
            title: t('common:states.error', 'Error'),
            description: t(
              'milestone.planner.detect.noGuideAttempt',
              'No guide attempt session found. Start a guide attempt first.'
            ),
          });
        }
        return;
      }

      setIsDetecting(true);
      try {
        const result = await sessionsApi.detectLatestMilestonePlan(sessionId);
        setPlanDetection(result);

        if (result.status === 'found' && result.plan) {
          setPlanText(prettyJson(result.plan));
          if (!opts?.silent) {
            toast({
              title: t('milestone.planner.detect.loadedTitle', 'Plan loaded'),
              description: t(
                'milestone.planner.detect.loadedBody',
                'Detected the latest plan from the guide output.'
              ),
            });
          }
          return;
        }

        if (!opts?.silent) {
          toast({
            variant: 'destructive',
            title: t('common:states.error', 'Error'),
            description:
              result.status === 'not_found'
                ? t(
                    'milestone.planner.detect.notFound',
                    'No milestone plan block detected in the latest guide output.'
                  )
                : (result.error ??
                  t(
                    'milestone.planner.detect.failed',
                    'Failed to detect a milestone plan.'
                  )),
          });
        }
      } catch (err) {
        console.error('Failed to detect plan from guide output:', err);
        if (!opts?.silent) {
          toast({
            variant: 'destructive',
            title: t('common:states.error', 'Error'),
            description:
              err instanceof Error ? err.message : 'Failed to detect plan',
          });
        }
      } finally {
        setIsDetecting(false);
      }
    },
    [latestGuideAttempt?.session?.id, t]
  );

  useEffect(() => {
    const sessionId = latestGuideAttempt?.session?.id;
    if (!sessionId) {
      setPlanDetection(null);
      return;
    }
    void handleDetectPlan({ silent: true });
  }, [handleDetectPlan, latestGuideAttempt?.session?.id]);

  return (
    <div className="h-full min-h-0 flex flex-col gap-3">
      <div className="rounded-lg border bg-background/60 p-3 space-y-3">
        <div className="flex items-center justify-between gap-2">
          <div className="text-sm font-semibold">
            {t('milestone.planner.title', 'Planner')}
          </div>
          <div className="flex items-center gap-2">
            <Button
              size="xs"
              variant="outline"
              onClick={() => handleDetectPlan()}
              disabled={!latestGuideAttempt?.session?.id || isDetecting}
              title={t(
                'milestone.planner.detect.title',
                'Detect the latest plan block from the guide output'
              )}
            >
              {isDetecting
                ? t('milestone.planner.detect.loading', 'Detecting…')
                : t('milestone.planner.detect.cta', 'Refresh plan')}
            </Button>
            <Button
              size="xs"
              variant="outline"
              onClick={handlePreview}
              disabled={!canPreview || isPreviewing || isApplying}
            >
              {isPreviewing
                ? t('milestone.planner.preview.loading', 'Previewing…')
                : t('milestone.planner.preview.cta', 'Preview')}
            </Button>
            <Button
              size="xs"
              onClick={handleApply}
              disabled={!canApply || isApplying}
            >
              {isApplying
                ? t('milestone.planner.apply.loading', 'Applying…')
                : t('milestone.planner.apply.cta', 'Apply')}
            </Button>
          </div>
        </div>

        <div className="text-xs text-muted-foreground">
          {t(
            'milestone.planner.flowHelp',
            'Generate a plan with the Guide, refresh to detect it, then Preview and Apply.'
          )}
        </div>

        {!latestGuideAttempt?.session?.id ? (
          <Alert>
            <AlertTitle>
              {t(
                'milestone.planner.status.noGuideTitle',
                'No guide output yet'
              )}
            </AlertTitle>
            <AlertDescription>
              {t(
                'milestone.planner.status.noGuideBody',
                'Start a guide attempt below to generate a plan payload.'
              )}
            </AlertDescription>
          </Alert>
        ) : planDetection?.status === 'found' ? (
          <Alert variant="success">
            <AlertTitle>
              {t('milestone.planner.status.foundTitle', 'Plan detected')}
            </AlertTitle>
            <AlertDescription>
              {t(
                'milestone.planner.status.foundBody',
                'A plan payload was detected from the latest guide output.'
              )}
            </AlertDescription>
          </Alert>
        ) : planDetection?.status === 'invalid' ? (
          <Alert variant="destructive">
            <AlertTitle>
              {t(
                'milestone.planner.status.invalidTitle',
                'Invalid plan payload'
              )}
            </AlertTitle>
            <AlertDescription>
              {planDetection.error ??
                t(
                  'milestone.planner.status.invalidBody',
                  'The latest plan payload could not be parsed.'
                )}
            </AlertDescription>
          </Alert>
        ) : planDetection?.status === 'unsupported' ? (
          <Alert variant="destructive">
            <AlertTitle>
              {t(
                'milestone.planner.status.unsupportedTitle',
                'Unsupported plan schema'
              )}
            </AlertTitle>
            <AlertDescription>
              {planDetection.error ??
                t(
                  'milestone.planner.status.unsupportedBody',
                  'The latest plan payload uses an unsupported schema.'
                )}
            </AlertDescription>
          </Alert>
        ) : planDetection?.status === 'not_found' ? (
          <Alert>
            <AlertTitle>
              {t('milestone.planner.status.notFoundTitle', 'No plan detected')}
            </AlertTitle>
            <AlertDescription>
              {t(
                'milestone.planner.status.notFoundBody',
                'Ask the guide to emit a fenced `milestone-plan-v1` block, then refresh.'
              )}
            </AlertDescription>
          </Alert>
        ) : (
          <Alert>
            <AlertTitle>
              {t('milestone.planner.status.readyTitle', 'Waiting for a plan')}
            </AlertTitle>
            <AlertDescription>
              {t(
                'milestone.planner.status.readyBody',
                'Run the guide and refresh to detect the latest plan payload.'
              )}
            </AlertDescription>
          </Alert>
        )}

        {parsed && parsed.ok && (
          <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
            <Badge variant="outline" className="text-[11px]">
              schema_version: {parsed.plan.schema_version}
            </Badge>
            <span>nodes: {parsed.plan.nodes?.length ?? 0}</span>
            <span>edges: {parsed.plan.edges?.length ?? 0}</span>
            {planDetection?.status === 'found' &&
              planDetection.extracted_from && (
                <span>detected: {planDetection.extracted_from}</span>
              )}
          </div>
        )}

        <details
          className="group rounded-md border bg-muted/20"
          open={isAdvancedOpen}
          onToggle={(e) => setIsAdvancedOpen(e.currentTarget.open)}
        >
          <summary className="list-none cursor-pointer px-3 py-2 text-xs font-medium flex items-center justify-between">
            <span>
              {t('milestone.planner.advanced.title', 'Advanced / Debug')}
            </span>
            <span className="text-muted-foreground">
              {isAdvancedOpen
                ? t('milestone.planner.advanced.hide', 'Hide')
                : t('milestone.planner.advanced.show', 'Show')}
            </span>
          </summary>

          {isAdvancedOpen && (
            <div className="px-3 pb-3 space-y-2 border-t">
              <div className="flex items-center justify-between gap-2">
                <div className="text-xs text-muted-foreground">
                  {t(
                    'milestone.planner.advanced.help',
                    'View/copy or import a raw plan payload for troubleshooting.'
                  )}
                </div>
                <div className="flex items-center gap-2">
                  <Button
                    size="xs"
                    variant="outline"
                    onClick={async () => {
                      try {
                        await navigator.clipboard.writeText(planText);
                        toast({
                          title: t(
                            'milestone.planner.advanced.copiedTitle',
                            'Copied'
                          ),
                          description: t(
                            'milestone.planner.advanced.copiedBody',
                            'Raw payload copied to clipboard.'
                          ),
                        });
                      } catch (err) {
                        toast({
                          variant: 'destructive',
                          title: t('common:states.error', 'Error'),
                          description:
                            err instanceof Error
                              ? err.message
                              : 'Failed to copy payload',
                        });
                      }
                    }}
                    disabled={!planText.trim()}
                  >
                    {t('milestone.planner.advanced.copy', 'Copy payload')}
                  </Button>
                  <Button
                    size="xs"
                    variant="outline"
                    onClick={handleAutoWire}
                    disabled={!canPreview || isPreviewing || isApplying}
                    title={t(
                      'milestone.planner.advanced.autoWireTitle',
                      'Generate a simple linear topology when edges are empty'
                    )}
                  >
                    {t('milestone.planner.advanced.autoWire', 'Auto-wire')}
                  </Button>
                </div>
              </div>

              <Textarea
                value={planText}
                onChange={(e) => setPlanText(e.target.value)}
                rows={10}
                placeholder={t(
                  'milestone.planner.advanced.rawPlaceholder',
                  'Paste a MilestonePlanV1 JSON payload, or paste agent output containing a fenced plan block.'
                )}
                className="text-xs font-mono"
              />

              {parsed && !parsed.ok && (
                <div className="text-xs text-destructive">{parsed.error}</div>
              )}
              {parsed && parsed.ok && parsed.extractedJson && (
                <div className="text-xs text-muted-foreground">
                  extracted from{' '}
                  {parsed.extractedFrom === 'embedded'
                    ? 'embedded JSON'
                    : 'fenced block'}
                </div>
              )}
            </div>
          )}
        </details>

        {preview && (
          <div className="pt-2 border-t space-y-2 text-xs">
            {preview.metadata_changes.length > 0 && (
              <div className="space-y-1">
                <div className="font-medium">Metadata changes</div>
                <div className="space-y-1">
                  {preview.metadata_changes.map((c) => (
                    <div key={`${c.field}:${c.from ?? ''}:${c.to ?? ''}`}>
                      <span className="font-mono">{c.field}</span>:{' '}
                      {c.from ?? '∅'} → {c.to ?? '∅'}
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
                      <span className="font-mono">{link.node_id}</span>:{' '}
                      {link.task_id}
                    </div>
                  ))}
                </div>
              </div>
            )}

            <div className="flex flex-wrap gap-2">
              <Badge variant="outline" className="text-[11px]">
                nodes: +{preview.node_diff.added.length} −
                {preview.node_diff.removed.length}
              </Badge>
              <Badge variant="outline" className="text-[11px]">
                edges: +{preview.edge_diff.added.length} −
                {preview.edge_diff.removed.length}
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
          <div className="text-sm font-semibold">
            {t('milestone.planner.guideTitle', 'Guide')}
          </div>
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
