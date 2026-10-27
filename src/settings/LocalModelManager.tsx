// Model download/management UI for local STT engines.
// Tracks active model, auto-activates on first download, supports Set Active.

import { useState, useEffect, useRef } from "react";
import {
  HardDrive,
  Download,
  Trash2,
  CheckCircle,
  Loader2,
  X,
  Star,
  CircleDot,
} from "lucide-react";
import { useModelDownload } from "../hooks/useModelDownload";
import { useConfigStore } from "../stores/configStore";
import { listLocalSTTEngines, deleteLocalSTTModel } from "../lib/ipc";
import type { LocalSTTEngineInfo, LocalSTTModelInfo } from "../lib/types";
import { showToast } from "../stores/toastStore";
import { isActiveDownload } from "../stores/modelDownloadStore";
import type { ModelDownloadProgress } from "../lib/types";

interface LocalModelManagerProps {
  compact?: boolean;
  /** If set, only show this engine's models (e.g. "sherpa_onnx") */
  engineFilter?: string;
}

function isEnglishOnlyWhisper(model: LocalSTTModelInfo): boolean {
  return model.engine === "whisper_cpp" && model.id === "distil-large-v3";
}

export function LocalModelManager({ compact, engineFilter }: LocalModelManagerProps) {
  const [engines, setEngines] = useState<LocalSTTEngineInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const { downloads, startDownload, cancelDownload, syncing, syncError } = useModelDownload();
  const completedRevisions = useRef(new Map<string, number>());
  const activeWhisperModel = useConfigStore((s) => s.activeWhisperModel);
  const setActiveWhisperModel = useConfigStore((s) => s.setActiveWhisperModel);
  const activeModelPerEngine = useConfigStore((s) => s.activeModelPerEngine);
  const setActiveModelForEngine = useConfigStore((s) => s.setActiveModelForEngine);

  async function loadEngines(showLoading = true) {
    if (showLoading) setLoading(true);
    try {
      const data = await listLocalSTTEngines();
      setEngines(data);
      return data;
    } catch (err) {
      console.error("Failed to load local STT engines:", err);
      return [];
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    loadEngines();
  }, []);

  // When a download completes: reload engines, auto-activate if no active model for that engine
  useEffect(() => {
    const completed = Object.entries(downloads).filter(
      ([key, d]) => d.status === "complete" && completedRevisions.current.get(key) !== d.revision
    );
    if (completed.length) {
      for (const [key, d] of completed) completedRevisions.current.set(key, d.revision);
      loadEngines(false).then((data) => {
        for (const engine of data) {
          const engineActiveModel = activeModelPerEngine[engine.engine];
          if (!engineActiveModel) {
            const firstDownloaded = engine.models.find(
              // English-only Distil must be an explicit choice in this VI/JA app.
              (m) => !m.id.startsWith("binary-") && m.is_downloaded && !isEnglishOnlyWhisper(m)
            );
            if (firstDownloaded) {
              setActiveModelForEngine(engine.engine, firstDownloaded.id);
              // Keep legacy field in sync for whisper_cpp
              if (engine.engine === "whisper_cpp") {
                setActiveWhisperModel(firstDownloaded.id);
              }
              showToast(`Model "${firstDownloaded.name}" activated for ${engine.name}`, "success");
            }
          }
        }
      });
    }
  }, [downloads]);

  if (loading) {
    return (
      <div className="flex items-center gap-2 py-4 text-xs text-muted-foreground">
        <Loader2 className="h-3.5 w-3.5 animate-spin" />
        Loading local models...
      </div>
    );
  }

  const filteredEngines = engineFilter
    ? engines.filter((e) => e.engine === engineFilter)
    : engines;

  return (
    <div className="space-y-4">
      {syncError && <p role="alert" className="text-xs text-destructive">Không đồng bộ được tiến trình tải: {syncError}. Đang thử kết nối lại…</p>}
      {filteredEngines.map((engine) => (
        <div key={engine.engine}>
          {!compact && !engineFilter && (
            <div className="mb-2">
              <h4 className="text-xs font-semibold text-foreground flex items-center gap-1.5">
                <HardDrive className="h-3.5 w-3.5" />
                {engine.name}
              </h4>
              <p className="text-meta text-muted-foreground mt-0.5">
                {engine.description}
              </p>
            </div>
          )}

          <div className="space-y-1.5">
            {engine.models.map((model) => {
              const downloadKey = `${model.engine}:${model.id}`;
              const progress = downloads[downloadKey];
              const isDownloading = isActiveDownload(progress?.status);
              // Per-engine active model: check if THIS model is active for THIS engine
              const isActive = (activeModelPerEngine[model.engine] ?? activeWhisperModel) === model.id;

              return (
                <ModelRow
                  key={model.id}
                  model={model}
                  isActive={isActive}
                  isDownloading={isDownloading}
                  progress={progress}
                  compact={compact}
                  syncing={syncing}
                  onDownload={() => startDownload(model.engine, model.id)}
                  onCancel={() => cancelDownload(model.engine, model.id)}
                  onDelete={async () => {
                    try {
                      await deleteLocalSTTModel(model.engine, model.id);
                      if (isActive) {
                        setActiveModelForEngine(model.engine, null);
                        if (model.engine === "whisper_cpp") setActiveWhisperModel(null);
                      }
                      showToast(`Deleted ${model.name}`, "success");
                      loadEngines();
                    } catch (err) {
                      showToast(`Failed to delete: ${err}`, "error");
                    }
                  }}
                  onSetActive={() => {
                    setActiveModelForEngine(model.engine, model.id);
                    // Keep legacy field in sync for whisper_cpp
                    if (model.engine === "whisper_cpp") setActiveWhisperModel(model.id);
                    showToast(`Activated "${model.name}" for ${engine.name}`, "success");
                  }}
                />
              );
            })}
          </div>
        </div>
      ))}
    </div>
  );
}

