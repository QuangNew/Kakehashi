import { useConfigStore } from "../stores/configStore";
import { useMeetingStore } from "../stores/meetingStore";
import { preparing, usePreparationStore } from "../stores/preparationStore";

export function ModelPreparationControls(){
  const enabled=useConfigStore(s=>s.preloadLocalModels);
  const setEnabled=useConfigStore(s=>s.setPreloadLocalModels);
  const active=useMeetingStore(s=>s.activeMeeting != null);
  const {status,prepare,release}=usePreparationStore();
  const busy=preparing(status.stage);
  return <section className="space-y-3 rounded-xl border border-border/40 p-4">
    <h3 className="text-sm font-semibold">Chuẩn bị model trước cuộc họp</h3>
    <label className="flex items-center gap-2 text-sm"><input type="checkbox" checked={enabled} onChange={e=>setEnabled(e.target.checked)}/>Tự chuẩn bị model local khi mở ứng dụng</label>
    <p className="text-xs text-muted-foreground">Giữ trọng số Whisper và runtime dịch trong RAM/VRAM để vào cuộc họp tiếp theo nhanh hơn. Không thu âm và không gọi API cloud khi chuẩn bị.</p>
    <div className="flex flex-wrap gap-2"><button disabled={active || busy} onClick={()=>void prepare()} className="rounded-lg border border-border px-3 py-2 text-sm disabled:opacity-50">{busy ? "Đang chuẩn bị…" : "Chuẩn bị ngay"}</button><button disabled={active || busy} onClick={()=>void release()} className="rounded-lg border border-border px-3 py-2 text-sm disabled:opacity-50">Giải phóng STT / dịch</button></div>
    <p role="status" className="text-xs text-muted-foreground">{status.stage === "ready" ? `Đã chuẩn bị trong ${(status.elapsed_ms/1000).toFixed(1)}s · dịch ${(status.translation_ms/1000).toFixed(1)}s · STT ${(status.whisper_ms/1000).toFixed(1)}s` : status.stage === "translation" ? "Đang nạp và làm nóng model dịch…" : status.stage === "whisper" ? "Đang nạp Whisper và chuẩn bị GPU…" : status.stage === "starting" ? "Đang bắt đầu…" : "Whisper đã giữ trong bộ nhớ: " + status.cached_whisper_models}</p>
    {status.error && <p role="alert" className="text-sm text-destructive">{status.error}</p>}
  </section>;
}
