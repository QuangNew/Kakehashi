import { create } from "zustand";
import type {
  RagConfig,
  RagIndexStatus,
  RagSearchResult,
  EmbeddingStatus,
} from "../lib/types";
import {
  getRagConfig,
  updateRagConfig,
  getRagStatus as ipcGetRagStatus,
  rebuildRagIndex as ipcRebuildRagIndex,
  rebuildFileIndex as ipcRebuildFileIndex,
  removeFileRagIndex as ipcRemoveFileRagIndex,
  clearRagIndex as ipcClearRagIndex,
  testRagSearch as ipcTestRagSearch,
  testEmbeddingConnection,
} from "../lib/ipc";
import { useConfigStore } from "./configStore";
import { showToast } from "./toastStore";

interface IndexProgress {
  status: string;
  filesTotal: number;
  filesDone: number;
  chunksTotal: number;
  chunksDone: number;
}


// Settings that affect chunk structure — changing these invalidates the index
const INDEX_AFFECTING_KEYS: (keyof RagConfig)[] = [
  "chunk_size",
  "chunk_overlap",
  "splitting_strategy",
  "embedding_model",
  "embedding_provider",
  "embedding_dimensions",
];

interface RagState {
  ragConfig: RagConfig | null;
  indexStatus: RagIndexStatus | null;
  embeddingStatus: EmbeddingStatus | null;
  pullProgress: import("../lib/types").OllamaPullProgressEvent | null;
  isPullingModel: boolean;
  setPullProgress: (progress: import("../lib/types").OllamaPullProgressEvent | null) => void;
  setIsPullingModel: (pulling: boolean) => void;
  isIndexing: boolean;
  indexProgress: IndexProgress | null;
  testSearchResults: RagSearchResult[];
  isSearching: boolean;
  searchLatencyMs: number | null;
  error: string | null;
  isCheckingConnection: boolean;

  // Tracks whether settings changed since last build
  indexStale: boolean;
  sourcesChangedSinceBuild: boolean;
  // Background auto-indexing (triggered by file add/remove)
  isAutoIndexing: boolean;

  // Actions
  loadRagConfig: () => Promise<void>;
  saveRagConfig: (config: RagConfig) => Promise<void>;
  saveRagConfigWithStaleCheck: (config: RagConfig, prevConfig: RagConfig) => Promise<void>;
  refreshIndexStatus: () => Promise<void>;
  checkEmbeddingStatus: () => Promise<void>;
  rebuildIndex: () => Promise<void>;
  clearIndex: () => Promise<void>;
  testSearch: (query: string) => Promise<void>;
  setIndexProgress: (progress: IndexProgress | null) => void;
  setIsIndexing: (indexing: boolean) => void;
  markSourcesChanged: () => void;
  resetTestSearch: () => void;
  autoIndexFile: (resourceId: string) => Promise<void>;
  autoRemoveFileIndex: (resourceId: string) => Promise<void>;
}

