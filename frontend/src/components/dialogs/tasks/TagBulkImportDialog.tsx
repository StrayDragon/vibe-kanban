import type { ChangeEvent } from 'react';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import NiceModal, { useModal } from '@ebay/nice-modal-react';
import { defineModal, getErrorMessage } from '@/lib/modals';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog';
import { Alert } from '@/components/ui/alert';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Loader2, Upload } from 'lucide-react';
import { tagsApi } from '@/lib/api';
import {
  parseTagMarkdown,
  type ParsedTagEntry,
} from '@/lib/tag-markdown-import';
import type { Tag } from 'shared/types';

export interface TagBulkImportDialogProps {
  existingTags: Tag[];
}

export type TagBulkImportResult = 'imported' | 'canceled';

type Stage = 'upload' | 'preview' | 'confirm-duplicates';

type DuplicateEntry = {
  tagName: string;
  newContent: string;
  existingContent: string;
};

const TagBulkImportDialogImpl = NiceModal.create<TagBulkImportDialogProps>(
  ({ existingTags }) => {
    const modal = useModal();
    const { t } = useTranslation('settings');
    const [stage, setStage] = useState<Stage>('upload');
    const [entries, setEntries] = useState<ParsedTagEntry[]>([]);
    const [fileName, setFileName] = useState<string | null>(null);
    const [error, setError] = useState<string | null>(null);
    const [importing, setImporting] = useState(false);
    const [selectedEntries, setSelectedEntries] = useState<Set<string>>(
      new Set()
    );
    const [confirmedUpdates, setConfirmedUpdates] = useState<Set<string>>(
      new Set()
    );

    const existingTagMap = useMemo(() => {
      const map = new Map<string, Tag>();
      existingTags.forEach((tag) => {
        map.set(tag.tag_name, tag);
      });
      return map;
    }, [existingTags]);

    const selectedEntriesList = useMemo(
      () => entries.filter((entry) => selectedEntries.has(entry.tagName)),
      [entries, selectedEntries]
    );

    const duplicateEntries = useMemo<DuplicateEntry[]>(() => {
      return entries
        .map((entry) => {
          const existing = existingTagMap.get(entry.tagName);
          if (!existing) return null;
          return {
            tagName: entry.tagName,
            newContent: entry.content,
            existingContent: existing.content ?? '',
          };
        })
        .filter((entry): entry is DuplicateEntry => entry !== null);
    }, [entries, existingTagMap]);

    const selectedDuplicateEntries = useMemo(
      () =>
        duplicateEntries.filter((entry) =>
          selectedEntries.has(entry.tagName)
        ),
      [duplicateEntries, selectedEntries]
    );

    const emptyContentEntries = useMemo(
      () => selectedEntriesList.filter((entry) => !entry.content.trim()),
      [selectedEntriesList]
    );

    const resetState = useCallback(() => {
      setStage('upload');
      setEntries([]);
      setFileName(null);
      setError(null);
      setImporting(false);
      setSelectedEntries(new Set());
      setConfirmedUpdates(new Set());
    }, []);

    useEffect(() => {
      if (modal.visible) {
        resetState();
      }
    }, [modal.visible, resetState]);

    const handleFileChange = useCallback(
      async (event: ChangeEvent<HTMLInputElement>) => {
        const file = event.target.files?.[0];
        if (!file) return;

        setError(null);
        setFileName(file.name);

        try {
          const text = await file.text();
          const parsed = parseTagMarkdown(text);

          if (parsed.length === 0) {
            setError(t('settings.general.tags.bulkImport.errors.noTags'));
            setEntries([]);
            setSelectedEntries(new Set());
            setStage('upload');
            return;
          }

          setEntries(parsed);
          setSelectedEntries(new Set(parsed.map((entry) => entry.tagName)));
          setStage('preview');
        } catch (err) {
          setError(
            getErrorMessage(err) ||
              t('settings.general.tags.bulkImport.errors.readFailed')
          );
        }
      },
      [t]
    );

    const handleOpenChange = (open: boolean) => {
      if (!open) {
        modal.resolve('canceled' as TagBulkImportResult);
        modal.hide();
      }
    };

    const toggleDuplicateConfirmation = (tagName: string) => {
      setConfirmedUpdates((prev) => {
        const next = new Set(prev);
        if (next.has(tagName)) {
          next.delete(tagName);
        } else {
          next.add(tagName);
        }
        return next;
      });
    };

    const toggleSelection = (tagName: string) => {
      setSelectedEntries((prev) => {
        const next = new Set(prev);
        if (next.has(tagName)) {
          next.delete(tagName);
        } else {
          next.add(tagName);
        }
        return next;
      });
    };

    const selectAll = () => {
      setSelectedEntries(new Set(entries.map((entry) => entry.tagName)));
    };

    const deselectAll = () => {
      setSelectedEntries(new Set());
    };

    const isAllSelected =
      entries.length > 0 && selectedEntries.size === entries.length;

    const canImport =
      emptyContentEntries.length === 0 && selectedEntriesList.length > 0;

    const allDuplicatesConfirmed =
      selectedDuplicateEntries.length === 0 ||
      selectedDuplicateEntries.every((entry) =>
        confirmedUpdates.has(entry.tagName)
      );

    const handleImport = async () => {
      if (!canImport) return;

      setImporting(true);
      setError(null);

      try {
        const requests = selectedEntriesList.map((entry) => {
          const existing = existingTagMap.get(entry.tagName);
          if (existing) {
            return tagsApi.update(existing.id, {
              tag_name: entry.tagName,
              content: entry.content,
            });
          }
          return tagsApi.create({
            tag_name: entry.tagName,
            content: entry.content,
          });
        });

        await Promise.all(requests);
        modal.resolve('imported' as TagBulkImportResult);
        modal.hide();
      } catch (err) {
        setError(
          getErrorMessage(err) ||
            t('settings.general.tags.bulkImport.errors.importFailed')
        );
      } finally {
        setImporting(false);
      }
    };

    const handlePreviewConfirm = () => {
      if (!canImport) return;
      if (selectedDuplicateEntries.length > 0) {
        setConfirmedUpdates(new Set());
        setStage('confirm-duplicates');
        return;
      }
      void handleImport();
    };

    const renderUploadStage = () => (
      <div className="space-y-4 py-4">
        <div className="space-y-2">
          <Label htmlFor="tag-import-file">
            {t('settings.general.tags.bulkImport.upload.label')}
          </Label>
          <Input
            id="tag-import-file"
            type="file"
            accept=".md,text/markdown"
            onChange={handleFileChange}
          />
          <p className="text-xs text-muted-foreground">
            {t('settings.general.tags.bulkImport.upload.hint')}
          </p>
        </div>
        {error && <Alert variant="destructive">{error}</Alert>}
      </div>
    );

    const renderPreviewTable = () => (
      <div className="border rounded-lg overflow-hidden">
        <div className="max-h-[320px] overflow-y-auto overflow-x-hidden">
          <table className="w-full table-fixed">
            <thead className="border-b bg-muted/50 sticky top-0">
              <tr>
                <th className="text-left p-2 text-xs font-medium w-12">
                  {t('settings.general.tags.bulkImport.preview.selectLabel')}
                </th>
                <th className="text-left p-2 text-sm font-medium w-[240px]">
                  {t('settings.general.tags.manager.table.tagName')}
                </th>
                <th className="text-left p-2 text-sm font-medium">
                  {t('settings.general.tags.manager.table.content')}
                </th>
              </tr>
            </thead>
            <tbody>
              {entries.map((entry) => {
                const isExisting = existingTagMap.has(entry.tagName);
                const isSelected = selectedEntries.has(entry.tagName);
                return (
                  <tr
                    key={entry.tagName}
                    className={
                      isExisting
                        ? 'border-b bg-amber-500/10 hover:bg-amber-500/15 transition-colors'
                        : 'border-b hover:bg-muted/30 transition-colors'
                    }
                  >
                    <td className="p-2 align-top">
                      <Checkbox
                        checked={isSelected}
                        onCheckedChange={() => toggleSelection(entry.tagName)}
                        className={
                          isSelected
                            ? 'mt-0.5 border-primary'
                            : 'mt-0.5 border-muted-foreground/60'
                        }
                      />
                    </td>
                    <td className="p-2 text-sm font-medium">
                      <div className="flex items-center gap-2">
                        <span className="break-all">@{entry.tagName}</span>
                        {isExisting && (
                          <Badge
                            variant="outline"
                            className="border-amber-300 bg-amber-50 text-amber-700"
                          >
                            {t(
                              'settings.general.tags.bulkImport.preview.existingBadge'
                            )}
                          </Badge>
                        )}
                      </div>
                    </td>
                    <td className="p-2 text-sm">
                      <div
                        className="truncate"
                        title={entry.content}
                      >
                        {entry.content || (
                          <span className="text-muted-foreground">-</span>
                        )}
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>
    );

    const renderPreviewStage = () => (
      <div className="space-y-4 py-4">
        <div className="space-y-1">
          <p className="text-sm font-medium">
            {t('settings.general.tags.bulkImport.preview.count', {
              count: entries.length,
            })}
          </p>
          {fileName && (
            <p className="text-xs text-muted-foreground">
              {t('settings.general.tags.bulkImport.preview.file', {
                fileName,
              })}
            </p>
          )}
          {selectedDuplicateEntries.length > 0 && (
            <p className="text-xs text-amber-600">
              {t('settings.general.tags.bulkImport.preview.duplicates', {
                count: selectedDuplicateEntries.length,
              })}
            </p>
          )}
        </div>
        <div className="flex items-center justify-between">
          <p className="text-xs text-muted-foreground">
            {t('settings.general.tags.bulkImport.preview.selected', {
              selected: selectedEntriesList.length,
              total: entries.length,
            })}
          </p>
          <Button
            variant="ghost"
            size="sm"
            onClick={isAllSelected ? deselectAll : selectAll}
          >
            {isAllSelected
              ? t('settings.general.tags.bulkImport.preview.deselectAll')
              : t('settings.general.tags.bulkImport.preview.selectAll')}
          </Button>
        </div>
        {renderPreviewTable()}
        {emptyContentEntries.length > 0 && (
          <Alert variant="destructive">
            {t('settings.general.tags.bulkImport.errors.emptyContent', {
              tags: emptyContentEntries
                .map((entry) => `@${entry.tagName}`)
                .join(', '),
            })}
          </Alert>
        )}
        {error && <Alert variant="destructive">{error}</Alert>}
      </div>
    );

    const renderDuplicateStage = () => (
      <div className="space-y-4 py-4">
        <div className="space-y-1">
          <p className="text-sm font-medium">
            {t('settings.general.tags.bulkImport.duplicates.title')}
          </p>
          <p className="text-xs text-muted-foreground">
            {t('settings.general.tags.bulkImport.duplicates.hint')}
          </p>
        </div>
        <div className="space-y-3 max-h-[360px] overflow-auto">
          {selectedDuplicateEntries.map((entry) => (
            <div
              key={entry.tagName}
              className="border rounded-lg p-3 space-y-2"
            >
              <div className="flex items-start justify-between gap-3">
                <div>
                  <p className="text-sm font-medium">@{entry.tagName}</p>
                  <p className="text-xs text-muted-foreground">
                    {t('settings.general.tags.bulkImport.duplicates.existing')}
                  </p>
                </div>
                <div className="flex items-center gap-2">
                  <Checkbox
                    checked={confirmedUpdates.has(entry.tagName)}
                    onCheckedChange={() =>
                      toggleDuplicateConfirmation(entry.tagName)
                    }
                    className={
                      confirmedUpdates.has(entry.tagName)
                        ? 'border-primary'
                        : 'border-muted-foreground/60'
                    }
                  />
                  <span className="text-xs">
                    {t('settings.general.tags.bulkImport.duplicates.confirm')}
                  </span>
                </div>
              </div>
              <div className="grid gap-3 md:grid-cols-2">
                <div className="rounded-md bg-muted/40 p-2">
                  <p className="text-[11px] uppercase text-muted-foreground">
                    {t(
                      'settings.general.tags.bulkImport.duplicates.existingLabel'
                    )}
                  </p>
                  <p className="text-xs whitespace-pre-wrap">
                    {entry.existingContent || '-'}
                  </p>
                </div>
                <div className="rounded-md bg-muted/40 p-2">
                  <p className="text-[11px] uppercase text-muted-foreground">
                    {t(
                      'settings.general.tags.bulkImport.duplicates.newLabel'
                    )}
                  </p>
                  <p className="text-xs whitespace-pre-wrap">
                    {entry.newContent || '-'}
                  </p>
                </div>
              </div>
            </div>
          ))}
        </div>
        {error && <Alert variant="destructive">{error}</Alert>}
      </div>
    );

    return (
      <Dialog
        open={modal.visible}
        onOpenChange={handleOpenChange}
        className="w-[95vw] sm:max-w-[980px]"
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <Upload className="h-5 w-5" />
              {t('settings.general.tags.bulkImport.title')}
            </DialogTitle>
          </DialogHeader>

          {stage === 'upload' && renderUploadStage()}
          {stage === 'preview' && renderPreviewStage()}
          {stage === 'confirm-duplicates' && renderDuplicateStage()}

          <DialogFooter>
            <Button variant="outline" onClick={() => handleOpenChange(false)}>
              {t('settings.general.tags.bulkImport.buttons.cancel')}
            </Button>
            {stage === 'preview' && (
              <Button
                variant="outline"
                onClick={() => setStage('upload')}
                disabled={importing}
              >
                {t('settings.general.tags.bulkImport.buttons.back')}
              </Button>
            )}
            {stage === 'confirm-duplicates' && (
              <Button
                variant="outline"
                onClick={() => setStage('preview')}
                disabled={importing}
              >
                {t('settings.general.tags.bulkImport.buttons.back')}
              </Button>
            )}
            {stage === 'preview' && (
              <Button
                onClick={handlePreviewConfirm}
                disabled={importing || !canImport}
              >
                {importing && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
                {t('settings.general.tags.bulkImport.buttons.import')}
              </Button>
            )}
            {stage === 'confirm-duplicates' && (
              <Button
                onClick={handleImport}
                disabled={importing || !allDuplicatesConfirmed}
              >
                {importing && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
                {t('settings.general.tags.bulkImport.buttons.confirm')}
              </Button>
            )}
          </DialogFooter>
        </DialogContent>
      </Dialog>
    );
  }
);

export const TagBulkImportDialog = defineModal<
  TagBulkImportDialogProps,
  TagBulkImportResult
>(TagBulkImportDialogImpl);
