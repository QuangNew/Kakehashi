import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

export interface SttPerformance {
  backend: string;
  device: string | null;
  audio_ms: number;
  inference_ms: number;
  queue_ms: number;
  lag_ms: number;
  queued_audio_ms: number;
  dropped_audio_ms: number;
  processed_audio_ms: number;
  total_inference_ms: number;
  error: string | null;
}
export interface TranslationPerformance {
  meeting_id: string;
  queue_ms: number;
  inference_ms: number;
}
interface SttEvent {
  meeting_id: string;
  source: "Mic" | "System";
  metrics: SttPerformance;
}
interface PerformanceState {
  mic?: SttPerformance;
  system?: SttPerformance;
  translation?: TranslationPerformance;
}

export function useRealtimePerformance(meetingId: string | null, active: boolean) {
  const [state, setState] = useState<PerformanceState>({});
  useEffect(() => {
    setState({});
    if (!meetingId || !active) return;
    let disposed = false;
    const listeners = [
      listen<SttEvent>("stt_performance", ({ payload }) => {
        if (!disposed && payload.meeting_id === meetingId) {
          setState(previous => ({ ...previous, [payload.source === "Mic" ? "mic" : "system"]: payload.metrics }));
        }
      }),
      listen<TranslationPerformance>("translation_performance", ({ payload }) => {
        if (!disposed && payload.meeting_id === meetingId) {
          setState(previous => ({ ...previous, translation:payload }));
        }
      }),
    ];
    return () => {
      disposed = true;
      for (const listener of listeners) listener.then(unlisten => unlisten()).catch(console.error);
    };
  }, [meetingId, active]);
  return state;
}
