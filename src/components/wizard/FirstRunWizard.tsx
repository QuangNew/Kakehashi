import { useState, useEffect, useCallback } from "react";
import { useConfigStore } from "../../stores/configStore";
import { MeetingAudioSettings } from "../../settings/MeetingAudioSettings";
import { LocalModelsSettings } from "../../settings/LocalModelsSettings";
import { LLMSettings } from "../../settings/LLMSettings";
import { TranslationSettings } from "../../settings/TranslationSettings";
import { useMeetingStore } from "../../stores/meetingStore";
import { flushSettings } from "../../lib/persistedSettings";
import { SettingsSaveStatus } from "../SettingsSaveStatus";
import { showToast } from "../../stores/toastStore";
const STEPS = ["Chào mừng", "Model local", "Âm thanh", "AI dịch", "Copilot"];
export function FirstRunWizard() {
  const [step,setStep] = useState(0);
  const complete = useConfigStore(s => s.setFirstRunCompleted);
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState("");
  const close=useCallback(async()=>{
    setBusy(true);setError("");
    try {
      complete(true);await flushSettings();
    }catch(error){setError(String(error));showToast("Chưa lưu được thiết lập. Hãy thử Lưu ngay trong Settings.","error");}
    finally{
      const meetings=useMeetingStore.getState();
      meetings.setSettingsOpen(false);
      meetings.setCurrentView("launcher");
      setBusy(false);
    }
  },[complete]);
  useEffect(()=>{const key=(event:KeyboardEvent)=>{if(event.key==="Escape" && !event.defaultPrevented){event.preventDefault();void close();}};window.addEventListener("keydown",key);return()=>window.removeEventListener("keydown",key);},[close]);
  return <div className="flex h-full min-h-0 flex-col bg-background text-foreground">
    <header className="relative shrink-0 border-b border-border p-5"><button type="button" aria-label="Đóng thiết lập nhanh" disabled={busy} onClick={()=>void close()} className="absolute right-4 top-4 rounded border border-border px-3 py-1.5 text-sm">Đóng</button><h1 className="text-xl font-semibold">Kakehashi</h1><p className="text-sm text-muted-foreground">{step+1} / {STEPS.length} · {STEPS[step]}</p></header>
    <main className="min-h-0 flex-1 overflow-y-auto overscroll-contain p-6">
      {step === 0 && <div className="space-y-4"><h2 className="text-lg font-semibold">Phiên dịch và trợ lý cuộc họp Việt–Nhật</h2><p>Whisper nhận dạng hai nguồn âm thanh trên máy. Chọn Hy-MT2 local hoặc API Groq/Google AI Studio để dịch; copilot có provider và model riêng.</p><p className="text-sm text-muted-foreground">Chuẩn bị model trong folder, kiểm tra microphone và system audio, sau đó chọn hai AI. Bạn có thể quay lại chỉnh trong Settings.</p></div>}
      {step === 1 && <LocalModelsSettings />}{step === 2 && <MeetingAudioSettings />}{step === 3 && <TranslationSettings />}{step === 4 && <LLMSettings />}
    </main>
    {error && <p role="alert" className="px-4 text-sm text-destructive">{error}</p>}<SettingsSaveStatus /><footer className="flex shrink-0 justify-between border-t border-border p-4"><button className="rounded px-4 py-2 text-sm hover:bg-accent" disabled={busy} onClick={() => step ? setStep(step-1) : void close()}>{step ? "Quay lại" : "Thiết lập sau"}</button><button className="rounded bg-primary px-4 py-2 text-sm text-primary-foreground" disabled={busy} onClick={() => step < STEPS.length-1 ? setStep(step+1) : void close()}>{step < STEPS.length-1 ? "Tiếp tục" : "Hoàn tất"}</button></footer>
  </div>;
}
