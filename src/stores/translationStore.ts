import { create } from "zustand";
import { load, Store } from "@tauri-apps/plugin-store";
import type {
  TranslationProviderType,
  TranslationDisplayMode,
  TranslationResult,
} from "../lib/types";
import { getApiKey, setTranslationLanguages, setTranslationProvider } from "../lib/ipc";
import { translationKey, type TranslationDirection } from "../lib/translationRouting";
import { useConfigStore } from "./configStore";

const STORE_FILE = "translation-config.json";

// Singleton store instance, lazily initialized
let storeInstance: Store | null = null;

async function getStore(): Promise<Store> {
  if (!storeInstance) {
    storeInstance = await load(STORE_FILE, { autoSave: true, defaults: {} });
  }
  return storeInstance;
}

/**
 * Persist a key-value pair to the Tauri plugin-store.
 * Fire-and-forget: errors are logged but do not block the UI.
 */
async function persistValue(key: string, value: unknown): Promise<void> {
  try {
    const store = await getStore();
    await store.set(key, value);
  } catch (err) {
    console.error(`[translationStore] Failed to persist "${key}":`, err);
  }
}

interface TranslationState {
  bidirectional: boolean;
  direction: TranslationDirection;
  lastError: string | null;
  setBidirectional: (enabled: boolean) => void;
  setDirection: (direction: TranslationDirection) => void;
  setLastError: (error: string | null) => void;
  // Persisted preferences
  provider: TranslationProviderType;
  apiModels: Partial<Record<TranslationProviderType, string>>;
  activateProvider: (provider: TranslationProviderType, model?: string, region?: string) => Promise<void>;
  targetLang: string;
  sourceLang: string; // "auto" or ISO code
  displayMode: TranslationDisplayMode;
  autoTranslateEnabled: boolean;
  selectionToolbarEnabled: boolean;
  cacheEnabled: boolean;

  // Session state (not persisted)
  autoTranslateActive: boolean; // current session toggle
  translations: Map<string, TranslationResult>; // segmentId:targetLang → result
  translating: Set<string>; // segmentIds currently being translated
  batchProgress: { completed: number; total: number } | null;

  // Actions
  setProvider: (provider: TranslationProviderType) => void;
  setTargetLang: (lang: string) => void;
  setSourceLang: (lang: string) => void;
  setDisplayMode: (mode: TranslationDisplayMode) => void;
  setAutoTranslateEnabled: (enabled: boolean) => void;
  setSelectionToolbarEnabled: (enabled: boolean) => void;
  setCacheEnabled: (enabled: boolean) => void;
  setAutoTranslateActive: (active: boolean) => void;
  addTranslation: (result: TranslationResult) => void;
  addTranslations: (results: TranslationResult[]) => void;
  setTranslating: (segmentId: string, isTranslating: boolean) => void;
  setBatchProgress: (progress: { completed: number; total: number } | null) => void;
  clearTranslations: () => void;
  loadConfig: () => Promise<void>;
}

