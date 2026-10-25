import { startPastMeetingSession, closePastMeetingSession } from "../../lib/ipc";
import { useState, useEffect, useCallback, useRef, useMemo } from "react";
import type { Meeting, MeetingBookmark, RecordingInfo, TranslationResult } from "../../lib/types";
import { getMeeting, getRecordingInfo, getAllMeetingTranslations, translateSegments, cancelGeneration, renameSpeaker } from "../../lib/ipc";
import { withSpeakerName } from "../../lib/speakerLabels";
import { routeTranslation, selectTranslation, translationKey } from "../../lib/translationRouting";
import { showToast } from "../../stores/toastStore";
import { onTranscriptFinal, onRecordingReady, onTranslationResult, onTranslationError } from "../../lib/events";
import { useMeetingStore } from "../../stores/meetingStore";
import { useTranslationStore } from "../../stores/translationStore";
import { useConfigStore } from "../../stores/configStore";
import { useMeetingStats } from "../../hooks/useMeetingStats";
import { useTranscriptSearch } from "../../hooks/useTranscriptSearch";
import { useSummaryGeneration } from "../../hooks/useSummaryGeneration";
import { useActionItemsExtraction } from "../../hooks/useActionItemsExtraction";
import { useBookmarkSuggestions } from "../../hooks/useBookmarkSuggestions";
import { useMeetingChat, answeredExchanges } from "../../hooks/useMeetingChat";
import { useAudioKeyboardShortcuts } from "../../hooks/useAudioKeyboardShortcuts";
import { exportMeetingAsMarkdown } from "../../lib/export";
import { mergeConsecutiveSegments } from "../../lib/mergeSegments";
import { useTranscriptStore } from "../../stores/transcriptStore";
import { useBookmarkStore } from "../../stores/bookmarkStore";
import { useActionItemStore } from "../../stores/actionItemStore";
import { useSpeakerStore } from "../../stores/speakerStore";
import { MeetingHeader } from "./MeetingHeader";
import { MeetingTabBar, type MeetingTab } from "./MeetingTabBar";
import { TranscriptView } from "./TranscriptView";
import { SummaryView } from "./SummaryView";
import { AIInteractionLog } from "./AIInteractionLog";
import { SpeakersTab } from "./SpeakersTab";
import { ActionItemsTab } from "./ActionItemsTab";
import { BookmarksTab } from "./BookmarksTab";
import { AskMeetingTab } from "./AskMeetingTab";
import { AudioPlayer, AudioPlayerSkeleton } from "../../components/AudioPlayer";
import { Loader2 } from "lucide-react";

interface MeetingDetailsProps {
  meetingId: string;
  onBack: () => void;
}

