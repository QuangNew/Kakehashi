import { getSettingsStore, writeSetting } from "../lib/persistedSettings";
import { ModelPreparationControls } from "../components/ModelPreparationControls";
import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Loader2, Play, Square } from "lucide-react";
import {
  getLocalModelsConfig, getLocalModelsStatus, saveLocalModelsConfig, startLocalModel, stopLocalModel,
  localModelsEqual, localModelsDraftMatches,
  type LocalModelsConfig, type ModelRole, type RuntimeStatus,
} from "../lib/localModels";
import { listAudioDevices, listLocalSTTEngines, setTranslationProvider, getRagConfig, updateRagConfig } from "../lib/ipc";
import { useConfigStore } from "../stores/configStore";
import { useMeetingStore } from "../stores/meetingStore";
import { useTranslationStore } from "../stores/translationStore";
import { useAIActionsStore } from "../stores/aiActionsStore";
import { useRagStore } from "../stores/ragStore";
import { useContextStore } from "../stores/contextStore";
import { BUILT_IN_PRESETS, applyPreset } from "./presets";

const TITLES: Record<ModelRole, string> = {
  translation: "Dịch Nhật ↔ Việt — Hy-MT2",
  copilot: "Trợ lý cuộc họp — Qwen",
  embeddings: "Tìm tài liệu — Qwen3 Embedding",
};
const BILINGUAL_INSTRUCTIONS = "For Vietnamese–Japanese meetings: explain briefly in Vietnamese. When suggesting what to say, provide a polite Japanese reply followed by its Vietnamese meaning. Use finalized transcript and supplied documents as evidence. Preserve numbers, dates, negation and uncertainty. Do not invent agreements, progress or deadlines. /no_think";
const inputClass = "w-full rounded-lg border border-border/50 bg-background px-3 py-2 text-sm text-foreground";
const buttonClass = "inline-flex items-center justify-center gap-2 rounded-lg border border-border/50 px-3 py-2 text-sm hover:bg-accent disabled:cursor-not-allowed disabled:opacity-50";

