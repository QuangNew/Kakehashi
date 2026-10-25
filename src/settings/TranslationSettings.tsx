import { useEffect, useState } from "react";
import { useTranslationStore } from "../stores/translationStore";
import { useConfigStore } from "../stores/configStore";
import { getApiKey, storeApiKey, listTranslationModels, testTranslationConnection } from "../lib/ipc";
import type { TranslationProviderType } from "../lib/types";
import { OpusMtModelManager } from "./OpusMtModelManager";

export function TranslationSettings() {
  const state = useTranslationStore();
  const config = useConfigStore();
  const [provider, setProvider] = useState<TranslationProviderType>(state.provider);
  const [model, setModel] = useState(state.provider === "hy-mt" ? "" : state.apiModels[state.provider] ?? "");
  const [key, setKey] = useState("");
  const [models, setModels] = useState<Array<{ id: string; name: string }>>([]);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const [region, setRegion] = useState("global");
  const usesModel = provider === "groq" || provider === "gemini";
  const usesKey = usesModel || ["microsoft", "google", "deepl"].includes(provider);

  useEffect(() => {
    let cancelled = false;
    setKey(""); setModels([]); setMessage(""); setError("");
    setModel(provider === "hy-mt" ? "" : useTranslationStore.getState().apiModels[provider] ?? "");
    if (usesKey) {
      getApiKey(`translation_${provider}`).then(value => { if (!cancelled) setKey(value ?? ""); })
        .catch(reason => { if (!cancelled) setError(String(reason)); });
    }
    if (provider === "microsoft") getApiKey("translation_microsoft_region").then(value => { if (!cancelled) setRegion(value || "global"); }).catch(() => {});
    return () => { cancelled = true; };
  }, [provider]);

  async function run(action: () => Promise<void>) {
    setBusy(true); setError(""); setMessage("");
    try { await action(); } catch (reason) { setError(String(reason)); }
    finally { setBusy(false); }
  }
  async function saveKey() {
    if (usesKey) {
      if (!key.trim()) throw new Error("Nhập API key cho AI dịch.");
      await storeApiKey(`translation_${provider}`, key.trim());
      if (provider === "microsoft") await storeApiKey("translation_microsoft_region", region.trim() || "global");
    }
  }

  return <div className="space-y-6">
    <div>
      <h2 className="text-lg font-semibold text-foreground">AI dịch — Nhật ↔ Việt</h2>
      <p className="mt-1 text-sm text-muted-foreground">Provider, model và key của AI dịch được chọn riêng với AI copilot.</p>
    </div>
    <fieldset disabled={busy} className="space-y-4 disabled:opacity-60">
      <label className="block text-sm">Nhà cung cấp
        <select aria-label="Nhà cung cấp AI dịch" value={provider} onChange={e => setProvider(e.target.value as TranslationProviderType)} className="mt-1 w-full rounded border border-border bg-background p-2">
          <option value="hy-mt">Hy-MT2 local — model trong folder</option>
          <option value="groq">Groq API</option>
          <option value="gemini">Google AI Studio — Gemini API</option>
          <option value="microsoft">Microsoft Translator</option>
          <option value="google">Google Cloud Translation</option>
          <option value="deepl">DeepL</option>
          <option value="opus-mt">OPUS-MT local</option>
          <option value="llm">LLM dùng chung provider/model copilot</option>
        </select>
      </label>
      {usesKey ? <>
        <p className="text-sm text-muted-foreground">Nội dung cần dịch được gửi đến nhà cung cấp đã chọn. Key này chỉ dành cho AI dịch.</p>
        <label className="block text-sm">API key
          <input aria-label="API key AI dịch" type="password" value={key} onChange={e => setKey(e.target.value)} autoComplete="off" className="mt-1 w-full rounded border border-border bg-background p-2" />
        </label>
        {provider === "microsoft" && <label className="block text-sm">Region<input aria-label="Region Microsoft Translator" value={region} onChange={e => setRegion(e.target.value)} className="mt-1 w-full rounded border border-border bg-background p-2" /></label>}
        {usesModel && <>
        <button type="button" className="rounded-md border border-border px-3 py-2 text-sm hover:bg-accent" onClick={() => void run(async () => { await saveKey(); setModels(await listTranslationModels(provider)); setMessage("Đã nạp danh sách model. Chọn model rồi áp dụng."); })}>Lưu key và nạp model</button>
        <label className="block text-sm">Model
          <input aria-label="Model AI dịch" list="translation-api-models" value={model} onChange={e => setModel(e.target.value)} placeholder="Nhập ID hoặc chọn model từ danh sách" className="mt-1 w-full rounded border border-border bg-background p-2" />
          <datalist id="translation-api-models">{models.map(item => <option key={item.id} value={item.id}>{item.name}</option>)}</datalist>
        </label>
        </>}
      </> : provider === "hy-mt" ? <p className="text-sm text-muted-foreground">Chọn file Hy-MT2 và runtime tại Local Models. AI dịch chạy trên máy.</p> : provider === "opus-mt" ? <OpusMtModelManager /> : <p className="text-sm text-muted-foreground">Bản dịch dùng provider, model và key copilot đang chọn. Groq/Gemini ở trên có cấu hình riêng nếu bạn muốn hai AI độc lập.</p>}
      <div className="flex flex-wrap gap-3">
        <button type="button" className="rounded-md border border-border px-3 py-2 text-sm hover:bg-accent" onClick={() => void run(async () => {
          await saveKey(); const result = await testTranslationConnection(provider, usesModel ? model.trim() || undefined : undefined, provider === "microsoft" ? region : undefined);
          if (!result.connected) throw new Error(result.error ?? "Không kết nối được.");
          setMessage("Kết nối thành công.");
        })}>Kiểm tra kết nối</button>
        <button type="button" className="rounded-md bg-primary px-3 py-2 text-sm text-primary-foreground" onClick={() => void run(async () => {
          await saveKey(); await state.activateProvider(provider, usesModel ? model.trim() || undefined : undefined, provider === "microsoft" ? region : undefined); setMessage("Đã áp dụng cấu hình AI dịch.");
        })}>Áp dụng AI dịch</button>
      </div>
    </fieldset>
    {busy && <p role="status" className="text-sm">Đang xử lý…</p>}
    {message && <p role="status" className="text-sm text-green-500">{message}</p>}
    {(error || state.lastError) && <p role="alert" className="text-sm text-red-400">{error || state.lastError}</p>}
    <p className="text-sm text-muted-foreground">Đang dùng: {state.provider}{state.provider !== "hy-mt" ? ` / ${state.apiModels[state.provider] ?? "chưa chọn model"}` : ""}</p>
    <div className="space-y-3 border-t border-border pt-4 text-sm">
      <label className="flex gap-2"><input type="checkbox" checked={state.bidirectional} onChange={e => state.setBidirectional(e.target.checked)} />Dịch hai chiều theo nguồn âm thanh</label>
      <label className="block">Ngôn ngữ của bạn
        <select value={state.direction} onChange={e => state.setDirection(e.target.value as "vi-ja" | "ja-vi")} className="ml-3 rounded bg-background p-2">
          <option value="vi-ja">Microphone: Việt → Nhật; system: Nhật → Việt</option>
          <option value="ja-vi">Microphone: Nhật → Việt; system: Việt → Nhật</option>
        </select>
      </label>
      {!state.bidirectional && <label className="block">Ngôn ngữ đích <select value={state.targetLang} onChange={e => state.setTargetLang(e.target.value)} className="ml-3 rounded bg-background p-2"><option value="vi">Tiếng Việt</option><option value="ja">日本語</option><option value="en">English</option></select></label>}
      <label className="block">Hiển thị <select value={state.displayMode} onChange={e => state.setDisplayMode(e.target.value as "inline" | "hover")} className="ml-3 rounded bg-background p-2"><option value="inline">Phụ đề song ngữ</option><option value="hover">Bản dịch khi rê chuột</option></select></label>
      <label className="flex gap-2"><input type="checkbox" checked={state.autoTranslateEnabled} onChange={e => state.setAutoTranslateEnabled(e.target.checked)} />Tự dịch câu đã chốt</label>
      <label className="flex gap-2"><input type="checkbox" checked={state.selectionToolbarEnabled} onChange={e => state.setSelectionToolbarEnabled(e.target.checked)} />Dịch đoạn văn được chọn</label>
      <label className="flex gap-2"><input type="checkbox" checked={config.showPostMeetingTranslation} onChange={e => config.setShowPostMeetingTranslation(e.target.checked)} />Hiển thị bản dịch trong lịch sử cuộc họp</label>
    </div>
  </div>;
}
