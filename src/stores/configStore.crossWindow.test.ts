// Two windows (launcher + overlay) each have their own configStore. They meet
// in the plugin-store: a write in one window emits `store://change` to every
// window, and `onKeyChange` applies it. This models that with one shared map
// and a broadcast to every window's listeners.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const saved = vi.hoisted(() => new Map<string, unknown>());
const keyListeners = vi.hoisted(() => [] as Array<{ key: string; cb: (v: unknown) => void }>);
const invoke = vi.hoisted(() => vi.fn(async (_cmd: string, _args?: unknown): Promise<unknown> => null));

vi.mock("@tauri-apps/plugin-store", () => ({
  load: async () => ({
    get: async (key: string) => saved.get(key),
    set: async (key: string, value: unknown) => {
      saved.set(key, value);
      // Like the Rust side: every window (the writer included) hears the change.
      for (const l of keyListeners) if (l.key === key) l.cb(structuredClone(value));
    },
    onKeyChange: async (key: string, cb: (v: unknown) => void) => {
      keyListeners.push({ key, cb });
      return () => {};
    },
  }),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: (cmd: string, args?: unknown) => invoke(cmd, args) }));

/** A window: a fresh module graph (own Zustand store) that has loaded its config. */
async function openWindow() {
  vi.resetModules();
  const { useConfigStore } = await import("./configStore");
  await useConfigStore.getState().loadConfig();
  return useConfigStore;
}

describe("local bilingual audio across launches", () => {
  beforeEach(() => { saved.clear(); keyListeners.length = 0; invoke.mockClear(); });
  afterEach(() => { vi.restoreAllMocks(); });

  it("keeps Whisper Large v3 Turbo and each source language in both windows and after restart", async () => {
    const { applyPreset, BUILT_IN_PRESETS } = await import("../settings/presets");
    const preset = applyPreset(BUILT_IN_PRESETS[0], []);
    saved.set("meetingAudioConfig", preset);
    saved.set("sttProvider", "whisper_cpp");
    saved.set("llmProvider", "local_gguf");
    saved.set("llmModel", "copilot");
    for (let index = 0; index < 3; index++) {
      const window = await openWindow();
      expect(window.getState().meetingAudioConfig).toEqual(preset);
      expect(window.getState().sttProvider).toBe("whisper_cpp");
      expect(window.getState().llmProvider).toBe("local_gguf");
      expect(saved.get("meetingAudioConfig")).toEqual(preset);
    }
  });

  it("restores missing Whisper model IDs with the multilingual Large v3 Turbo default", async () => {
    saved.set("meetingAudioConfig", {
      you: { role: "You", device_id: "default", is_input_device: true, stt_provider: "whisper_cpp", language: "vi-VN" },
      them: { role: "Them", device_id: "default", is_input_device: false, stt_provider: "whisper_cpp", language: "ja-JP" },
      recording_enabled: false, preset_name: null,
    });
    const window = await openWindow();
    expect(window.getState().meetingAudioConfig).toMatchObject({
      you: { stt_provider: "whisper_cpp", local_model_id: "large-v3-turbo", language: "vi-VN" },
      them: { stt_provider: "whisper_cpp", local_model_id: "large-v3-turbo", language: "ja-JP" },
    });
  });
});

