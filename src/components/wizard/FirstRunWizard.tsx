import { useState } from "react";
import { useConfigStore } from "../../stores/configStore";
import { MeetingAudioSettings } from "../../settings/MeetingAudioSettings";
import { LocalModelsSettings } from "../../settings/LocalModelsSettings";
import { LLMSettings } from "../../settings/LLMSettings";
import { TranslationSettings } from "../../settings/TranslationSettings";
const STEPS = ["Chào mừng", "Model local", "Âm thanh", "AI dịch", "Copilot"];
export function FirstRunWizard() {
  const [step,setStep] = useState(0);
  const complete = useConfigStore(s => s.setFirstRunCompleted);
  return <div className="flex h-full flex-col bg-background text-foreground">
    <header className="border-b border-border p-5"><h1 className="text-xl font-semibold">Kakehashi</h1><p className="text-sm text-muted-foreground">{step+1} / {STEPS.length} · {STEPS[step]}</p></header>
    <main className="flex-1 overflow-auto p-6">
      {step === 0 && <div className="space-y-4"><h2 className="text-lg font-semibold">Phiên dịch và trợ lý cuộc họp Việt–Nhật</h2><p>Whisper nhận dạng hai nguồn âm thanh trên máy. Chọn Hy-MT2 local hoặc API Groq/Google AI Studio để dịch; copilot có provider và model riêng.</p><p className="text-sm text-muted-foreground">Chuẩn bị model trong folder, kiểm tra microphone và system audio, sau đó chọn hai AI. Bạn có thể quay lại chỉnh trong Settings.</p></div>}
      {step === 1 && <LocalModelsSettings />}{step === 2 && <MeetingAudioSettings />}{step === 3 && <TranslationSettings />}{step === 4 && <LLMSettings />}
    </main>
    <footer className="flex justify-between border-t border-border p-4"><button className="rounded px-4 py-2 text-sm hover:bg-accent" onClick={() => step ? setStep(step-1) : complete(true)}>{step ? "Quay lại" : "Thiết lập sau"}</button><button className="rounded bg-primary px-4 py-2 text-sm text-primary-foreground" onClick={() => step < STEPS.length-1 ? setStep(step+1) : complete(true)}>{step < STEPS.length-1 ? "Tiếp tục" : "Hoàn tất"}</button></footer>
  </div>;
}
