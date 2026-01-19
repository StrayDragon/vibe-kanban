export type TranslationDisplayMode = 'bilingual' | 'translated_only';
export type TranslationStatus = 'idle' | 'loading' | 'success' | 'error';

export type TranslationLanguage = 'en' | 'zh-CN';

export const DEFAULT_SOURCE_LANG: TranslationLanguage = 'en';
export const DEFAULT_TARGET_LANG: TranslationLanguage = 'zh-CN';

export const MYMEMORY_ENDPOINT =
  'https://api.mymemory.translated.net/get';

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

export async function translateMyMemory(
  text: string,
  sourceLang: string,
  targetLang: string
) {
  const url = new URL(MYMEMORY_ENDPOINT);
  url.searchParams.set('q', text);
  url.searchParams.set('langpair', `${sourceLang}|${targetLang}`);

  const response = await fetch(url.toString());
  if (!response.ok) {
    throw new Error(`Translation failed (${response.status})`);
  }
  const data = (await response.json()) as {
    responseStatus?: number;
    responseDetails?: string;
    responseData?: { translatedText?: string };
  };

  if (data.responseStatus && data.responseStatus !== 200) {
    throw new Error(data.responseDetails || 'Translation failed');
  }

  const translatedText = data.responseData?.translatedText;
  if (!translatedText) {
    throw new Error('Translation unavailable');
  }
  return translatedText;
}