describe("custom API endpoint persistence", () => {
  beforeEach(() => { saved.clear(); keyListeners.length = 0; invoke.mockReset(); invoke.mockResolvedValue(null); });
  afterEach(() => { invoke.mockReset().mockResolvedValue(null); vi.restoreAllMocks(); });

  it("shares the endpoint between windows and restores it with its credential after restart", async () => {
    const launcher = await openWindow();
    const overlay = await openWindow();
    launcher.getState().setCustomLlmConfig("http://127.0.0.1:8000/v1", "bearer");
    await vi.waitFor(() => expect(overlay.getState().customLlmBaseUrl).toBe("http://127.0.0.1:8000/v1"));
    invoke.mockImplementation(async (command: string) => command === "get_api_key" ? "test-token" : null);
    await launcher.getState().activateLLMProvider("custom", { model: "meeting-model" });
    const provider = JSON.stringify({ provider_type: "custom", base_url: "http://127.0.0.1:8000/v1", auth_type: "bearer", auth_value: "test-token" });
    expect(invoke).toHaveBeenCalledWith("set_llm_provider", { provider });
    expect(saved.get("customLlmBaseUrl")).toBe("http://127.0.0.1:8000/v1");
    expect(JSON.stringify([...saved.values()])).not.toContain("test-token");
    invoke.mockClear();
    const nextStart = await openWindow();
    expect(nextStart.getState().customLlmAuthType).toBe("bearer");
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("set_llm_provider", { provider }));
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("set_active_model", { provider: "custom", modelId: "meeting-model" }));
  });

  it("does not attach a previously saved credential when authentication is disabled", async () => {
    const window = await openWindow();
    window.getState().setCustomLlmConfig("http://127.0.0.1:8000/v1", "none");
    invoke.mockImplementation(async () => "old-token");
    await window.getState().activateLLMProvider("custom", { model: "meeting-model" });
    expect(invoke).toHaveBeenCalledWith("set_llm_provider", {
      provider: JSON.stringify({ provider_type: "custom", base_url: "http://127.0.0.1:8000/v1" }),
    });
  });
});

describe("local context budget synchronization", () => {
  beforeEach(() => { saved.clear(); keyListeners.length = 0; invoke.mockReset(); });
  afterEach(() => { invoke.mockReset().mockResolvedValue(null); vi.restoreAllMocks(); });

  it("replaces the startup budget after the saved local provider finishes restoring", async () => {
    saved.set("llmProvider", "local_gguf");
    saved.set("llmModel", "copilot");
    let finishRestore!: () => void;
    const restoring = new Promise<void>((resolve) => { finishRestore = resolve; });
    let limit = 128_000;
    invoke.mockImplementation(async (command: string) => {
      if (command === "set_llm_provider") { await restoring; limit = 4096; }
      if (command === "get_token_budget") return JSON.stringify({ total: 283, limit, segments: [] });
      return null;
    });
    await openWindow();
    const { useContextStore } = await import("./contextStore");
    await useContextStore.getState().refreshTokenBudget();
    expect(useContextStore.getState().tokenBudget?.limit).toBe(128_000);
    finishRestore();
    await vi.waitFor(() => expect(useContextStore.getState().tokenBudget?.limit).toBe(4096));
  });

  it("updates both windows when switching between local and API copilot", async () => {
    saved.set("llmProvider", "groq");
    saved.set("llmModel", "copilot-api");
    let limit = 128_000;
    invoke.mockImplementation(async (command: string, args?: unknown) => {
      if (command === "set_llm_provider") {
        const provider = JSON.parse((args as { provider: string }).provider).provider_type;
        limit = provider === "local_gguf" ? 4096 : 128_000;
      }
      if (command === "get_token_budget") return JSON.stringify({ total: 283, limit, segments: [] });
      return null;
    });
    const launcher = await openWindow();
    const launcherContext = (await import("./contextStore")).useContextStore;
    await vi.waitFor(() => expect(launcherContext.getState().tokenBudget?.limit).toBe(128_000));
    await openWindow();
    const overlayContext = (await import("./contextStore")).useContextStore;
    await vi.waitFor(() => expect(overlayContext.getState().tokenBudget?.limit).toBe(128_000));
    await launcher.getState().activateLLMProvider("local_gguf", { model: "copilot" });
    await vi.waitFor(() => {
      expect(launcherContext.getState().tokenBudget?.limit).toBe(4096);
      expect(overlayContext.getState().tokenBudget?.limit).toBe(4096);
    });
    await launcher.getState().activateLLMProvider("openai", { model: "test-api-model" });
    await vi.waitFor(() => {
      expect(launcherContext.getState().tokenBudget?.limit).toBe(128_000);
      expect(overlayContext.getState().tokenBudget?.limit).toBe(128_000);
    });
  });
});

