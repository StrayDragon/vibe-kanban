import type { ApiResponse } from 'shared/types';

export type TranslationDisplayMode = 'bilingual' | 'translated_only';
export type TranslationStatus = 'idle' | 'loading' | 'success' | 'error';

export type TranslationLanguage = 'en' | 'zh-CN';

export const DEFAULT_SOURCE_LANG: TranslationLanguage = 'en';
export const DEFAULT_TARGET_LANG: TranslationLanguage = 'zh-CN';

type TranslationApiRequest = {
  text: string;
  source_lang: string;
  target_lang: string;
};

type TranslationApiResult = {
  translated_text: string;
};

const TRANSLATION_ENDPOINT = '/api/translation';

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
  const response = await fetch(TRANSLATION_ENDPOINT, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      text,
      source_lang: sourceLang,
      target_lang: targetLang,
    } satisfies TranslationApiRequest),
  });

  let payload: ApiResponse<TranslationApiResult> | null = null;
  try {
    payload = (await response.json()) as ApiResponse<TranslationApiResult>;
  } catch {
    payload = null;
  }

  if (!response.ok || !payload?.success) {
    const message =
      payload?.message ?? `Translation failed (${response.status})`;
    throw new Error(message);
  }

  const translatedText = payload.data?.translated_text;
  if (!translatedText) {
    throw new Error('Translation unavailable');
  }
  return translatedText;
}
