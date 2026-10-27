import { useEffect } from "react";
import { useApiPoolStore } from "../stores/apiPoolStore";
import { API_PROVIDERS } from "../lib/apiPool";
import { ApiPoolSettings } from "../settings/ApiPoolSettings";

export function ApiPoolDialogs() {
  const open=useApiPoolStore(s=>s.dialogOpen);
  const requested=useApiPoolStore(s=>s.requestedProvider);
  const missing=useApiPoolStore(s=>s.missingProvider);
  useEffect(()=>{
    if(!open && !missing)return;
    const escape=(event:KeyboardEvent)=>{if(event.key==="Escape"){event.stopImmediatePropagation();event.preventDefault();useApiPoolStore.getState().close();useApiPoolStore.getState().dismissMissing();}};
    window.addEventListener("keydown",escape,true);return()=>window.removeEventListener("keydown",escape,true);
  },[open,missing]);
  if(missing) return <div className="fixed inset-0 z-[80] flex items-center justify-center bg-black/60 p-4" role="dialog" aria-modal="true" aria-label="Thiếu API key">
    <div className="w-full max-w-md space-y-4 rounded-xl border border-border bg-card p-5"><h2 className="font-semibold">Chưa có key cho {API_PROVIDERS.find(p=>p[0]===missing)?.[1] ?? missing}</h2><p className="text-sm text-muted-foreground">Bạn muốn mở API Pool để thêm hoặc chọn key không?</p><div className="flex flex-wrap gap-2"><button onClick={()=>useApiPoolStore.getState().open(missing)} className="rounded-lg bg-primary px-3 py-2 text-sm text-primary-foreground">Mở API Pool</button><button onClick={()=>useApiPoolStore.getState().dismissMissing()} className="rounded-lg border border-border px-3 py-2 text-sm">Để sau</button></div></div>
  </div>;
  if(!open)return null;
  return <div className="fixed inset-0 z-[80] flex items-center justify-center bg-black/60 p-3" role="dialog" aria-modal="true" aria-label="API Pool" onClick={event=>{if(event.target===event.currentTarget)useApiPoolStore.getState().close();}}>
    <div className="flex max-h-[calc(100dvh-1.5rem)] w-full max-w-2xl flex-col overflow-hidden rounded-xl border border-border bg-card"><header className="flex shrink-0 items-center justify-between border-b border-border px-4 py-3"><span className="text-sm font-semibold">API Pool</span><button aria-label="Đóng API Pool" onClick={()=>useApiPoolStore.getState().close()} className="rounded px-3 py-1 hover:bg-accent">Đóng</button></header><div className="min-h-0 overflow-y-auto overscroll-contain p-4"><ApiPoolSettings initialProvider={requested} /></div></div>
  </div>;
}
