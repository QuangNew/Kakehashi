import { create } from "zustand";
import type { ModelDownloadProgress } from "../lib/types";

export const downloadKey = (engine: string, modelId: string) => `${engine}:${modelId}`;
export const isActiveDownload = (status?: ModelDownloadProgress["status"]) =>
  !!status && !["complete", "error", "cancelled"].includes(status);

interface ModelDownloads {
  downloads: Record<string, ModelDownloadProgress>;
  merge: (updates: ModelDownloadProgress[]) => void;
  setPending: (engine: string, modelId: string) => void;
  setError: (engine: string, modelId: string, error: string) => void;
}

export const useModelDownloadStore = create<ModelDownloads>((set) => ({
  downloads: {},
  merge: (updates) => set((state) => {
    const downloads = { ...state.downloads };
    for (const progress of updates) {
      const key = downloadKey(progress.engine, progress.model_id);
      // An event may arrive while the snapshot IPC call is in flight.
      if (!downloads[key] || progress.revision > downloads[key].revision) downloads[key] = progress;
    }
    return { downloads };
  }),
  setPending: (engine, modelId) => set((state) => {
    const key = downloadKey(engine, modelId);
    const previous = state.downloads[key];
    return { downloads: { ...state.downloads, [key]: {
      engine, model_id: modelId, downloaded_bytes: previous?.downloaded_bytes ?? 0,
      total_bytes: previous?.total_bytes ?? 0, percent: previous?.percent ?? 0,
      revision: previous?.revision ?? 0, status: "queued", error: null,
    } } };
  }),
  setError: (engine, modelId, error) => set((state) => {
    const key = downloadKey(engine, modelId);
    const previous = state.downloads[key];
    if (!previous) return {};
    return { downloads: { ...state.downloads, [key]: { ...previous, status: "error", error } } };
  }),
}));
