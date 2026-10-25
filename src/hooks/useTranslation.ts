import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { onTranslationResult, onTranslationError, onBatchTranslationProgress, onTranscriptFinal } from "../lib/events";
import { getAllMeetingTranslations } from "../lib/ipc";
import { routeTranslation } from "../lib/translationRouting";
import { useTranslationStore } from "../stores/translationStore";
import { useMeetingStore } from "../stores/meetingStore";
import { useTranscriptStore } from "../stores/transcriptStore";
import type { TranscriptSegment, TranslationResult } from "../lib/types";

export function useTranslation(liveOwner = true) {
  const meetingId = useMeetingStore((s) => s.activeMeeting?.id ?? null);
  const active = useTranslationStore((s) => s.autoTranslateActive);
  const bidirectional = useTranslationStore((s) => s.bidirectional);
  const direction = useTranslationStore((s) => s.direction);
  const source = useTranslationStore((s) => s.sourceLang);
  const target = useTranslationStore((s) => s.targetLang);
  const provider = useTranslationStore((s) => s.provider);

  useEffect(() => {
    let disposed = false;
    const listeners = [
      onTranslationResult((result) => {
        if (disposed) return;
        if (result.request_id) return; // Live replies are revision-checked by their owner below.
        const current = useMeetingStore.getState().activeMeeting?.id;
        if (result.meeting_id && result.meeting_id !== current) return;
        if (result.segment_id) {
          const segment = useTranscriptStore.getState().segments.find((s) => s.id === result.segment_id);
          if (!segment || segment.text !== result.original_text) return;
        }
        useTranslationStore.getState().addTranslation(result);
      }),
      onTranslationError((error) => {
        if (disposed) return;
        useTranslationStore.getState().setLastError(error.error);
        if (error.segment_id) useTranslationStore.getState().setTranslating(error.segment_id, false);
      }),
      onBatchTranslationProgress((progress) => {
        if (!disposed) useTranslationStore.getState().setBatchProgress(progress.completed >= progress.total ? null : progress);
      }),
    ];
    return () => { disposed = true; for (const listener of listeners) listener.then((unlisten) => unlisten()).catch(console.error); };
  }, []);

  useEffect(() => {
    if (liveOwner) useTranslationStore.getState().setAutoTranslateActive(!!meetingId && useTranslationStore.getState().autoTranslateEnabled);
  }, [meetingId, liveOwner]);

  useEffect(() => {
    let disposed = false;
    useTranslationStore.getState().clearTranslations();
    if (!meetingId) return;
    getAllMeetingTranslations(meetingId).then((results) => {
      if (!disposed) useTranslationStore.getState().addTranslations(results);
    }).catch((error) => console.warn("Cannot load meeting translations", error));
    return () => { disposed = true; };
  }, [meetingId]);

  useEffect(() => {
    if (!meetingId || !active || !liveOwner) return;
    let disposed = false;
    let sessionId: string | null = null;
    const timers = new Map<string, ReturnType<typeof setTimeout>>();
    const revisions = new Map<string, string>();
    const store = useTranslationStore.getState;
    store().setLastError(null);
    const session = invoke<string>("begin_live_translation", { meetingId }).then((id) => {
      sessionId = id;
      if (disposed) void invoke("end_live_translation", { sessionId: id }).catch(console.error);
      return id;
    });
    // Observe this rejection immediately, even when no speech arrives.
    session.catch((error) => { if (!disposed) store().setLastError(String(error)); });

    const queue = (segment: TranscriptSegment) => {
      if (disposed || !segment.is_final || !segment.text.trim()) return;
      const route = routeTranslation(segment, bidirectional, direction, source, target);
      if (!route) return;
      const key = `${segment.id}:${route.target}`;
      if (!timers.has(key) && timers.size >= 128) { store().setLastError("Translation queue is full"); return; }
      const previous = timers.get(key);
      if (previous) clearTimeout(previous);
      const requestId = crypto.randomUUID();
      revisions.set(key, requestId);
      store().setTranslating(segment.id, true);
      timers.set(key, setTimeout(async () => {
        timers.delete(key);
        try {
          const id = await session;
          if (disposed || revisions.get(key) !== requestId) return;
          const result = await invoke<TranslationResult | null>("translate_live_segment", {
            meetingId, sessionId: id, segmentId: segment.id, text: segment.text,
            sourceLang: route.source, targetLang: route.target, requestId,
          });
          if (disposed || revisions.get(key) !== requestId) return;
          if (result) {
            store().addTranslation(result);
            store().setLastError(null);
          }
        } catch (error) {
          if (!disposed && revisions.get(key) === requestId) store().setLastError(String(error));
        } finally {
          if (!disposed && revisions.get(key) === requestId) {
            revisions.delete(key);
            store().setTranslating(segment.id, false);
          }
        }
      }, 200));
    };
    const listener = onTranscriptFinal(({ segment }) => queue(segment));
    return () => {
      disposed = true;
      for (const timer of timers.values()) clearTimeout(timer);
      for (const key of revisions.keys()) store().setTranslating(key.slice(0, key.lastIndexOf(":")), false);
      listener.then((unlisten) => unlisten()).catch(console.error);
      if (sessionId) void invoke("end_live_translation", { sessionId }).catch(console.error);
    };
  }, [meetingId, active, bidirectional, direction, source, target, provider, liveOwner]);
}
