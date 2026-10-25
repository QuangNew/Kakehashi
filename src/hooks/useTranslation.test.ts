import { act, renderHook, cleanup } from "@testing-library/react";
import { beforeEach, afterEach, describe, expect, it, vi } from "vitest";
import type { Meeting, TranscriptSegment, TranslationResult } from "../lib/types";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), final: undefined as undefined | ((event: { segment: TranscriptSegment }) => void) }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/plugin-store", () => ({ load: vi.fn(async () => ({ set: vi.fn(), get: vi.fn(), onKeyChange: vi.fn() })) }));
vi.mock("../lib/ipc", () => ({ getAllMeetingTranslations: vi.fn(async () => []), setTranslationLanguages: vi.fn(async () => {}) }));
vi.mock("../lib/events", () => ({
  onTranscriptFinal: vi.fn(async (callback) => { mocks.final = callback; return vi.fn(); }),
  onTranslationResult: vi.fn(async () => vi.fn()), onTranslationError: vi.fn(async () => vi.fn()),
  onBatchTranslationProgress: vi.fn(async () => vi.fn()),
}));
vi.mock("../stores/meetingStore", async () => {
  const { create } = await import("zustand");
  return { useMeetingStore: create(() => ({ activeMeeting: null })) };
});
vi.mock("../stores/transcriptStore", async () => {
  const { create } = await import("zustand");
  return { useTranscriptStore: create(() => ({ segments: [] })) };
});

import { useTranslation } from "./useTranslation";
import { useTranslationStore } from "../stores/translationStore";
import { useMeetingStore } from "../stores/meetingStore";

const line = (id: string, speaker: TranscriptSegment["speaker"], text: string): TranscriptSegment => ({ id, speaker, text, is_final: true, timestamp_ms: 0, confidence: 1 });
const requestCalls = () => mocks.invoke.mock.calls.filter(([command]) => command === "translate_live_segment");

beforeEach(() => {
  vi.useFakeTimers(); mocks.invoke.mockReset(); mocks.final = undefined;
  mocks.invoke.mockImplementation(async (command, args) => command === "begin_live_translation" ? `session-${args.meetingId}` : null);
  useMeetingStore.setState({ activeMeeting: { id: "meeting" } as Meeting });
  useTranslationStore.setState({ ...useTranslationStore.getInitialState(), translations: new Map(), translating: new Set() });
});
afterEach(() => { cleanup(); vi.useRealTimers(); });

describe("live bilingual translation lifecycle", () => {
  it("sends the correct source and target for both audio sides", async () => {
    renderHook(() => useTranslation());
    await act(async () => {});
    await act(async () => {
      mocks.final?.({ segment: line("you", "User", "Xin chào") });
      mocks.final?.({ segment: line("them", "Them", "こんにちは") });
      await vi.advanceTimersByTimeAsync(210);
    });
    expect(requestCalls().map(([, args]) => [args.segmentId, args.sourceLang, args.targetLang])).toEqual([["you", "vi", "ja"], ["them", "ja", "vi"]]);
  });
  it("coalesces final corrections that arrive during the debounce", async () => {
    renderHook(() => useTranslation()); await act(async () => {});
    await act(async () => {
      mocks.final?.({ segment: line("same", "User", "Xin") });
      mocks.final?.({ segment: line("same", "User", "Xin chào") });
      await vi.advanceTimersByTimeAsync(210);
    });
    expect(requestCalls()).toHaveLength(1);
    expect(requestCalls()[0][1].text).toBe("Xin chào");
  });
  it("ignores an in-flight reply after the meeting ends", async () => {
    let finish!: (value: TranslationResult) => void;
    mocks.invoke.mockImplementation((command, args) => {
      if (command === "begin_live_translation") return Promise.resolve(`session-${args.meetingId}`);
      if (command === "translate_live_segment") return new Promise((resolve) => { finish = resolve; });
      return Promise.resolve(null);
    });
    renderHook(() => useTranslation()); await act(async () => {});
    await act(async () => { mocks.final?.({ segment: line("old", "User", "Xin chào") }); await vi.advanceTimersByTimeAsync(210); });
    await act(async () => { useMeetingStore.setState({ activeMeeting: null }); });
    await act(async () => { finish({ segment_id: "old", original_text: "Xin chào", translated_text: "こんにちは", source_lang: "vi", target_lang: "ja", provider: "local" }); });
    expect(useTranslationStore.getState().translations.size).toBe(0);
    expect(mocks.invoke).toHaveBeenCalledWith("end_live_translation", { sessionId: "session-meeting" });
  });
  it("does not start a second translation session in a display-only window", async () => {
    useTranslationStore.setState({ autoTranslateActive: true });
    renderHook(() => useTranslation(false)); await act(async () => {});
    expect(mocks.invoke.mock.calls.some(([command]) => command === "begin_live_translation")).toBe(false);
  });
});
