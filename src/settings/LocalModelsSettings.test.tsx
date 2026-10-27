import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, afterEach, describe, expect, it, vi } from "vitest";
const mocks = vi.hoisted(() => ({
  start: vi.fn(), save: vi.fn(), activate: vi.fn(), setAudio: vi.fn(), setProvider: vi.fn(), updateRag: vi.fn(),
  engines: vi.fn(),
  draft: null as unknown,
  activeMeeting: null as object | null,
}));
vi.mock("../lib/localModels", async (original) => ({
  ...(await original<typeof import("../lib/localModels")>()),
  getLocalModelsConfig: vi.fn(async () => ({ models_dir: "C:/models", server_path: "C:/runtime/llama-server.exe",
    translation: { file: "translation.gguf", context: 2048, gpu_layers: 99 },
    copilot: { file: "copilot.gguf", context: 4096, gpu_layers: 0 },
    embeddings: { file: "embedding.gguf", context: 2048, gpu_layers: 0 },
  })), getLocalModelsStatus: vi.fn(async () => []), saveLocalModelsConfig: mocks.save, startLocalModel: mocks.start, stopLocalModel: vi.fn(async () => {}),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(async () => "C:/別のフォルダー/models") }));
vi.mock("../lib/ipc", () => ({ listAudioDevices: vi.fn(async () => ({ inputs: [], outputs: [] })), setTranslationProvider: mocks.setProvider,
  listLocalSTTEngines: mocks.engines,
  getTokenBudget: vi.fn(async () => ({ limit: 4096, total: 0, segments: [] })),
  getRagConfig: vi.fn(async () => ({ enabled: false, embedding_model: "nomic-embed-text", embedding_dimensions: 768 })), updateRagConfig: mocks.updateRag,
}));
vi.mock("../stores/configStore", () => ({ useConfigStore: { getState: () => ({ setActiveWhisperModel: vi.fn(), setActiveModelForEngine: vi.fn(), setMeetingAudioConfig: mocks.setAudio, activateLLMProvider: mocks.activate, setContextStrategy: vi.fn() }) } }));
vi.mock("../stores/meetingStore", () => ({ useMeetingStore: (select: (state: { activeMeeting: object | null }) => unknown) => select({ activeMeeting: mocks.activeMeeting }) }));
vi.mock("../stores/translationStore", () => {
  const state = { direction: "vi-ja", setDirection: vi.fn(), setProvider: vi.fn(), setBidirectional: vi.fn(), setDisplayMode: vi.fn(), setAutoTranslateEnabled: vi.fn() };
  return { useTranslationStore: Object.assign((select: (state: object) => unknown) => select(state), { getState: () => state }) };
});
vi.mock("../stores/aiActionsStore", () => ({ useAIActionsStore: { getState: () => ({ configs: { customInstructions: "" }, setCustomInstructions: vi.fn() }) } }));
vi.mock("../stores/ragStore", () => ({ useRagStore: { getState: () => ({ loadRagConfig: vi.fn(async () => {}), markSourcesChanged: vi.fn() }) } }));
vi.mock("../lib/persistedSettings", () => ({
  getSettingsStore: async () => ({get: async () => mocks.draft}),
  writeSetting: vi.fn(async () => {}),
}));
vi.mock("../components/ModelPreparationControls", () => ({ModelPreparationControls: () => null}));
import { LocalModelsSettings } from "./LocalModelsSettings";

beforeEach(() => { vi.clearAllMocks(); mocks.draft=null; mocks.activeMeeting = null; mocks.start.mockResolvedValue("local"); mocks.save.mockResolvedValue(undefined); mocks.activate.mockResolvedValue(undefined); mocks.engines.mockResolvedValue([{ engine: "whisper_cpp", models: [{ id: "large-v3-turbo", is_downloaded: true }] }]); });
afterEach(cleanup);
describe("local model setup", () => {
  it("does not show a pending draft when plugin-store reorders JSON properties",async()=>{
    const backend=await (await import("../lib/localModels")).getLocalModelsConfig();
    const reorder=(value:unknown):unknown=>value && typeof value==="object" ? Object.fromEntries(Object.entries(value).sort(([a],[b])=>a.localeCompare(b)).map(([key,value])=>[key,reorder(value)])) : value;
    mocks.draft={config:reorder(backend),base:JSON.stringify(reorder(backend))};
    render(<LocalModelsSettings/>);
    expect(await screen.findByRole("textbox",{name:"Models folder"})).toHaveValue(backend.models_dir);
    expect(screen.queryByText(/Bản nháp đã lưu/)).not.toBeInTheDocument();
  });
  it("applies bilingual audio and the dedicated translator without switching the copilot", async () => {
    render(<LocalModelsSettings />);
    fireEvent.click(await screen.findByRole("button", { name: "Áp dụng chế độ Việt–Nhật" }));
    await waitFor(() => expect(mocks.setAudio).toHaveBeenCalled());
    expect(mocks.start).toHaveBeenCalledWith("translation");
    expect(mocks.setProvider).toHaveBeenCalledWith("hy-mt");
    expect(mocks.setAudio.mock.calls[0][0]).toMatchObject({ you: { language: "vi-VN", stt_provider: "whisper_cpp" }, them: { language: "ja-JP", stt_provider: "whisper_cpp" } });
    expect(mocks.activate).not.toHaveBeenCalled();
  });
  it("activates the local copilot separately", async () => {
    render(<LocalModelsSettings />);
    fireEvent.click(await screen.findByRole("button", { name: "Dùng copilot local" }));
    await waitFor(() => expect(mocks.activate).toHaveBeenCalledWith("local_gguf", { model: "copilot" }));
    expect(mocks.setProvider).not.toHaveBeenCalled();
  });
  it("shows a missing-model error without applying meeting settings", async () => {
    mocks.start.mockRejectedValueOnce("GGUF model not found");
    render(<LocalModelsSettings />);
    fireEvent.click(await screen.findByRole("button", { name: "Áp dụng chế độ Việt–Nhật" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("GGUF model not found");
    expect(mocks.setAudio).not.toHaveBeenCalled();
  });
  it("configures local multilingual embedding instead of Ollama", async () => {
    render(<LocalModelsSettings />);
    fireEvent.click(await screen.findByRole("button", { name: "Dùng RAG local (1024d)" }));
    await waitFor(() => expect(mocks.updateRag).toHaveBeenCalledWith(expect.objectContaining({ embedding_provider: "local_gguf", embedding_model: "embeddings", embedding_dimensions: 1024 })));
  });
  it("requires the multilingual Whisper model before applying the audio preset", async () => {
    mocks.engines.mockResolvedValueOnce([]);
    render(<LocalModelsSettings />);
    fireEvent.click(await screen.findByRole("button", { name: "Áp dụng chế độ Việt–Nhật" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Thiếu Whisper Large v3 Turbo");
    expect(mocks.setAudio).not.toHaveBeenCalled();
    expect(mocks.start).not.toHaveBeenCalled();
  });
  it("protects model configuration during a meeting", async () => {
    mocks.activeMeeting = { id: "active" };
    render(<LocalModelsSettings />);
    expect(await screen.findByRole("button", { name: "Áp dụng chế độ Việt–Nhật" })).toBeDisabled();
  });
});