export const useRagStore = create<RagState>((set) => ({
  ragConfig: null,
  indexStatus: null,
  embeddingStatus: null,
  pullProgress: null,
  isPullingModel: false,
  setPullProgress: (pullProgress) => set({ pullProgress }),
  setIsPullingModel: (isPullingModel) => set({ isPullingModel }),
  isIndexing: false,
  indexProgress: null,
  testSearchResults: [],
  isSearching: false,
  searchLatencyMs: null,
  error: null,
  isCheckingConnection: false,
  indexStale: false,
  sourcesChangedSinceBuild: false,
  isAutoIndexing: false,

  loadRagConfig: async () => {
    try {
      const config = await getRagConfig();
      // Sync enabled flag with persisted contextStrategy
      const strategy = useConfigStore.getState().contextStrategy;
      const shouldBeEnabled = strategy === "local_rag";
      if (config.enabled !== shouldBeEnabled) {
        config.enabled = shouldBeEnabled;
        await updateRagConfig(config);
      }
      set({ ragConfig: config, error: null });
    } catch (e) {
      console.error("[ragStore] Failed to load RAG config:", e);
      set({ error: e instanceof Error ? e.message : String(e) });
    }
  },

  saveRagConfig: async (config: RagConfig) => {
    try {
      set({ ragConfig: config, error: null });
      await updateRagConfig(config);
    } catch (e) {
      console.error("[ragStore] Failed to save RAG config:", e);
      set({ error: e instanceof Error ? e.message : String(e) });
    }
  },

  saveRagConfigWithStaleCheck: async (config: RagConfig, prevConfig: RagConfig) => {
    try {
      // Check if any index-affecting setting changed
      const affectsIndex = INDEX_AFFECTING_KEYS.some(
        (key) => config[key] !== prevConfig[key]
      );
      set({ ragConfig: config, error: null });
      if (affectsIndex) {
        const hasIndex = (useRagStore.getState().indexStatus?.total_chunks ?? 0) > 0;
        if (hasIndex) {
          set({ indexStale: true });
        }
      }
      await updateRagConfig(config);
    } catch (e) {
      console.error("[ragStore] Failed to save RAG config:", e);
      set({ error: e instanceof Error ? e.message : String(e) });
    }
  },

  refreshIndexStatus: async () => {
    try {
      const status = await ipcGetRagStatus();
      set({ indexStatus: status });
    } catch (e) {
      console.warn("[ragStore] Failed to refresh index status:", e);
    }
  },

  checkEmbeddingStatus: async () => {
    set({ isCheckingConnection: true });
    try {
      const status = await testEmbeddingConnection();
      set({ embeddingStatus: status, error: null, isCheckingConnection: false });
      if (status.connected) {
        showToast(
          `Embedding backend connected — ${status.models.length} model${status.models.length !== 1 ? "s" : ""} available`,
          "success"
        );
      } else {
        showToast("Embedding backend is not reachable", "error");
      }
    } catch (e) {
      set({
        embeddingStatus: { connected: false, models: [] },
        error: e instanceof Error ? e.message : String(e),
        isCheckingConnection: false,
      });
      showToast("Failed to connect to the embedding backend", "error");
    }
  },

  rebuildIndex: async () => {
    try {
      set({ isIndexing: true, error: null });
      await ipcRebuildRagIndex();
      showToast("Index rebuilt successfully", "success");
      set({ indexStale: false, sourcesChangedSinceBuild: false });
    } catch (e) {
      console.error("[ragStore] Failed to rebuild index:", e);
      const msg = e instanceof Error ? e.message : String(e);
      showToast(msg, "error");
      set({ isIndexing: false, error: msg });
    } finally {
      set({ isIndexing: false });
      useRagStore.getState().refreshIndexStatus();
    }
  },

  clearIndex: async () => {
    try {
      set({ error: null });
      await ipcClearRagIndex();
      set({ indexStatus: null, indexStale: false, sourcesChangedSinceBuild: false });
      showToast("Index cleared", "info");
      useRagStore.getState().refreshIndexStatus();
    } catch (e) {
      console.error("[ragStore] Failed to clear index:", e);
      const msg = e instanceof Error ? e.message : String(e);
      showToast(msg, "error");
      set({ error: msg });
    }
  },


  testSearch: async (query: string) => {
    try {
      const start = performance.now();
      set({ isSearching: true, error: null, testSearchResults: [], searchLatencyMs: null });
      const results = await ipcTestRagSearch(query);
      const latency = Math.round(performance.now() - start);
      set({ testSearchResults: results, isSearching: false, searchLatencyMs: latency });
    } catch (e) {
      console.error("[ragStore] Failed to test search:", e);
      set({
        isSearching: false,
        searchLatencyMs: null,
        error: e instanceof Error ? e.message : String(e),
      });
    }
  },

  setIndexProgress: (progress) => set({ indexProgress: progress }),
  setIsIndexing: (indexing) => set({ isIndexing: indexing }),
  markSourcesChanged: () => {
    const hasIndex = (useRagStore.getState().indexStatus?.total_chunks ?? 0) > 0;
    if (hasIndex) {
      set({ sourcesChangedSinceBuild: true });
    }
  },

  resetTestSearch: () => set({
    testSearchResults: [],
    error: null,
    isSearching: false,
    searchLatencyMs: null,
  }),

  // Auto-index a single newly-added file in the background (non-blocking for the user)
  autoIndexFile: async (resourceId: string) => {
    set({ isAutoIndexing: true });
    try {
      await ipcRebuildFileIndex(resourceId);
      await useRagStore.getState().refreshIndexStatus();
    } catch (e) {
      // Non-critical — index can be rebuilt manually; just log it
      console.warn("[ragStore] Auto-index failed for", resourceId, e);
    } finally {
      set({ isAutoIndexing: false });
    }
  },

  // Remove a single file's chunks from the index when the file is removed
  autoRemoveFileIndex: async (resourceId: string) => {
    set({ isAutoIndexing: true });
    try {
      await ipcRemoveFileRagIndex(resourceId);
      await useRagStore.getState().refreshIndexStatus();
    } catch (e) {
      console.warn("[ragStore] Auto-remove index failed for", resourceId, e);
    } finally {
      set({ isAutoIndexing: false });
    }
  },
}));
