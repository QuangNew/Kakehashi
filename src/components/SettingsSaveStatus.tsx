import { useState } from "react";
import { flushSettings, useSettingsSaveState } from "../lib/persistedSettings";

export function SettingsSaveStatus() {
  const state=useSettingsSaveState();const [error,setError]=useState("");
  return <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-t border-border/30 px-4 py-2 text-xs">
    <span role={state.error || error ? "alert" : "status"} className={state.error || error ? "text-destructive" : "text-muted-foreground"}>{error || state.error || (state.saving ? "Đang lưu thiết lập…" : state.changed ? "Đã lưu thiết lập cho lần sau" : "Thiết lập được lưu trên máy")}</span>
    <button type="button" className="rounded border border-border px-2 py-1 hover:bg-accent" onClick={()=>{setError("");void flushSettings().catch(e=>setError(String(e)));}}>Lưu ngay</button>
  </div>;
}
