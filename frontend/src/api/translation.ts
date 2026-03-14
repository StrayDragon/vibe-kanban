import { handleApiResponse, makeRequest } from './client';

type TranslationApiRequest = {
  text: string;
  source_lang: string;
  target_lang: string;
};

type TranslationApiResult = {
  translated_text: string;
};

export const translationApi = {
  translate: async (
    text: string,
    sourceLang: string,
    targetLang: string
  ): Promise<string> => {
    const response = await makeRequest('/api/translation', {
      method: 'POST',
      body: JSON.stringify({
        text,
        source_lang: sourceLang,
        target_lang: targetLang,
      } satisfies TranslationApiRequest),
    });

    const payload = await handleApiResponse<TranslationApiResult>(response);
    const translatedText = payload.translated_text;
    if (!translatedText) {
      throw new Error('Translation unavailable');
    }

    return translatedText;
  },
};