export function LocalModelsSettings() {
  const [config, setConfig] = useState<LocalModelsConfig | null>(null);
  const [actualConfig,setActualConfig]=useState<LocalModelsConfig | null>(null);
  const [statuses, setStatuses] = useState<RuntimeStatus[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const meetingActive = useMeetingStore((s) => s.activeMeeting != null);
  const direction = useTranslationStore((s) => s.direction);
  const setDirection = useTranslationStore((s) => s.setDirection);
  const disabled = !!busy || meetingActive;

  useEffect(() => {
    let disposed = false;
    getLocalModelsConfig().then(async(value) => {
      const draft=await getSettingsStore("settings-drafts.json").then(store=>store.get<{config:LocalModelsConfig;base:string}>("localModels")).catch(()=>null);
      if(!disposed){setActualConfig(value);setConfig(draft && localModelsDraftMatches(draft.base,value) ? draft.config : value);}
    }).catch((e) => { if (!disposed) setError(String(e)); });
    const refresh = () => getLocalModelsStatus().then((value) => { if (!disposed) setStatuses(value); }).catch((e) => { if (!disposed) setError(String(e)); });
    void refresh();
    const timer = setInterval(refresh, 2000);
    return () => { disposed = true; clearInterval(timer); };
  }, []);

  useEffect(()=>{
    if(config && actualConfig)void writeSetting("settings-drafts.json","localModels",localModelsEqual(config,actualConfig) ? null : {config,base:JSON.stringify(actualConfig)}).catch(()=>{});
  },[config,actualConfig]);
  async function applyModelConfig(value:LocalModelsConfig){
    await saveLocalModelsConfig(value);setActualConfig(value);
    await writeSetting("settings-drafts.json","localModels",null);
  }

  async function run(label: string, action: () => Promise<void>) {
    setBusy(label); setError(null); setMessage(null);
    try { await action(); setStatuses(await getLocalModelsStatus()); }
    catch (e) { setError(String(e)); }
    finally { setBusy(null); }
  }

  async function browse(field: "models_dir" | "server_path" | ModelRole) {
    try {
      const chosen = await open(field === "models_dir"
        ? { directory: true, multiple: false, title: "Chọn thư mục models" }
        : { multiple: false, filters: [{ name: field === "server_path" ? "llama-server" : "GGUF model", extensions: [field === "server_path" ? "exe" : "gguf"] }] });
      if (typeof chosen !== "string") return;
      setConfig((previous) => previous && (field === "models_dir" || field === "server_path"
        ? { ...previous, [field]: chosen }
        : { ...previous, [field]: { ...previous[field], file: chosen } }));
    } catch (e) { setError(String(e)); }
  }

  async function saveAndStart(role: ModelRole) {
    if (!config) throw new Error("Model configuration is not loaded");
    await applyModelConfig(config);
    await useContextStore.getState().refreshTokenBudget();
    await startLocalModel(role);
  }

  async function enableMeeting() {
    if (!config) throw new Error("Model configuration is not loaded");
    await applyModelConfig(config);
    await useContextStore.getState().refreshTokenBudget();
    const engines = await listLocalSTTEngines();
    if (!engines.find((engine) => engine.engine === "whisper_cpp")?.models.some((model) => model.id === "large-v3-turbo" && model.is_downloaded)) {
      throw new Error("Thiếu Whisper Large v3 Turbo: đặt ggml-large-v3-turbo.bin trong models/whisper_cpp rồi áp dụng lại. Có thể tải bằng pnpm models:setup.");
    }
    await startLocalModel("translation");
    const devices = await listAudioDevices();
    const preset = BUILT_IN_PRESETS.find((p) => p.you.language === "vi-VN" && p.them.language === "ja-JP")!;
    const audio = applyPreset(preset, [...devices.inputs, ...devices.outputs]);
    if (direction === "ja-vi") { audio.you.language = "ja-JP"; audio.them.language = "vi-VN"; }
    await setTranslationProvider("hy-mt");
    useConfigStore.getState().setActiveModelForEngine("whisper_cpp", "large-v3-turbo");
    useConfigStore.getState().setActiveWhisperModel("large-v3-turbo");
    useConfigStore.getState().setMeetingAudioConfig(audio);
    const translation = useTranslationStore.getState();
    translation.setProvider("hy-mt"); translation.setBidirectional(true);
    translation.setDisplayMode("inline"); translation.setAutoTranslateEnabled(true);
    const actions = useAIActionsStore.getState();
    const previous = actions.configs.customInstructions;
    if (!previous.includes("For Vietnamese–Japanese meetings:")) actions.setCustomInstructions([previous, BILINGUAL_INSTRUCTIONS].filter(Boolean).join("\n\n"));
    setMessage("Đã bật phụ đề hai chiều. Whisper Large v3 Turbo cần nằm trong models/whisper_cpp. Copilot được chọn riêng bên dưới hoặc trong LLM Providers.");
  }

  async function enableCopilot() {
    await saveAndStart("copilot");
    await useConfigStore.getState().activateLLMProvider("local_gguf", { model: "copilot" });
    setMessage("Copilot local đã sẵn sàng; model dịch được giữ riêng.");
  }

  async function enableRag() {
    await saveAndStart("embeddings");
    const previous = await getRagConfig();
    await updateRagConfig({ ...previous, enabled: true, embedding_provider: "local_gguf", embedding_model: "embeddings", embedding_dimensions: 1024, chunk_size: 256, chunk_overlap: 32, batch_size: 8, include_transcript: false });
    useConfigStore.getState().setContextStrategy("local_rag");
    await useRagStore.getState().loadRagConfig();
    useRagStore.getState().markSourcesChanged();
    setMessage("RAG local đã bật. Hãy Rebuild tài liệu trong Context trước cuộc họp.");
  }

  if (!config) return <div role="status" className="p-4 text-sm text-muted-foreground">{error || "Đang đọc cấu hình model local…"}</div>;
  return (
    <div className="space-y-5">
      <div>
        <h2 className="text-lg font-semibold">Model local · Việt ↔ 日本語</h2>
        <p className="mt-1 text-sm text-muted-foreground">Chọn thư mục model và llama-server. Ứng dụng tự mở runtime khi dùng AI và dừng các tiến trình do nó tạo khi thoát.</p>
      </div>
      {config && actualConfig && !localModelsEqual(config,actualConfig) && <p role="status" className="rounded-lg bg-amber-500/10 p-3 text-sm">Bản nháp đã lưu cho lần sau; nhấn Kiểm tra model hoặc Áp dụng để đổi cấu hình backend.</p>}
      {meetingActive && <p role="status" className="rounded-lg bg-amber-500/10 p-3 text-sm">Kết thúc cuộc họp để đổi file model hoặc cấu hình runtime.</p>}
      <ModelPreparationControls />
      {error && <p role="alert" className="break-words rounded-lg bg-destructive/10 p-3 text-sm text-destructive">{error}</p>}
      {message && <p role="status" className="rounded-lg bg-success/10 p-3 text-sm">{message}</p>}
      <fieldset disabled={disabled} className="space-y-4 rounded-xl border border-border/40 p-4">
        {(["models_dir", "server_path"] as const).map((field) => (
          <label key={field} className="block space-y-1.5 text-xs text-muted-foreground">
            <span>{field === "models_dir" ? "Thư mục models" : "File llama-server.exe"}</span>
            <div className="flex gap-2">
              <input aria-label={field === "models_dir" ? "Models folder" : "Runtime executable"} value={config[field]} onChange={(e) => setConfig({ ...config, [field]: e.target.value })} className={inputClass} />
              <button type="button" aria-label={`Browse ${field}`} className={buttonClass} onClick={() => browse(field)}><FolderOpen className="h-4 w-4" /></button>
            </div>
          </label>
        ))}
        <label className="block space-y-1.5 text-xs text-muted-foreground">
          <span>Ngôn ngữ theo nguồn âm thanh</span>
          <select aria-label="Meeting translation direction" className={inputClass} value={direction} onChange={(e) => setDirection(e.target.value as "vi-ja" | "ja-vi")}>
            <option value="vi-ja">Mic: Tiếng Việt → 日本語 · System: 日本語 → Tiếng Việt</option>
            <option value="ja-vi">Mic: 日本語 → Tiếng Việt · System: Tiếng Việt → 日本語</option>
          </select>
        </label>
      </fieldset>
      {(["translation", "copilot", "embeddings"] as const).map((role) => {
        const status = statuses.find((s) => s.role === role);
        return <fieldset key={role} disabled={disabled} className="space-y-3 rounded-xl border border-border/40 p-4">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <h3 className="text-sm font-semibold">{TITLES[role]}</h3>
            <span className="text-xs text-muted-foreground">{status?.state ?? "stopped"}{status?.device ? ` · ${status.device}` : ""}</span>
          </div>
          {role === "translation" && <label className="block space-y-1.5 text-xs text-muted-foreground">
            <span>Cấu hình dịch local</span>
            <select aria-label="Translation model profile" className={inputClass}
              value={config.translation.file.includes("Hy-MT2-7B-") ? "quality" : config.translation.file.includes("Hy-MT2-1.8B-") ? "fast" : "custom"}
              onChange={(e) => {
                if (e.target.value === "custom") return;
                const quality = e.target.value === "quality";
                setConfig({ ...config, translation: { file: quality ? "translation/Hy-MT2-7B-Q4_K_M.gguf" : "translation/Hy-MT2-1.8B-Q4_K_M.gguf", context: 2048, gpu_layers: quality ? 22 : 99 } });
              }}>
              <option value="quality">Hy-MT2 7B Q4 — ưu tiên chất lượng (4,62 GB)</option>
              <option value="fast">Hy-MT2 1.8B Q4 — ưu tiên tốc độ (1,13 GB)</option>
              <option value="custom">File tùy chọn</option>
            </select>
            <p>Model 7B dùng nhiều RAM hơn. Tải bằng <code>pnpm models:setup</code>, sau đó áp dụng cấu hình.</p>
          </label>}
          <label className="block space-y-1.5 text-xs text-muted-foreground">
            <span>File GGUF (đường dẫn đầy đủ hoặc tương đối trong thư mục models)</span>
            <div className="flex gap-2">
              <input aria-label={`${role} model file`} className={inputClass} value={config[role].file} onChange={(e) => setConfig({ ...config, [role]: { ...config[role], file: e.target.value } })} />
              <button type="button" className={buttonClass} aria-label={`Browse ${role} model`} onClick={() => browse(role)}><FolderOpen className="h-4 w-4" /></button>
            </div>
          </label>
          <details className="text-xs text-muted-foreground">
            <summary className="cursor-pointer">Tài nguyên chạy model</summary>
            <div className="mt-3 grid grid-cols-2 gap-3">
              <label>Số lớp GPU (0 = CPU)<input aria-label={`${role} GPU layers`} className={inputClass} type="number" min={0} max={99} value={config[role].gpu_layers} onChange={(e) => setConfig({ ...config, [role]: { ...config[role], gpu_layers: Number(e.target.value) } })} /></label>
              <label>Context (tokens)<input aria-label={`${role} context`} className={inputClass} type="number" min={512} max={16384} value={config[role].context} onChange={(e) => setConfig({ ...config, [role]: { ...config[role], context: Number(e.target.value) } })} /></label>
            </div>
          </details>
          {status?.error && <p className="break-words text-xs text-destructive">{status.error}</p>}
          <div className="flex flex-wrap gap-2">
            <button className={buttonClass} onClick={() => run(role, () => saveAndStart(role))}><Play className="h-3.5 w-3.5" />Kiểm tra model</button>
            <button className={buttonClass} onClick={() => run(`stop-${role}`, () => stopLocalModel(role))}><Square className="h-3.5 w-3.5" />Dừng</button>
            {role === "copilot" && <button className={buttonClass} onClick={() => run("enable-copilot", enableCopilot)}>Dùng copilot local</button>}
            {role === "embeddings" && <button className={buttonClass} onClick={() => run("enable-rag", enableRag)}>Dùng RAG local (1024d)</button>}
          </div>
        </fieldset>;
      })}
      <button disabled={disabled} className={`${buttonClass} bg-primary text-primary-foreground`} onClick={() => run("enable-meeting", enableMeeting)}>
        {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <Play className="h-4 w-4" />}Áp dụng chế độ Việt–Nhật
      </button>
      {busy && <button className={`${buttonClass} ml-2`} onClick={async () => { try { for (const role of ["translation", "copilot", "embeddings"] as const) await stopLocalModel(role); } catch (e) { setError(String(e)); } }}>Hủy khởi động</button>}
      <p className="text-xs leading-relaxed text-muted-foreground">Chuẩn bị model bằng <code>scripts/setup-local-models.ps1</code>. RTX 4050 6 GB: ưu tiên GPU cho dịch; copilot và embedding bắt đầu trên CPU. Copilot dùng API vẫn chọn được trong LLM Providers.</p>
    </div>
  );
}