export const useTranslationStore = create<TranslationState>((set, get) => ({
  // Persisted defaults
  provider: "hy-mt",
  apiModels: {},
  activateProvider: async (provider, model, region) => {
    await setTranslationProvider(provider, model, region);
    if (provider === "groq" || provider === "gemini") {
      const apiModels = { ...get().apiModels, [provider]: model?.trim() };
      set({ apiModels });
      await persistValue("apiModels", apiModels);
    }
    set({ provider, lastError: null });
    await persistValue("provider", provider);
  },
  targetLang: "vi",
  sourceLang: "auto",
  bidirectional: true,
  direction: "vi-ja",
  lastError: null,
  setBidirectional: (bidirectional) => { set({ bidirectional }); persistValue("bidirectional", bidirectional); },
  setDirection: (direction) => {
    set({ direction }); persistValue("direction", direction);
    const config = useConfigStore.getState();
    const audio = config.meetingAudioConfig;
    if (audio) config.setMeetingAudioConfig({ ...audio,
      you: { ...audio.you, language: direction === "vi-ja" ? "vi-VN" : "ja-JP" },
      them: { ...audio.them, language: direction === "vi-ja" ? "ja-JP" : "vi-VN" },
    });
  },
  setLastError: (lastError) => set({ lastError }),
  displayMode: "inline",
  autoTranslateEnabled: true,
  selectionToolbarEnabled: true,
  cacheEnabled: true,

  // Session defaults
  autoTranslateActive: false,
  translations: new Map(),
  translating: new Set(),
  batchProgress: null,

  setProvider: (provider) => {
    set({ provider });
    persistValue("provider", provider);
  },
  setTargetLang: (lang) => {
    set({ targetLang: lang });
    persistValue("targetLang", lang);
    // Sync to backend immediately so all windows + translate calls use it
    const { sourceLang } = get();
    setTranslationLanguages(lang, sourceLang === "auto" ? undefined : sourceLang).catch(() => {});
  },
  setSourceLang: (lang) => {
    set({ sourceLang: lang });
    persistValue("sourceLang", lang);
    // Sync to backend immediately
    const { targetLang } = get();
    setTranslationLanguages(targetLang, lang === "auto" ? undefined : lang).catch(() => {});
  },
  setDisplayMode: (mode) => {
    set({ displayMode: mode });
    persistValue("displayMode", mode);
  },
  setAutoTranslateEnabled: (enabled) => {
    set({ autoTranslateEnabled: enabled });
    persistValue("autoTranslateEnabled", enabled);
  },
  setSelectionToolbarEnabled: (enabled) => {
    set({ selectionToolbarEnabled: enabled });
    persistValue("selectionToolbarEnabled", enabled);
  },
  setCacheEnabled: (enabled) => {
    set({ cacheEnabled: enabled });
    persistValue("cacheEnabled", enabled);
  },
  setAutoTranslateActive: (active) => set({ autoTranslateActive: active }),

  addTranslation: (result) => {
    if (!result.segment_id) return;
    set((state) => {
      const updated = new Map(state.translations);
      updated.set(translationKey(result.segment_id!, result.target_lang), result);
      const translating = new Set(state.translating);
      translating.delete(result.segment_id!);
      return { translations: updated, translating };
    });
  },
  addTranslations: (results) => {
    set((state) => {
      const updated = new Map(state.translations);
      const translating = new Set(state.translating);
      for (const r of results) {
        if (r.segment_id) {
          updated.set(translationKey(r.segment_id, r.target_lang), r);
          translating.delete(r.segment_id);
        }
      }
      return { translations: updated, translating };
    });
  },
  setTranslating: (segmentId, isTranslating) => {
    set((state) => {
      const updated = new Set(state.translating);
      if (isTranslating) updated.add(segmentId);
      else updated.delete(segmentId);
      return { translating: updated };
    });
  },
  setBatchProgress: (progress) => set({ batchProgress: progress }),
  clearTranslations: () => set({ translations: new Map(), translating: new Set() }),

  loadConfig: async () => {
    try {
      const store = await getStore();
      const savedProvider = await store.get<string>("provider");
      const supported: TranslationProviderType[] = ["hy-mt", "groq", "gemini", "microsoft", "google", "deepl", "opus-mt", "llm"];
      const provider: TranslationProviderType = supported.includes(savedProvider as TranslationProviderType) ? savedProvider as TranslationProviderType : "hy-mt";
      const apiModels = await store.get<Partial<Record<"groq" | "gemini", string>>>("apiModels") ?? {};
      const migrated = savedProvider != null && !supported.includes(savedProvider as TranslationProviderType);
      const bidirectional = await store.get<boolean>("bidirectional");
      const direction = await store.get<TranslationDirection>("direction");
      const targetLang = await store.get<string>("targetLang");
      const sourceLang = await store.get<string>("sourceLang");
      const displayMode = await store.get<TranslationDisplayMode>("displayMode");
      const autoTranslateEnabled = await store.get<boolean>("autoTranslateEnabled");
      const selectionToolbarEnabled = await store.get<boolean>("selectionToolbarEnabled");
      const cacheEnabled = await store.get<boolean>("cacheEnabled");

      set({
        apiModels,
        lastError: migrated ? "Không nhận ra nhà cung cấp dịch đã lưu. Đã chọn Hy-MT2 local." : null,
        bidirectional: bidirectional ?? (provider == null || provider === "hy-mt"),
        direction: direction === "ja-vi" ? "ja-vi" : "vi-ja",
        ...(provider != null && { provider }),
        ...(targetLang != null && { targetLang }),
        ...(sourceLang != null && { sourceLang }),
        ...(displayMode != null && { displayMode }),
        ...(autoTranslateEnabled != null && { autoTranslateEnabled }),
        ...(selectionToolbarEnabled != null && { selectionToolbarEnabled }),
        ...(cacheEnabled != null && { cacheEnabled }),
      });

      if (migrated) await persistValue("provider", provider);
      try {
        const region = provider === "microsoft" ? await getApiKey("translation_microsoft_region") || undefined : undefined;
        await setTranslationProvider(provider, provider === "groq" || provider === "gemini" ? apiModels[provider] : undefined, region);
      }
      catch (error) { set({ lastError: String(error) }); }
      store.onKeyChange<TranslationState["apiModels"]>("apiModels", (value) => { if (value) set({ apiModels: value }); });

      // Sync the loaded languages to the backend immediately
      const tl = targetLang ?? "vi";
      const sl = sourceLang === "auto" ? undefined : (sourceLang ?? undefined);
      setTranslationLanguages(tl, sl).catch(() => {});

      // Cross-window sync: when another window changes the store,
      // update this window's Zustand state automatically.
      store.onKeyChange<TranslationProviderType>("provider", (val) => {
        if (val != null) set({ provider: val });
      });
      store.onKeyChange<boolean>("bidirectional", (val) => { if (val != null) set({ bidirectional: val }); });
      store.onKeyChange<TranslationDirection>("direction", (val) => { if (val === "vi-ja" || val === "ja-vi") set({ direction: val }); });
      store.onKeyChange<string>("targetLang", (val) => {
        if (val != null) set({ targetLang: val });
      });
      store.onKeyChange<string>("sourceLang", (val) => {
        if (val != null) set({ sourceLang: val });
      });
      store.onKeyChange<TranslationDisplayMode>("displayMode", (val) => {
        if (val != null) set({ displayMode: val });
      });
      store.onKeyChange<boolean>("autoTranslateEnabled", (val) => {
        if (val != null) set({ autoTranslateEnabled: val });
      });
      store.onKeyChange<boolean>("selectionToolbarEnabled", (val) => {
        if (val != null) set({ selectionToolbarEnabled: val });
      });
      store.onKeyChange<boolean>("cacheEnabled", (val) => {
        if (val != null) set({ cacheEnabled: val });
      });

      console.log("[translationStore] Config loaded from store (with cross-window sync)");
    } catch (err) {
      console.error("[translationStore] Failed to load config:", err);
    }
  },
}));
