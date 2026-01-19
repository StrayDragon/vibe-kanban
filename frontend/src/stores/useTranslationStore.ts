import { create } from 'zustand';
import type {
  TranslationDisplayMode,
  TranslationStatus,
} from '@/utils/translation';

export type TranslationEntry = {
  status: TranslationStatus;
  translatedText?: string;
  error?: string;
  sourceHash: string;
  targetLang: string;
};

type TranslationStore = {
  translations: Record<string, TranslationEntry>;
  displayModes: Record<string, TranslationDisplayMode>;
  setTranslation: (key: string, entry: TranslationEntry) => void;
  clearTranslation: (key: string) => void;
  setDisplayMode: (key: string, mode: TranslationDisplayMode) => void;
};

export const useTranslationStore = create<TranslationStore>((set) => ({
  translations: {},
  displayModes: {},
  setTranslation: (key, entry) =>
    set((state) => ({
      translations: {
        ...state.translations,
        [key]: entry,
      },
    })),
  clearTranslation: (key) =>
    set((state) => {
      const next = { ...state.translations };
      delete next[key];
      return { translations: next };
    }),
  setDisplayMode: (key, mode) =>
    set((state) => ({
      displayModes: {
        ...state.displayModes,
        [key]: mode,
      },
    })),
}));
