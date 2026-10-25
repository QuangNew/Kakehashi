import { beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => null) }));
vi.mock("@tauri-apps/plugin-store", () => ({ load: vi.fn(async () => ({ set: vi.fn(), get: vi.fn(), onKeyChange: vi.fn() })) }));
import { initMeetingSpeakers, resetMeetingScopedStores } from "./resetMeeting";
import { useTranscriptStore } from "./transcriptStore";
import { useStreamStore } from "./streamStore";
import { useSpeakerStore } from "./speakerStore";
import { useTranslationStore } from "./translationStore";
import { useAudioPlayerStore } from "./audioPlayerStore";
import { useBookmarkStore } from "./bookmarkStore";
import { useCallLogStore } from "./callLogStore";
import { useConfigStore } from "./configStore";
import { applyPreset, BUILT_IN_PRESETS } from "../settings/presets";

beforeEach(async () => { await resetMeetingScopedStores(); });

describe("meeting isolation", () => {
  it("clears old meeting data and playback without changing local AI preferences", async () => {
    const audioConfig = applyPreset(BUILT_IN_PRESETS[0], []);
    useConfigStore.setState({ llmProvider: "local_gguf", llmModel: "copilot", meetingAudioConfig: audioConfig });
    useTranslationStore.setState({ provider: "hy-mt", bidirectional: true, direction: "vi-ja", lastError: "old failure", batchProgress: { completed: 1, total: 9 }, autoTranslateActive: true });
    useTranscriptStore.getState().appendSegment({ id: "old", speaker: "User", text: "Nội dung cuộc họp cũ", is_final: true, timestamp_ms: 0, confidence: 1 });
    useTranscriptStore.getState().setSearchQuery("old search");
    useTranslationStore.getState().addTranslation({ segment_id: "old", original_text: "Nội dung cuộc họp cũ", translated_text: "前の会議", source_lang: "vi", target_lang: "ja", provider: "hy-mt" });
    useSpeakerStore.getState().initForOnline();
    useSpeakerStore.getState().renameSpeaker("them", "Previous customer");
    useStreamStore.setState({ currentContent: "Old AI response", isStreaming: true, pendingRequestId: "old-request" });
    useBookmarkStore.getState().addBookmark(100, "old note");
    useCallLogStore.getState().setOpen(true);
    useAudioPlayerStore.setState({ isPlaying: true, currentTimeMs: 500, activeSegmentId: "old" });

    await resetMeetingScopedStores();
    expect(useTranscriptStore.getState()).toMatchObject({ segments: [], searchQuery: "" });
    expect(useStreamStore.getState()).toMatchObject({ isStreaming: false, currentContent: "", pendingRequestId: null });
    expect(useSpeakerStore.getState().getAllSpeakers()).toEqual([]);
    expect(useBookmarkStore.getState().bookmarks).toEqual([]);
    expect(useCallLogStore.getState()).toMatchObject({ entries: [], isOpen: false });
    expect(useAudioPlayerStore.getState()).toMatchObject({ isPlaying: false, currentTimeMs: 0, activeSegmentId: null });
    expect(useTranslationStore.getState()).toMatchObject({ lastError: null, batchProgress: null, autoTranslateActive: false, provider: "hy-mt", bidirectional: true, direction: "vi-ja" });
    expect(useTranslationStore.getState().translations.size).toBe(0);
    expect(useConfigStore.getState()).toMatchObject({ llmProvider: "local_gguf", llmModel: "copilot", meetingAudioConfig: audioConfig });
  });

  it("initializes fresh You/Them identities after resetting a previous meeting", async () => {
    initMeetingSpeakers("online");
    useSpeakerStore.getState().renameSpeaker("them", "Old customer");
    await resetMeetingScopedStores();
    expect(initMeetingSpeakers("online")).toBe(true);
    expect(useSpeakerStore.getState().getSpeakerDisplayName("them")).toBe("Them");
    expect(useSpeakerStore.getState().speakerOrder).toEqual(["you", "them"]);
    expect(useSpeakerStore.getState().getSpeaker("them")?.stats.segment_count).toBe(0);
  });
});
