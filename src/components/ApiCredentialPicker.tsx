import { useState } from "react";
import { bindApiPoolEntry, poolProvider, selectedCredential, type ApiUsage } from "../lib/apiPool";
import { useApiPoolStore } from "../stores/apiPoolStore";

export function ApiCredentialPicker({usage,provider}:{usage:ApiUsage;provider:string}) {
  const snapshot = useApiPoolStore(s=>s.snapshot);
  const loaded = useApiPoolStore(s=>s.loaded);
  const [error,setError] = useState("");
  const [busy,setBusy] = useState(false);
  const canonical = poolProvider(provider);
  const selected = selectedCredential(snapshot,usage,provider);
  const entries = snapshot.entries.filter(entry=>entry.provider===canonical && entry.key_present);
  return <div className="space-y-2 rounded-lg border border-border/40 p-3">
    <div className="flex flex-wrap items-center justify-between gap-2">
      <span className={`inline-flex items-center gap-2 text-xs ${selected ? "text-success" : "text-destructive"}`}>
        <span aria-hidden="true" className={`h-2 w-2 rounded-full ${selected ? "bg-success" : "bg-destructive"}`} />
        {!loaded ? "Đang đọc API Pool…" : selected ? `Đã lưu · ${selected.label}` : "Chưa chọn key trong API Pool"}
      </span>
      <button type="button" onClick={()=>useApiPoolStore.getState().open(provider)} className="rounded border border-border px-2 py-1 text-xs hover:bg-accent">Mở API Pool</button>
    </div>
    {entries.length > 0 && <label className="block text-xs text-muted-foreground">Key dùng cho {usage === "translation" ? "AI dịch" : usage === "copilot" ? "copilot" : "STT"}
      <select aria-label={`API Pool key ${usage}`} disabled={busy} value={snapshot.bindings[`${usage}.${canonical}`] ?? ""}
        className="mt-1 w-full rounded border border-border bg-background p-2 text-foreground"
        onChange={async event=>{
          setBusy(true);setError("");
          try { await bindApiPoolEntry(usage,provider,event.target.value || null);await useApiPoolStore.getState().refresh(); }
          catch(error){setError(String(error));}finally{setBusy(false);}
        }}>
        <option value="">{snapshot.bindings[`${usage}.${canonical}`] === "" ? "Key cũ đã xóa — hãy chọn key khác" : "Dùng key mặc định của nhà cung cấp"}</option>
        {entries.map(entry=><option key={entry.id} value={entry.id}>{entry.label}</option>)}
      </select>
    </label>}
    {error && <p role="alert" className="text-xs text-destructive">{error}</p>}
    <p className="text-xs text-muted-foreground">Key được giữ trong Windows Credential Manager. Trạng thái đã lưu không xác nhận quota hay quyền truy cập model.</p>
  </div>;
}