describe("configStore: active LLM across windows", () => {
  beforeEach(() => {
    saved.clear();
    keyListeners.length = 0;
    invoke.mockClear();
    saved.set("llmProvider", "ollama");
    saved.set("llmModel", "llama3.2");
    vi.spyOn(console, "log").mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("a provider set in one window reaches the other", async () => {
    const launcher = await openWindow();
    const overlay = await openWindow();
    expect(overlay.getState().llmProvider).toBe("ollama");

    launcher.getState().setLLMProvider("claude_cli");
    launcher.getState().setLLMModel("haiku", "claude_cli");

    await vi.waitFor(() => expect(overlay.getState().llmProvider).toBe("claude_cli"));
    expect(overlay.getState().llmModel).toBe("haiku");
  });

  it("the Claude model catalog fetched by the launcher reaches the overlay and the next start", async () => {
    const launcher = await openWindow();
    const overlay = await openWindow();
    expect(overlay.getState().claudeModels).toBeNull();
    const catalog = [
      {
        value: "haiku",
        resolved_model: "claude-haiku-4-5-20251001",
        display_name: "Haiku 4.5",
        description: "",
        supports_effort: false,
        effort_levels: [],
      },
    ];
    launcher.getState().setClaudeModels(catalog);
    await vi.waitFor(() => expect(overlay.getState().claudeModels).toEqual(catalog));
    const nextStart = await openWindow();
    expect(nextStart.getState().claudeModels).toEqual(catalog);
  });

  it("the transcript scope picked in the overlay reaches the launcher and the next start", async () => {
    const launcher = await openWindow();
    const overlay = await openWindow();
    expect(overlay.getState().contextScope).toEqual({ kind: "default", minutes: 5 });
    expect(overlay.getState().contextScopeSticky).toBe(true);

    overlay.getState().setContextScope({ kind: "minutes", minutes: 10 });
    overlay.getState().setContextScopeSticky(false);
    await vi.waitFor(() => expect(launcher.getState().contextScope).toEqual({ kind: "minutes", minutes: 10 }));
    expect(launcher.getState().contextScopeSticky).toBe(false);

    const nextStart = await openWindow();
    expect(nextStart.getState().contextScope).toEqual({ kind: "minutes", minutes: 10 });
    expect(nextStart.getState().contextScopeSticky).toBe(false);
  });

  it("a malformed saved scope falls back to the default", async () => {
    saved.set("contextScope", { kind: "bogus", minutes: 3 });
    const w = await openWindow();
    expect(w.getState().contextScope).toEqual({ kind: "default", minutes: 5 });
  });

  it("activating in either window updates the backend and both windows", async () => {
    const launcher = await openWindow();
    const overlay = await openWindow();

    // Settings (launcher) → Claude.
    await launcher.getState().activateLLMProvider("claude_cli");
    expect(invoke).toHaveBeenCalledWith("set_llm_provider", { provider: JSON.stringify({ provider_type: "claude_cli" }) });
    expect(invoke).toHaveBeenCalledWith("set_active_model", { provider: "claude_cli", modelId: "haiku" });
    await vi.waitFor(() => expect(overlay.getState().llmProvider).toBe("claude_cli"));
    expect(launcher.getState().verifiedCloudProviders).toContain("claude_cli");

    // Footer (overlay) → back to Ollama with its last model.
    await overlay.getState().activateLLMProvider("ollama");
    await vi.waitFor(() => expect(launcher.getState().llmProvider).toBe("ollama"));
    expect(launcher.getState().llmModel).toBe("llama3.2");
    expect(invoke.mock.calls.filter(([command]) => command === "set_active_model").slice(-1)[0])
      .toEqual(["set_active_model", { provider: "ollama", modelId: "llama3.2" }]);
  });
});