/** Check if any whisper model is downloaded. Used by other components.
 *  Re-fetches whenever activeWhisperModel changes (e.g., after download + auto-activate). */
export function useWhisperModelReady(): {
  anyDownloaded: boolean;
  activeModel: string | null;
  activeModelName: string | null;
  loading: boolean;
} {
  const [engines, setEngines] = useState<LocalSTTEngineInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const activeWhisperModel = useConfigStore((s) => s.activeWhisperModel);

  useEffect(() => {
    setLoading(true);
    listLocalSTTEngines()
      .then(setEngines)
      .catch(() => {})
      .finally(() => setLoading(false));
  }, [activeWhisperModel]); // re-fetch when active model changes

  const whisperEngine = engines.find((e) => e.engine === "whisper_cpp");
  const anyDownloaded =
    whisperEngine?.models.some((m) => m.is_downloaded) ?? false;
  const activeModelInfo = whisperEngine?.models.find(
    (m) => m.id === activeWhisperModel && m.is_downloaded
  );

  return {
    anyDownloaded,
    activeModel: activeModelInfo ? activeWhisperModel : null,
    activeModelName: activeModelInfo?.name ?? null,
    loading,
  };
}

/** Check if a specific local engine has at least one model downloaded.
 *  Used to gate availability of sherpa_onnx, ort_streaming in dropdowns. */
export function useLocalEngineReady(engineId: string): {
  ready: boolean;
  hasModels: boolean;
  hasBinary: boolean;
} {
  const [engines, setEngines] = useState<LocalSTTEngineInfo[]>([]);
  useEffect(() => {
    listLocalSTTEngines().then(setEngines).catch(() => {});
  }, []);

  const engine = engines.find((e) => e.engine === engineId);
  if (!engine) return { ready: false, hasModels: false, hasBinary: false };

  const hasBinary = true; // No separate binary needed for any engine
  const hasModels = engine.models.some(
    (m) => !m.id.startsWith("binary-") && m.is_downloaded
  );

  const ready = hasModels;

  return { ready, hasModels, hasBinary };
}

// ── Model Row ──

