// The backend owns downloads; this view restores snapshots after every mount.
import { useState, useEffect, useCallback } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ModelDownloadProgress } from "../lib/types";
import { downloadLocalSTTModel, cancelModelDownload, getModelDownloads } from "../lib/ipc";
import { useModelDownloadStore, downloadKey, isActiveDownload } from "../stores/modelDownloadStore";
import { showToast } from "../stores/toastStore";

export function useModelDownload() {
  const downloads = useModelDownloadStore((s) => s.downloads);
  const [syncing, setSyncing] = useState(true);
  const [syncError, setSyncError] = useState<string | null>(null);

  useEffect(() => {
    let disposed = false;
    let unlisten: UnlistenFn | undefined;
    let polling = false;
    const refresh = async () => {
      if (disposed || polling) return;
      polling = true;
      try {
        const snapshot = await getModelDownloads();
        if (!disposed) {
          useModelDownloadStore.getState().merge(snapshot);
          setSyncError(null);
          setSyncing(false);
        }
      } catch (error) {
        if (!disposed) setSyncError(String(error));
      } finally { polling = false; }
    };
    // Subscribe first, then hydrate. Revision checks prevent stale IPC replies
    // from replacing a newer event received while the query was in flight.
    void listen<ModelDownloadProgress>("model_download_progress", ({ payload }) => {
      if (!disposed) useModelDownloadStore.getState().merge([payload]);
    }).then((stop) => {
      if (disposed) { stop(); return; }
      unlisten = stop;
      void refresh();
    }).catch((error) => { if (!disposed) { setSyncError(String(error)); void refresh(); } });
    const timer = setInterval(refresh, 2000);
    return () => { disposed = true; clearInterval(timer); unlisten?.(); };
  }, []);

  const startDownload = useCallback(async (engine: string, modelId: string) => {
    const store = useModelDownloadStore.getState();
    if (isActiveDownload(store.downloads[downloadKey(engine, modelId)]?.status)) return;
    store.setPending(engine, modelId); // Synchronous: double-clicks cannot start two requests.
    try {
      await downloadLocalSTTModel(engine, modelId);
      useModelDownloadStore.getState().merge(await getModelDownloads());
    } catch (error) {
      const message = String(error);
      // Another window may have started this download first.
      try {
        const snapshot = await getModelDownloads();
        useModelDownloadStore.getState().merge(snapshot);
        if (snapshot.some((p) => p.engine === engine && p.model_id === modelId && isActiveDownload(p.status))) return;
      } catch { /* Keep the actionable error below. */ }
      useModelDownloadStore.getState().setError(engine, modelId, message);
      showToast(`Download failed: ${message}`, "error");
    }
  }, []);

  const cancelDownload = useCallback(async (engine: string, modelId: string) => {
    try { await cancelModelDownload(engine, modelId); }
    catch (error) { showToast(`Could not cancel download: ${String(error)}`, "error"); }
  }, []);

  return { downloads, startDownload, cancelDownload, syncing, syncError };
}
