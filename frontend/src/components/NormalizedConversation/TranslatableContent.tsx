import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { AlertCircle, Loader2 } from 'lucide-react';
import WYSIWYGEditor from '@/components/ui/wysiwyg';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import { useTranslationStore } from '@/stores/useTranslationStore';
import {
  DEFAULT_SOURCE_LANG,
  DEFAULT_TARGET_LANG,
  getLanguageLabel,
  hasCodeFence,
  hashText,
  translateViaApi,
  type TranslationDisplayMode,
} from '@/utils/translation';

type Props = {
  entryKey: string;
  content: string;
  markdown: boolean;
  contentClassName?: string;
  taskAttemptId?: string;
  taskId?: string;
  canTranslate?: boolean;
  onEdit?: () => void;
};

const buildTranslationKey = (entryKey: string, targetLang: string) =>
  `${entryKey}:${targetLang}`;

const defaultDisplayMode: TranslationDisplayMode = 'bilingual';

const TranslatableContent = ({
  entryKey,
  content,
  markdown,
  contentClassName,
  taskAttemptId,
  taskId,
  canTranslate = false,
  onEdit,
}: Props) => {
  const { t, i18n } = useTranslation('common');
  const { translations, displayModes, setTranslation, setDisplayMode } =
    useTranslationStore();

  const targetLang = DEFAULT_TARGET_LANG;
  const translationKey = useMemo(
    () => buildTranslationKey(entryKey, targetLang),
    [entryKey, targetLang]
  );
  const sourceHash = useMemo(() => hashText(content), [content]);
  const translation = translations[translationKey];
  const displayMode = displayModes[entryKey] ?? defaultDisplayMode;
  const isStale = translation?.sourceHash !== sourceHash;
  const hasTranslation = translation?.status === 'success' && !isStale;
  const translationError =
    translation?.status === 'error' ? translation.error : null;
  const translationLoading = translation?.status === 'loading';
  const showTranslateAction =
    canTranslate && content.trim().length > 0 && !hasCodeFence(content);

  const targetLabel = useMemo(
    () => getLanguageLabel(targetLang, i18n.language),
    [i18n.language, targetLang]
  );

  const shouldHideOriginal =
    hasTranslation && displayMode === 'translated_only';

  if (!markdown) {
    return <div className={contentClassName}>{content}</div>;
  }

  const runTranslation = async () => {
    if (!showTranslateAction || translationLoading) return;
    if (hasTranslation) return;

    const sourceLang = DEFAULT_SOURCE_LANG;
    if (sourceLang === targetLang) {
      setTranslation(translationKey, {
        status: 'success',
        translatedText: content,
        sourceHash,
        targetLang,
      });
      return;
    }

    setTranslation(translationKey, {
      status: 'loading',
      translatedText: translation?.translatedText,
      sourceHash,
      targetLang,
      error: undefined,
    });

    try {
      const translatedText = await translateViaApi(
        content,
        sourceLang,
        targetLang
      );
      setTranslation(translationKey, {
        status: 'success',
        translatedText,
        sourceHash,
        targetLang,
      });
    } catch (error) {
      setTranslation(translationKey, {
        status: 'error',
        translatedText: translation?.translatedText,
        sourceHash,
        targetLang,
        error: error instanceof Error ? error.message : String(error),
      });
    }
  };

  return (
    <div>
      <WYSIWYGEditor
        value={content}
        disabled
        className={cn(
          'whitespace-pre-wrap break-words flex flex-col gap-1 font-light',
          contentClassName,
          shouldHideOriginal && 'hidden'
        )}
        taskAttemptId={taskAttemptId}
        taskId={taskId}
        onEdit={onEdit}
        onTranslate={showTranslateAction ? runTranslation : undefined}
        translateState={translationLoading ? 'loading' : undefined}
        translateLabel={t('conversation.translation.action')}
      />

      {translationLoading && (
        <div className="mt-2 flex items-center gap-2 text-xs text-muted-foreground">
          <Loader2 className="h-3 w-3 animate-spin" />
          <span>{t('conversation.translation.loading')}</span>
        </div>
      )}

      {translationError && !translationLoading && (
        <div className="mt-2 flex items-center gap-2 text-xs text-destructive">
          <AlertCircle className="h-3 w-3" />
          <span>{t('conversation.translation.failed')}</span>
          <Button
            type="button"
            variant="ghost"
            size="xs"
            onClick={runTranslation}
            className="text-destructive"
          >
            {t('conversation.translation.retry')}
          </Button>
        </div>
      )}

      {translation?.status === 'success' && isStale && (
        <div className="mt-2 flex items-center gap-2 text-xs text-muted-foreground">
          <AlertCircle className="h-3 w-3" />
          <span>{t('conversation.translation.stale')}</span>
          <Button
            type="button"
            variant="ghost"
            size="xs"
            onClick={runTranslation}
          >
            {t('conversation.translation.retranslate')}
          </Button>
        </div>
      )}

      {hasTranslation && (
        <div className="mt-3 border-l-2 border-emerald-300/50 pl-3 text-sm text-muted-foreground">
          <div className="flex flex-wrap items-center gap-2 text-xs uppercase tracking-wide text-muted-foreground">
            <span>
              {t('conversation.translation.label', { language: targetLabel })}
            </span>
            <Button
              type="button"
              variant="ghost"
              size="xs"
              onClick={() =>
                setDisplayMode(
                  entryKey,
                  displayMode === 'bilingual' ? 'translated_only' : 'bilingual'
                )
              }
            >
              {displayMode === 'bilingual'
                ? t('conversation.translation.showOnly')
                : t('conversation.translation.showBoth')}
            </Button>
          </div>
          <div className="mt-1 whitespace-pre-wrap break-words text-foreground">
            {translation.translatedText}
          </div>
        </div>
      )}
    </div>
  );
};

export default TranslatableContent;