export function MeetingDetails({ meetingId, onBack }: MeetingDetailsProps) {
  const [meeting, setMeeting] = useState<Meeting | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<MeetingTab>("transcript");
  const [expandedInteraction, setExpandedInteraction] = useState<string | null>(null);
  const [scrollToSegmentIndex, setScrollToSegmentIndex] = useState<number | null>(null);
  const [recordingInfo, setRecordingInfo] = useState<RecordingInfo | null>(null);
  const [recordingProcessing, setRecordingProcessing] = useState(false);

  // Translation state for post-meeting view
  const [translationResults, setTranslationResults] = useState<Map<string, TranslationResult>>(new Map());
  const [translatingSegments, setTranslatingSegments] = useState<Set<string>>(new Set());
  const [translationsVisible, setTranslationsVisible] = useState(true);
  const [retranslating, setRetranslating] = useState(false);

  // Translation store selectors
  const displayMode = useTranslationStore((s) => s.displayMode);
  const setDisplayMode = useTranslationStore((s) => s.setDisplayMode);
  const currentTargetLang = useTranslationStore((s) => s.targetLang);
  const autoTranslateEnabled = useTranslationStore((s) => s.autoTranslateEnabled);
  const bidirectional = useTranslationStore((s) => s.bidirectional);
  const direction = useTranslationStore((s) => s.direction);
  const sourceLang = useTranslationStore((s) => s.sourceLang);
  const translations = useMemo(() => {
    const selected = new Map<string, TranslationResult>();
    for (const segment of meeting?.transcript ?? []) {
      const result = selectTranslation(translationResults, segment, bidirectional, direction, sourceLang, currentTargetLang);
      const previous = translationResults.get(segment.id);
      const paired = previous && ((previous.source_lang === "vi" && previous.target_lang === "ja") || (previous.source_lang === "ja" && previous.target_lang === "vi"));
      if (bidirectional && paired && previous.original_text === segment.text) selected.set(segment.id, previous);
      else if (result) selected.set(segment.id, result);
      else if (!bidirectional && previous?.original_text === segment.text) selected.set(segment.id, previous);
    }
    return selected;
  }, [translationResults, meeting?.transcript, bidirectional, direction, sourceLang, currentTargetLang]);
  const showPostMeetingTranslation = useConfigStore((s) => s.showPostMeetingTranslation);

  const loadMeeting = useCallback(async () => {
    setLoading(true);
    setError(null);

    try {
      const data = await getMeeting(meetingId);
      setMeeting(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load meeting");
    } finally {
      setLoading(false);
    }
  }, [meetingId]);

  useEffect(() => { loadMeeting(); }, [loadMeeting]);

  // Live transcript subscription
  const activeMeetingId = useMeetingStore((s) => s.activeMeeting?.id);
  const isActiveMeeting = activeMeetingId === meetingId;

  // Claude CLI: keep a warm session holding this meeting's transcript and
  // summary while the view is open; closed on leaving. The running meeting
  // uses its live session instead.
  const llmProvider = useConfigStore((s) => s.llmProvider);
  // True while this view's summary / action items / bookmark generation runs.
  const aiBusyRef = useRef(false);
  useEffect(() => {
    if (isActiveMeeting || llmProvider !== "claude_cli") return;
    startPastMeetingSession(meetingId).catch((err) =>
      console.warn("[MeetingDetails] Claude session did not start:", err)
    );
    return () => {
      // Stop this view's answer first: closing the session would cut it off.
      if (aiBusyRef.current) cancelGeneration().catch(() => {});
      closePastMeetingSession(meetingId).catch(() => {});
    };
  }, [meetingId, isActiveMeeting, llmProvider]);



  // Cancel this view's request when it closes; generation guards release after streaming stops.
  useEffect(() => () => { if (aiBusyRef.current) void cancelGeneration().catch(() => {}); }, [meetingId]);

  useEffect(() => {
    if (!isActiveMeeting) return;
    const unlistenPromise = onTranscriptFinal((event) => {
      setMeeting((prev) => {
        if (!prev) return prev;
        const transcript = [...prev.transcript];
        const index = transcript.findIndex((segment) => segment.id === event.segment.id);
        if (index < 0) transcript.push(event.segment); else transcript[index] = event.segment;
        return { ...prev, transcript };
      });
    });
    return () => { unlistenPromise.then((unlisten) => unlisten()); };
  }, [isActiveMeeting]);

  // Load recording info when meeting loads (waveform_data is included in the response)
  useEffect(() => {
    if (!meeting?.id) return;
    getRecordingInfo(meeting.id).then((info) => {
      if (info) {
        setRecordingInfo(info);
      }
    }).catch(() => { /* no recording for this meeting */ });
  }, [meeting?.id]);

  // Listen for recording_ready event (fires when post-meeting processing completes)
  useEffect(() => {
    const unlisten = onRecordingReady((data) => {
      if (data.meeting_id === meeting?.id) {
        setRecordingProcessing(false);
        getRecordingInfo(meeting.id).then((info) => {
          if (info) {
            setRecordingInfo(info);
          }
        });
      }
    });
    return () => { unlisten.then((fn) => fn()); };
  }, [meeting?.id]);

  // Load all translations for this meeting
  useEffect(() => {
    let disposed = false;
    setTranslationResults(new Map());
    if (!meetingId) return;
    getAllMeetingTranslations(meetingId).then((results) => {
      if (disposed) return;
      const map = new Map<string, TranslationResult>();
      // Results are ordered by created_at DESC — first match per segment wins
      for (const r of results) {
        if (r.segment_id) map.set(translationKey(r.segment_id, r.target_lang), r);
        if (r.segment_id && !map.has(r.segment_id)) {
          map.set(r.segment_id, r);
        }
      }
      // Second pass: override with currentTargetLang matches
      for (const r of results) {
        if (r.segment_id && r.target_lang === currentTargetLang) {
          map.set(r.segment_id, r);
        }
      }
      setTranslationResults(map);
    }).catch((err) => {
      console.error("[MeetingDetails] Failed to load translations:", err);
    });
    return () => { disposed = true; };
  }, [meetingId, currentTargetLang]);

  // Listen for new translation results (from on-demand translating)
  useEffect(() => {
    const unlistenResult = onTranslationResult((result) => {
      if (result.meeting_id && result.meeting_id !== meetingId) return;
      if (result.segment_id) {
        setTranslationResults((prev) => {
          const next = new Map(prev);
          next.set(translationKey(result.segment_id!, result.target_lang), result);
          next.set(result.segment_id!, result);
          return next;
        });
        setTranslatingSegments((prev) => {
          const next = new Set(prev);
          next.delete(result.segment_id!);
          return next;
        });
      }
    });
    const unlistenError = onTranslationError((error) => {
      if (error.segment_id) {
        setTranslatingSegments((prev) => {
          const next = new Set(prev);
          next.delete(error.segment_id!);
          return next;
        });
      }
      setRetranslating(false);
    });
    return () => {
      unlistenResult.then((fn) => fn());
      unlistenError.then((fn) => fn());
    };
  }, [meetingId]);

  // Audio keyboard shortcuts (Space, Arrow keys, [ ])
  useAudioKeyboardShortcuts();

  // Hooks
  const stats = useMeetingStats(meeting);
  const search = useTranscriptSearch(meeting?.transcript ?? []);
  const summaryGeneration = useSummaryGeneration(meeting, (summary) => {
    setMeeting((prev) => (prev ? { ...prev, summary } : prev));
  });
  const actionExtraction = useActionItemsExtraction(meeting, (items) => {
    setMeeting((prev) => (prev ? { ...prev, action_items: items } : prev));
  });
  const bookmarkSuggestions = useBookmarkSuggestions(meeting, (newBookmark) => {
    setMeeting((prev) =>
      prev
        ? { ...prev, bookmarks: [...(prev.bookmarks ?? []), newBookmark] }
        : prev,
    );
  });
  // "Ask this meeting" is for finished meetings (not the running one, not demo).
  const showAsk = !isActiveMeeting;
  const meetingChat = useMeetingChat(meetingId, {
    enabled: showAsk,
  });
  const chatExchanges = useMemo(() => answeredExchanges(meetingChat.items), [meetingChat.items]);
  const otherAiBusy =
    summaryGeneration.isGenerating || actionExtraction.isExtracting || bookmarkSuggestions.isSuggesting;
  const aiBusy = otherAiBusy || meetingChat.isAsking;
  useEffect(() => {
    aiBusyRef.current = aiBusy;
  }, [aiBusy]);
  const searchInputRef = useRef<HTMLInputElement>(null);

  // Keyboard shortcuts
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "f") {
        if (activeTab === "transcript") {
          e.preventDefault();
          searchInputRef.current?.focus();
        }
      }
      if ((e.ctrlKey || e.metaKey) && e.key === "s") {
        if (activeTab === "summary") {
          e.preventDefault();
          if (!summaryGeneration.isGenerating && meeting && !meeting.summary) {
            summaryGeneration.generate();
          }
        }
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [activeTab, summaryGeneration, meeting]);

  // On-demand translate a single segment
  const handleTranslateSegment = useCallback(async (segmentId: string, _text: string) => {
    if (!meeting) return;
    const group = mergeConsecutiveSegments(meeting.transcript).find((segment) => segment.id === segmentId);
    const ids = new Set(group?.originalIds ?? [segmentId]);
    for (const segment of meeting.transcript.filter((item) => ids.has(item.id))) {
      const saved = translations.get(segment.id);
      const savedSource = saved?.source_lang;
      const route = bidirectional && (savedSource === "vi" || savedSource === "ja")
        ? { source: savedSource, target: savedSource === "vi" ? "ja" : "vi" }
        : routeTranslation(segment, bidirectional, direction, sourceLang, currentTargetLang);
      if (!route) continue;
      setTranslatingSegments((previous) => new Set(previous).add(segment.id));
      try { await translateSegments([segment.id], [segment.text], meetingId, route.target, route.source === "auto" ? undefined : route.source); }
      catch (error) { showToast(String(error), "error"); }
      finally { setTranslatingSegments((previous) => { const next = new Set(previous); next.delete(segment.id); return next; }); }
    }
  }, [meeting, meetingId, translations, bidirectional, direction, sourceLang, currentTargetLang]);

  // Retranslate a single mismatched segment
  const handleRetranslateSegment = useCallback(async (segmentId: string, text: string) => {
    await handleTranslateSegment(segmentId, text);
  }, [handleTranslateSegment]);

  // Retranslate all mismatched segments
  const handleRetranslateAll = useCallback(async () => {
    if (!meeting || !currentTargetLang || bidirectional) return;
    const mismatched = meeting.transcript
      .filter((s) => s.id && translations.has(s.id) && translations.get(s.id)!.target_lang !== currentTargetLang)
      .map((s) => ({ id: s.id!, text: s.text }));
    if (mismatched.length === 0) return;
    setRetranslating(true);
    for (const seg of mismatched) {
      setTranslatingSegments((prev) => new Set(prev).add(seg.id));
    }
    try {
      await translateSegments(
        mismatched.map((s) => s.id),
        mismatched.map((s) => s.text),
        meeting.id,
        currentTargetLang,
      );
    } catch (err) {
      console.error("[MeetingDetails] Retranslate all failed:", err);
    } finally {
      setRetranslating(false);
    }
  }, [meeting, translations, currentTargetLang, bidirectional]);

  // Translation toolbar stats — use merged segments to match what the user sees
  const mergedSegments = useMemo(
    () => meeting ? mergeConsecutiveSegments(meeting.transcript) : [],
    [meeting?.transcript]
  );
  const totalSegments = mergedSegments.length;
  const translatedCount = mergedSegments.filter((segment) => {
    const target = bidirectional ? translations.get(segment.id)?.target_lang : currentTargetLang;
    return !!target && !!selectTranslation(translations, segment, false, direction, sourceLang, target);
  }).length;
  const mismatchedCount = bidirectional ? 0 : mergedSegments.filter(
    (s) => s.id && translations.has(s.id) && translations.get(s.id)!.target_lang !== currentTargetLang
  ).length;
  const showToolbar = showPostMeetingTranslation && (translations.size > 0 || autoTranslateEnabled);

  // Export
  const handleExport = useCallback(async () => {
    if (!meeting) return;
    await exportMeetingAsMarkdown(meeting);
  }, [meeting]);

  const handleTitleChanged = useCallback((title: string) => {
    setMeeting((prev) => (prev ? { ...prev, title } : prev));
  }, []);

  const handleBookmarksChanged = useCallback((bookmarks: MeetingBookmark[]) => {
    setMeeting((prev) => (prev ? { ...prev, bookmarks } : prev));
  }, []);

  // Name a remote speaker (`them:S1`): every line with that tag follows, and
  // the name is saved with the meeting (also used for its AI context).
  const handleRenameSpeaker = useCallback((speakerKey: string, name: string) => {
    setMeeting((prev) => (prev ? { ...prev, speakers: withSpeakerName(prev.speakers, speakerKey, name) } : prev));
    renameSpeaker(meetingId, speakerKey, name).catch((err) => {
      console.error("[MeetingDetails] Failed to save the speaker name:", err);
      showToast("Couldn't save the speaker name", "error");
    });
  }, [meetingId]);

  const handleScrollHandled = useCallback(() => setScrollToSegmentIndex(null), []);

  const handleToggleTranslationVisibility = useCallback(() => setTranslationsVisible((v) => !v), []);

  if (loading) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2">
        <Loader2 className="h-5 w-5 animate-spin text-primary" />
        <p className="text-xs text-muted-foreground">Loading meeting data...</p>
      </div>
    );
  }

  if (error || !meeting) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2">
        <p className="text-xs text-red-400">{error || "Meeting not found"}</p>
        <button onClick={onBack} className="text-xs text-primary hover:underline cursor-pointer">Go back</button>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      <MeetingHeader
        meeting={meeting}
        stats={stats}
        onBack={onBack}
        onTitleChanged={handleTitleChanged}
      />

      <MeetingTabBar
        activeTab={activeTab}
        setActiveTab={setActiveTab}
        meeting={meeting}
        onGenerateSummary={() => {
          setActiveTab("summary");
          if (!summaryGeneration.isGenerating) {
            summaryGeneration.generate();
          }
        }}
        onExtractActions={() => {
          setActiveTab("actions");
          if (!actionExtraction.isExtracting) {
            actionExtraction.extract();
          }
        }}
        onSuggestBookmarks={() => {
          setActiveTab("bookmarks");
          if (!bookmarkSuggestions.isSuggesting) {
            bookmarkSuggestions.suggest();
          }
        }}
        isSummaryGenerating={summaryGeneration.isGenerating}
        isActionsExtracting={actionExtraction.isExtracting}
        isBookmarksSuggesting={bookmarkSuggestions.isSuggesting}
        showAsk={showAsk}
        aiLogCount={meeting.ai_interactions.length + chatExchanges.length}
      />

      <div className={`flex-1 min-h-0 ${activeTab === "transcript" || activeTab === "ask" ? "overflow-hidden flex flex-col" : "overflow-y-auto"}`} role="tabpanel">
        {/* Transcript stays mounted (hidden) so switching back is instant */}
        <div className={activeTab === "transcript" ? "flex flex-1 min-h-0 flex-col" : "hidden"}>
          <TranscriptView
            segments={meeting.transcript}
            mergedSegments={mergedSegments}
            isVisible={activeTab === "transcript"}
            search={search}
            meetingStartTime={new Date(meeting.start_time).getTime()}
            recordingOffsetMs={recordingInfo?.offset_ms ?? 0}
            speakers={meeting.speakers}
            searchInputRef={searchInputRef}
            bookmarks={meeting.bookmarks}
            meetingId={meeting.id}
            onBookmarksChanged={handleBookmarksChanged}
            onRenameSpeaker={handleRenameSpeaker}
            initialScrollToIndex={scrollToSegmentIndex}
            onScrollHandled={handleScrollHandled}
            translations={translations}
            translatingSegments={translatingSegments}
            translationDisplayMode={displayMode}
            currentTargetLang={currentTargetLang}
            translationBidirectional={bidirectional}
            translationDirection={direction}
            showTranslations={translationsVisible}
            onTranslateSegment={handleTranslateSegment}
            onRetranslateSegment={handleRetranslateSegment}
            showToolbar={showToolbar}
            translatedCount={translatedCount}
            totalSegments={totalSegments}
            mismatchedCount={mismatchedCount}
            onDisplayModeChange={setDisplayMode}
            onRetranslateAll={handleRetranslateAll}
            onToggleTranslationVisibility={handleToggleTranslationVisibility}
            retranslating={retranslating}
          />
        </div>
        {activeTab === "summary" && (
          <SummaryView meeting={meeting} generation={summaryGeneration} onExport={handleExport} />
        )}
        {activeTab === "ask" && showAsk && (
          <AskMeetingTab
            chat={meetingChat}
            otherTaskRunning={otherAiBusy}
            unavailableReason={
              meeting.transcript.length === 0 && !meeting.summary
                ? "This meeting has no transcript to ask about"
                : null
            }
          />
        )}
        {activeTab === "ai" && (
          <AIInteractionLog
            interactions={meeting.ai_interactions}
            chat={chatExchanges}
            expandedId={expandedInteraction}
            onToggle={(id) => setExpandedInteraction(expandedInteraction === id ? null : id)}
          />
        )}
        {activeTab === "speakers" && (
          <SpeakersTab
            meeting={meeting}
            onSegmentClick={(idx) => {
              setScrollToSegmentIndex(idx);
              setActiveTab("transcript");
            }}
          />
        )}
        {activeTab === "actions" && (
          <ActionItemsTab
            meeting={meeting}
            extraction={actionExtraction}
            onItemsUpdated={(items) =>
              setMeeting((prev) => (prev ? { ...prev, action_items: items } : prev))
            }
          />
        )}
        {activeTab === "bookmarks" && (
          <BookmarksTab
            meeting={meeting}
            onBookmarkUpdated={(bookmarks) =>
              setMeeting((prev) => (prev ? { ...prev, bookmarks } : prev))
            }
            onNavigateToBookmark={(bookmark) => {
              // Find the segment index matching this bookmark
              const idx = meeting.transcript.findIndex(
                (s) => (bookmark.segment_id && s.id === bookmark.segment_id)
                  || s.timestamp_ms === bookmark.timestamp_ms
              );
              setScrollToSegmentIndex(idx >= 0 ? idx : null);
              setActiveTab("transcript");
            }}
            suggestions={bookmarkSuggestions}
          />
        )}
      </div>

      {/* Audio Player — sticky bottom bar */}
      {recordingProcessing && <AudioPlayerSkeleton />}
      {recordingInfo && !recordingProcessing && (
        <AudioPlayer
          meetingId={meeting.id}
          meetingStartMs={new Date(meeting.start_time).getTime()}
          recordingPath={recordingInfo.path}
          recordingSize={recordingInfo.size_bytes}
          recordingOffsetMs={recordingInfo.offset_ms}
          durationMs={recordingInfo.duration_ms}
          waveformData={recordingInfo.waveform_data ?? null}
          bookmarks={meeting.bookmarks}
          topicSections={meeting.topic_sections}
        />
      )}
    </div>
  );
}