function ModelRow({
  model,
  isActive,
  isDownloading,
  progress,
  compact,
  syncing,
  onDownload,
  onCancel,
  onDelete,
  onSetActive,
}: {
  model: LocalSTTModelInfo;
  isActive: boolean;
  isDownloading: boolean;
  progress?: ModelDownloadProgress;
  compact?: boolean;
  syncing?: boolean;
  onDownload: () => void;
  onCancel: () => void;
  onDelete: () => void;
  onSetActive: () => void;
}) {
  const sizeLabel = formatSize(model.size_bytes);
  const englishOnly = isEnglishOnlyWhisper(model);

  return (
    <div
      className={`rounded-lg border px-3 py-2 transition-all ${
        isActive
          ? "border-primary/40 bg-primary/5"
          : "border-border/30 hover:border-border/50"
      }`}
    >
      <div className="flex items-center gap-2">
        {/* Model info */}
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1.5">
            <span className="text-xs font-medium text-foreground truncate">
              {compact ? model.id : model.name}
            </span>
            {isActive && (
              <span className="shrink-0 rounded bg-primary/20 px-1.5 py-0.5 text-meta font-semibold text-primary">
                Active
              </span>
            )}
          </div>
          {!compact && (
            <div className="flex items-center gap-2 mt-0.5">
              <span className="text-meta text-muted-foreground">
                {sizeLabel}
              </span>
              <RatingBadge label="Accuracy" value={model.accuracy_rating} max={5} />
              <RatingBadge label="Speed" value={model.speed_rating} max={5} />
            </div>
          )}
        </div>

        {/* Actions */}
        <div className="flex items-center gap-1 shrink-0">
          {isDownloading ? (
            <button
              onClick={onCancel}
              className="rounded-md p-1.5 text-muted-foreground hover:text-red-500 hover:bg-red-500/10"
              title="Cancel download"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          ) : model.is_downloaded ? (
            <>
              {!isActive && (
                <button
                  onClick={onSetActive}
                  className="inline-flex items-center gap-1 rounded-md bg-accent px-2 py-1 text-meta font-medium text-foreground hover:bg-primary/10 hover:text-primary"
                  title="Set as active model"
                >
                  <CircleDot className="h-3 w-3" />
                  {compact ? "" : "Activate"}
                </button>
              )}
              {isActive && (
                <CheckCircle className="h-3.5 w-3.5 text-success" />
              )}
              <button
                onClick={onDelete}
                className="rounded-md p-1.5 text-muted-foreground hover:text-red-500 hover:bg-red-500/10"
                title="Delete model"
              >
                <Trash2 className="h-3 w-3" />
              </button>
            </>
          ) : (
            <button
              onClick={onDownload}
              disabled={syncing}
              title={syncing ? "Đang kiểm tra tải nền…" : "Download / resume model"}
              className="inline-flex items-center gap-1 rounded-md bg-primary/10 px-2 py-1 text-meta font-medium text-primary hover:bg-primary/20 disabled:opacity-50"
            >
              <Download className="h-3 w-3" />
              {compact ? "" : progress?.status === "error" || progress?.status === "cancelled" ? progress.downloaded_bytes > 0 ? "Resume" : "Retry" : "Download"}
            </button>
          )}
        </div>
      </div>

      {englishOnly && (
        <p className="mt-1 text-xs text-amber-500">
          Chỉ nhận dạng tiếng Anh. Với tiếng Việt/Nhật, chọn Whisper Large v3 Turbo.
        </p>
      )}

      {/* Download progress bar */}
      {isDownloading && progress && (
        <div className="mt-1.5">
          <div className="relative h-1.5 overflow-hidden rounded-full bg-muted">
            <div
              className="absolute inset-y-0 left-0 rounded-full bg-primary transition-all duration-200"
              style={{ width: `${Math.min(progress.percent, 100)}%` }}
            />
          </div>
          <div className="mt-0.5 flex items-center justify-between text-meta text-muted-foreground">
            <span>
              {progress.status === "queued"
                ? "Connecting…"
                : progress.status === "retrying"
                  ? "Kết nối gián đoạn; đang tải tiếp…"
                  : progress.status === "extracting"
                ? "Extracting..."
                : progress.status === "verifying"
                  ? "Verifying..."
                  : `${formatSize(progress.downloaded_bytes)} / ${formatSize(progress.total_bytes)}`}
            </span>
            <span>{Math.round(progress.percent)}%</span>
          </div>
        </div>
      )}
      {progress?.status === "error" && <p role="alert" className="mt-1.5 break-words text-xs text-destructive">{progress.error || "Tải bị gián đoạn."} {progress.downloaded_bytes > 0 ? "Phần đã tải được giữ; bấm Resume để tiếp tục." : "Bấm Retry để thử lại."}</p>}
    </div>
  );
}

function RatingBadge({ label, value, max }: { label: string; value: number; max: number }) {
  return (
    <span className="inline-flex items-center gap-0.5 text-meta text-muted-foreground">
      {label}:
      {Array.from({ length: max }, (_, i) => (
        <Star
          key={i}
          className={`h-2 w-2 ${
            i < value ? "fill-amber-400 text-amber-400" : "text-muted-foreground/50"
          }`}
        />
      ))}
    </span>
  );
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(0)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}
