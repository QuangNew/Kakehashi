import { useEffect, useState } from "react";
import { API_PROVIDERS, deleteApiPoolEntry, saveApiPoolEntry, type ApiPoolEntry } from "../lib/apiPool";
import { useApiPoolStore } from "../stores/apiPoolStore";
import { invoke } from "@tauri-apps/api/core";

const inputClass = "w-full rounded-lg border border-border/50 bg-background px-3 py-2 text-sm text-foreground";
export function ApiPoolSettings({initialProvider}:{initialProvider?:string | null}) {
  const snapshot=useApiPoolStore(s=>s.snapshot);
  const loadError=useApiPoolStore(s=>s.error);
  const [provider,setProvider]=useState(initialProvider ?? "groq");
  const [id,setId]=useState<string>();
  const [label,setLabel]=useState("");
  const [key,setKey]=useState("");
  const [region,setRegion]=useState("");
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState("");
  const [message,setMessage]=useState("");
  const [deleting,setDeleting]=useState<string | null>(null);
  useEffect(()=>{void useApiPoolStore.getState().refresh();},[]);
  useEffect(()=>{if(initialProvider)setProvider(initialProvider);},[initialProvider]);
  const reset=()=>{setId(undefined);setLabel("");setKey("");setRegion("");};
  const edit=(entry:ApiPoolEntry)=>{setProvider(entry.provider);setId(entry.id);setLabel(entry.label);setRegion(entry.region ?? "");setKey("");setError("");};
  async function run(action:()=>Promise<void>) {
    setBusy(true);setError("");setMessage("");
    try{await action();await useApiPoolStore.getState().refresh();}catch(error){setError(String(error));}finally{setBusy(false);}
  }
  return <section className="space-y-5" aria-label="API Pool">
    <header><h2 className="text-lg font-semibold">API Pool</h2><p className="mt-1 text-sm text-muted-foreground">Thêm key một lần, rồi chọn cho STT, AI dịch và copilot. Có thể lưu nhiều tài khoản cùng nhà cung cấp.</p></header>
    {(error || loadError) && <p role="alert" className="break-words rounded-lg bg-destructive/10 p-3 text-sm text-destructive">{error || loadError}</p>}
    {message && <p role="status" className="text-sm text-success">{message}</p>}
    <form className="space-y-3 rounded-xl border border-border/40 p-4" onSubmit={event=>{
      event.preventDefault();void run(async()=>{
        await saveApiPoolEntry({id,provider,label:label.trim() || API_PROVIDERS.find(p=>p[0]===provider)?.[1] || provider,key:key.trim() || undefined,region:region.trim() || undefined});
        reset();setMessage("Đã lưu key trong Windows Credential Manager. Có thể chọn ở các phần cấu hình AI.");
      });
    }}>
      <h3 className="text-sm font-semibold">{id ? "Sửa thông tin key" : "Thêm API key"}</h3>
      <label className="block space-y-1 text-xs text-muted-foreground">Nhà cung cấp<select aria-label="API Pool provider" disabled={busy || !!id} value={provider} onChange={e=>setProvider(e.target.value)} className={inputClass}>{API_PROVIDERS.map(([value,name])=><option key={value} value={value}>{name}</option>)}</select></label>
      <label className="block space-y-1 text-xs text-muted-foreground">Tên gợi nhớ<input aria-label="API Pool label" value={label} maxLength={80} onChange={e=>setLabel(e.target.value)} placeholder="Ví dụ: Cá nhân, Công việc" className={inputClass} /></label>
      <label className="block space-y-1 text-xs text-muted-foreground">API key<input aria-label="API Pool secret" type="password" autoComplete="off" value={key} maxLength={4096} onChange={e=>setKey(e.target.value)} placeholder={id ? "Để trống để giữ nguyên key đã lưu" : "Nhập key của nhà cung cấp"} required={!id} className={inputClass} /></label>
      {["azure_speech","microsoft"].includes(provider) && <label className="block space-y-1 text-xs text-muted-foreground">Region<input aria-label="API Pool region" value={region} onChange={e=>setRegion(e.target.value)} placeholder={provider==="azure_speech" ? "eastus" : "global"} className={inputClass}/></label>}
      <div className="flex gap-2"><button type="submit" disabled={busy} className="rounded-lg bg-primary px-4 py-2 text-sm text-primary-foreground disabled:opacity-50">{busy ? "Đang lưu…" : "Lưu key"}</button>{id && <button type="button" onClick={reset} className="rounded-lg border border-border px-4 py-2 text-sm">Hủy chỉnh sửa</button>}</div>
      <p className="text-xs text-muted-foreground">Không lưu key trong file cấu hình hay Git. Màu xanh nghĩa là có key đã lưu; quyền truy cập model và quota phụ thuộc tài khoản.</p>
    </form>
    <div className="space-y-3">
      {snapshot.entries.length===0 && <p className="text-sm text-muted-foreground">Chưa có key. Các API key cũ sẽ được nhận lại tự động khi ứng dụng khởi động.</p>}
      {snapshot.entries.map(entry=><article key={entry.id} className="space-y-2 rounded-xl border border-border/40 p-3">
        <div className="flex flex-wrap items-center justify-between gap-2"><div><p className="text-sm font-medium">{entry.label}</p><p className="text-xs text-muted-foreground">{API_PROVIDERS.find(p=>p[0]===entry.provider)?.[1] ?? entry.provider}{entry.region ? ` · ${entry.region}` : ""}</p></div><span className={`inline-flex items-center gap-1.5 text-xs ${entry.key_present ? "text-success" : "text-destructive"}`}><span aria-hidden="true" className={`h-2 w-2 rounded-full ${entry.key_present ? "bg-success" : "bg-destructive"}`} />{entry.key_present ? "Đã lưu · có thể chọn" : "Không tìm thấy key"}</span></div>
        <div className="flex flex-wrap gap-2"><button type="button" disabled={busy} onClick={()=>edit(entry)} className="rounded border border-border px-3 py-1 text-xs">Chỉnh sửa</button>
          {["groq","gemini","openai","anthropic","openrouter"].includes(entry.provider) && <button type="button" disabled={busy} onClick={()=>void run(async()=>{const count=await invoke<number>("test_api_pool_entry",{id:entry.id});setMessage(`${entry.label}: truy cập được ${count} model. Không đổi provider đang dùng.`);})} className="rounded border border-border px-3 py-1 text-xs">Kiểm tra kết nối</button>}
          <button type="button" disabled={busy} onClick={()=>setDeleting(entry.id)} className="rounded border border-destructive/30 px-3 py-1 text-xs text-destructive">Xóa key</button>
        </div>
        {deleting===entry.id && <div className="flex flex-wrap items-center gap-2 rounded bg-destructive/10 p-2 text-xs"><span>Xóa key “{entry.label}” khỏi API Pool?</span><button type="button" disabled={busy} onClick={()=>void run(async()=>{await deleteApiPoolEntry(entry.id);setDeleting(null);if(id===entry.id)reset();setMessage("Đã xóa key. Các vai trò dùng key này cần chọn lại.");})} className="rounded bg-destructive px-2 py-1 text-white">Xác nhận xóa</button><button type="button" onClick={()=>setDeleting(null)}>Giữ lại</button></div>}
      </article>)}
    </div>
  </section>;
}
