import { translationApi } from '@/lib/api';

export type TranslationDisplayMode = 'bilingual' | 'translated_only';
export type TranslationStatus = 'idle' | 'loading' | 'success' | 'error';

export type TranslationLanguage = 'en' | 'zh-CN';

export const DEFAULT_SOURCE_LANG: TranslationLanguage = 'en';
export const DEFAULT_TARGET_LANG: TranslationLanguage = 'zh-CN';

export function hasCodeFence(text: string) {
  return text.includes('```');
}

export function hashText(text: string) {
  let hash = 5381;
  for (let i = 0; i < text.length; i += 1) {
    hash = ((hash << 5) + hash) ^ text.charCodeAt(i);
  }
  return String(hash >>> 0);
}

export function getLanguageLabel(lang: string, displayLocale: string) {
  try {
    return (
      new Intl.DisplayNames([displayLocale], { type: 'language' }).of(lang) ||
      lang
    );
  } catch {
    return lang;
  }
}

export async function translateViaApi(
  text: string,
  sourceLang: string,
  targetLang: string
) {
  return translationApi.translate(text, sourceLang, targetLang);
}
