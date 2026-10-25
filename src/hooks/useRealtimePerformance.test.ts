import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useRealtimePerformance, type SttPerformance } from "./useRealtimePerformance";

const events = vi.hoisted(() => ({
  handlers: new Map<string, (event: { payload: unknown }) => void>(),
  unlisten: vi.fn(),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, handler: (event: { payload: unknown }) => void) => {
    events.handlers.set(name, handler);
    return events.unlisten;
  }),
}));
const metrics: SttPerformance = {
  backend:"Vulkan", device:"RTX 4050", audio_ms:3000, inference_ms:250,
  queue_ms:0, lag_ms:250, queued_audio_ms:0, dropped_audio_ms:0,
  processed_audio_ms:3000, total_inference_ms:250, error:null,
};
beforeEach(() => { events.handlers.clear(); events.unlisten.mockClear(); });
afterEach(cleanup);

describe("realtime diagnostics", () => {
  it("keeps microphone/system measurements separate and ignores another meeting", () => {
    const { result } = renderHook(() => useRealtimePerformance("current", true));
    act(() => {
      events.handlers.get("stt_performance")?.({ payload:{ meeting_id:"old", source:"Mic", metrics } });
      events.handlers.get("stt_performance")?.({ payload:{ meeting_id:"current", source:"System", metrics } });
    });
    expect(result.current.mic).toBeUndefined();
    expect(result.current.system).toEqual(metrics);
    act(() => events.handlers.get("translation_performance")?.({ payload:{ meeting_id:"current", queue_ms:200, inference_ms:300 } }));
    expect(result.current.translation?.queue_ms).toBe(200);
  });

  it("clears stale timings and ignores late events when capture ends", async () => {
    const { result, rerender } = renderHook(({ active }) => useRealtimePerformance("current", active), { initialProps:{ active:true } });
    const old = events.handlers.get("stt_performance");
    act(() => old?.({ payload:{ meeting_id:"current", source:"Mic", metrics } }));
    expect(result.current.mic?.backend).toBe("Vulkan");
    rerender({ active:false });
    act(() => old?.({ payload:{ meeting_id:"current", source:"Mic", metrics } }));
    expect(result.current).toEqual({});
    await act(async () => {});
    expect(events.unlisten).toHaveBeenCalledTimes(2);
  });
});
