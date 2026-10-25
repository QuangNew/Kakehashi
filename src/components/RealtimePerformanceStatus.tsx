import { useMeetingStore } from "../stores/meetingStore";
import { useTranslationStore } from "../stores/translationStore";
import { useConfigStore } from "../stores/configStore";
import { useRealtimePerformance, type SttPerformance } from "../hooks/useRealtimePerformance";

export function RealtimePerformanceStatus() {
  const active = useMeetingStore(s => s.isRecording);
  const meetingId = useMeetingStore(s => s.activeMeeting?.id ?? null);
  const audioMode = useMeetingStore(s => s.audioMode);
  const provider = useTranslationStore(s => s.provider);
  const audioConfig = useConfigStore(s => s.meetingAudioConfig);
  const pending = useTranslationStore(s => s.translating.size);
  const performance = useRealtimePerformance(meetingId, active);
  const seconds = (ms: number) => `${(ms / 1000).toFixed(1)}s`;
  const stt = (label: string, metrics?: SttPerformance) => metrics?.backend && <span
    className={metrics.queued_audio_ms > 3000 || metrics.lag_ms > 4000 ? "text-amber-500" : ""}
    title={`${metrics.device ?? metrics.backend}; từ cuối đoạn audio; suy luận ${seconds(metrics.inference_ms)}, chờ ${seconds(metrics.queued_audio_ms)} audio`}
  >{label} · Whisper {metrics.backend === "Vulkan" ? "GPU" : "CPU"}{metrics.inference_ms > 0 ? ` ${seconds(metrics.lag_ms)}` : ""}</span>;
  const dropped = (performance.mic?.dropped_audio_ms ?? 0) + (performance.system?.dropped_audio_ms ?? 0);
  const error = performance.mic?.error || performance.system?.error;
  return <span className="flex flex-wrap items-center gap-2 text-meta text-muted-foreground">
    <span>Dịch: {provider}</span>
    {active && audioConfig?.you.stt_provider === "whisper_cpp" && stt("Bạn", performance.mic)}
    {active && audioConfig?.them.stt_provider === "whisper_cpp" && stt(audioMode === "in_person" ? "Phòng" : "Khách", performance.system)}
    {active && performance.translation && <span title={`Dịch ${seconds(performance.translation.inference_ms)}; chờ ${seconds(performance.translation.queue_ms)}`}>
      Dịch {seconds(performance.translation.queue_ms + performance.translation.inference_ms)}
    </span>}
    {active && pending > 0 && <span>{pending} đoạn đang dịch</span>}
    {active && dropped > 0 && <span role="status" className="text-red-500">Quá tải · bỏ {seconds(dropped)} audio</span>}
    {active && error && <span role="status" title={error} className="text-red-500">Lỗi Whisper</span>}
  </span>;
}
