import { beforeEach, describe, expect, it, vi } from "vitest";
const saved = vi.hoisted(() => new Map<string,unknown>());
const listeners = vi.hoisted(() => [] as Array<{key:string; callback:(value:unknown) => void}>);
const invoke = vi.hoisted(() => vi.fn(async (_command:string,_args?:unknown):Promise<unknown> => null));
vi.mock("@tauri-apps/api/core", () => ({invoke}));
vi.mock("@tauri-apps/plugin-store", () => ({load: async () => ({
  get:async (key:string) => saved.get(key),
  set:async (key:string,value:unknown) => { saved.set(key,value); listeners.filter(l => l.key === key).forEach(l => l.callback(structuredClone(value))); },
  onKeyChange:async (key:string,callback:(value:unknown) => void) => { listeners.push({key,callback}); return () => {}; },
})}));
async function windowStore() { vi.resetModules(); return (await import("./translationStore")).useTranslationStore; }
describe("translation provider persistence", () => {
  beforeEach(() => { saved.clear(); listeners.length=0; invoke.mockReset().mockResolvedValue(null); });
  it("restores an API model on restart and shares it across windows independently of copilot", async () => {
    const launcher = await windowStore(); await launcher.getState().loadConfig();
    const overlay = await windowStore(); await overlay.getState().loadConfig();
    await launcher.getState().activateProvider("groq","translation-model");
    expect(overlay.getState()).toMatchObject({provider:"groq",apiModels:{groq:"translation-model"}});
    const restarted = await windowStore(); await restarted.getState().loadConfig();
    expect(restarted.getState()).toMatchObject({provider:"groq",apiModels:{groq:"translation-model"}});
    expect(invoke).toHaveBeenCalledWith("set_translation_provider",{provider:"groq",model:"translation-model"});
    expect(invoke.mock.calls.some(([command]) => command === "set_llm_provider")).toBe(false);
    expect(saved.has("llmProvider")).toBe(false);
  });
  it("remembers a distinct model for each translation API", async () => {
    const store = await windowStore();
    await store.getState().activateProvider("groq","groq-model");
    await store.getState().activateProvider("gemini","gemini-model");
    expect(saved.get("apiModels")).toEqual({groq:"groq-model",gemini:"gemini-model"});
    expect(saved.get("provider")).toBe("gemini");
  });
  it("restores the optional shared-copilot translator without changing copilot configuration", async () => {
    saved.set("provider","llm");
    const store = await windowStore(); await store.getState().loadConfig();
    expect(store.getState().provider).toBe("llm"); expect(saved.get("provider")).toBe("llm");
    expect(store.getState().lastError).toBeNull();
    expect(invoke.mock.calls.some(([command]) => command === "set_llm_provider")).toBe(false);
  });
  it.each(["google", "deepl", "opus-mt"] as const)("keeps the restored %s provider across restart", async provider => {
    saved.set("provider", provider);
    const store = await windowStore(); await store.getState().loadConfig();
    expect(store.getState().provider).toBe(provider);
    expect(store.getState().lastError).toBeNull();
  });
  it.each(["vi-ja","ja-vi"] as const)("keeps STT source languages in sync when selecting %s", async direction => {
    const translation = await windowStore();
    const { useConfigStore } = await import("./configStore");
    useConfigStore.setState({ meetingAudioConfig:{
      you:{role:"You",device_id:"mic",is_input_device:true,stt_provider:"whisper_cpp",language:"vi-VN"},
      them:{role:"Them",device_id:"headphones",is_input_device:false,stt_provider:"whisper_cpp",language:"ja-JP"},
      recording_enabled:true,preset_name:null,
    },llmProvider:"groq",llmModel:"copilot-model" });
    translation.getState().setDirection(direction);
    const config = useConfigStore.getState();
    expect(config.meetingAudioConfig?.you).toMatchObject({device_id:"mic",language:direction === "vi-ja" ? "vi-VN" : "ja-JP"});
    expect(config.meetingAudioConfig?.them).toMatchObject({device_id:"headphones",language:direction === "vi-ja" ? "ja-JP" : "vi-VN"});
    expect(config.meetingAudioConfig?.recording_enabled).toBe(true);
    expect(config.llmProvider).toBe("groq");expect(config.llmModel).toBe("copilot-model");
  });
});
