import { beforeEach, describe, expect, it, vi } from "vitest";
const mock=vi.hoisted(()=>({load:vi.fn(),disk:new Map<string,unknown>(),save:vi.fn(),set:vi.fn()}));
vi.mock("@tauri-apps/plugin-store",()=>({load:mock.load}));
beforeEach(()=>{
  vi.resetModules();mock.disk.clear();mock.save.mockReset().mockResolvedValue(undefined);
  const memory=new Map<string,unknown>();
  mock.set.mockReset().mockImplementation(async(key,value)=>{memory.set(key,structuredClone(value));});
  mock.save.mockImplementation(async()=>{for(const [key,value] of memory)mock.disk.set(key,value);});
  mock.load.mockReset().mockImplementation(async()=>({set:mock.set,save:mock.save,get:async(key:string)=>memory.get(key)}));
});
describe("durable user settings",()=>{
  it("shares a store while writes are in flight and flushes the latest value to disk",async()=>{
    const settings=await import("./persistedSettings");
    await Promise.all([settings.writeSetting("config.json","language","ja"),settings.writeSetting("config.json","language","vi"),settings.writeSetting("config.json","preloadLocalModels",true)]);
    await settings.flushSettings();
    expect(mock.load).toHaveBeenCalledTimes(1);
    expect(mock.disk.get("language")).toBe("vi");
    expect(mock.disk.get("preloadLocalModels")).toBe(true);
    expect(settings.useSettingsSaveState.getState().error).toBeNull();
  });
  it("retains a failed write, reports it, and retries when the user saves again",async()=>{
    const settings=await import("./persistedSettings");
    mock.save.mockRejectedValueOnce(new Error("disk unavailable"));
    await expect(settings.writeSetting("config.json","language","ja")).rejects.toThrow("disk unavailable");
    expect(settings.useSettingsSaveState.getState().error).toContain("disk unavailable");
    expect(mock.disk.has("language")).toBe(false);
    await settings.flushSettings();
    expect(mock.disk.get("language")).toBe("ja");
    expect(settings.useSettingsSaveState.getState().error).toBeNull();
  });
});
