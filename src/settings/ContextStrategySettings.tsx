import { useEffect, useState } from "react";
import { useConfigStore } from "../stores/configStore";
import { useRagStore } from "../stores/ragStore";
import { useRagEvents } from "../hooks/useRagEvents";

export function ContextStrategySettings() {
  const config = useConfigStore();
  const rag = useRagStore();
  const [query,setQuery] = useState("");
  useRagEvents();
  useEffect(() => { void rag.loadRagConfig(); void rag.refreshIndexStatus(); }, []);
  const settings = rag.ragConfig;
  return <div className="space-y-5">
    <h2 className="text-lg font-semibold">Tài liệu và ngữ cảnh</h2>
    <div className="space-y-3 text-sm">
      <label className="flex gap-2"><input type="radio" name="context-strategy" checked={config.contextStrategy === "stuffing"} onChange={() => { config.setContextStrategy("stuffing"); if (settings) void rag.saveRagConfig({ ...settings, enabled:false }); }} />Đưa tài liệu trực tiếp vào ngữ cảnh — phù hợp tài liệu ngắn</label>
      <label className="flex gap-2"><input type="radio" name="context-strategy" checked={config.contextStrategy === "local_rag"} onChange={() => { config.setContextStrategy("local_rag"); if (settings) void rag.saveRagConfig({ ...settings, enabled:true }); }} />Tìm đoạn liên quan bằng RAG local — phù hợp nhiều tài liệu</label>
    </div>
    <p className="text-sm text-muted-foreground">Embedding chạy bằng model trong folder. Copilot nhận phần tài liệu được chọn theo cấu hình từng AI action.</p>
    {settings && <div className="space-y-4 rounded-lg border border-border p-4">
      <p className="text-sm">Model: {settings.embedding_model} · {settings.embedding_dimensions} chiều. Đổi file model ở Local Models rồi dựng lại chỉ mục.</p>
      <div className="grid grid-cols-2 gap-4">
        {([["chunk_size","Kích thước đoạn",64,4096], ["chunk_overlap","Phần chồng lặp",0,512], ["top_k","Số đoạn đưa vào AI",1,20], ["similarity_threshold","Ngưỡng liên quan",0,1], ["semantic_weight","Trọng số ngữ nghĩa",0,1]] as const).map(([id,label,min,max]) => <label className="text-sm" key={id}>{label}<input className="mt-1 w-full rounded border border-border bg-background p-2" type="number" min={min} max={max} step={max === 1 ? 0.05 : 1} value={settings[id]} onChange={e => { const value = Number(e.target.value); if (Number.isFinite(value)) void rag.saveRagConfigWithStaleCheck({ ...settings, [id]: value },settings); }} /></label>)}
        <label className="text-sm">Cách tìm<select className="mt-1 w-full rounded border border-border bg-background p-2" value={settings.search_mode} onChange={e => void rag.saveRagConfig({ ...settings, search_mode:e.target.value })}><option value="hybrid">Từ khóa + ngữ nghĩa</option><option value="semantic">Ngữ nghĩa</option><option value="keyword">Từ khóa</option></select></label>
      </div>
      <label className="flex gap-2 text-sm"><input type="checkbox" checked={settings.include_transcript} onChange={e => void rag.saveRagConfig({ ...settings, include_transcript:e.target.checked })} />Lập chỉ mục transcript trong cuộc họp</label>
    </div>}
    <p className="text-sm text-muted-foreground">{rag.indexStatus?.indexed_files ?? 0} tài liệu · {rag.indexStatus?.total_chunks ?? 0} đoạn{rag.indexStale || rag.sourcesChangedSinceBuild ? " · Cần dựng lại chỉ mục" : ""}</p>
    <div className="flex flex-wrap gap-2">
      <button disabled={rag.isCheckingConnection || rag.isIndexing} className="rounded border border-border px-3 py-2 text-sm disabled:opacity-50" onClick={() => void rag.checkEmbeddingStatus()}>Kiểm tra embedding</button>
      <button disabled={rag.isIndexing} className="rounded bg-primary px-3 py-2 text-sm text-primary-foreground disabled:opacity-50" onClick={() => void rag.rebuildIndex()}>{rag.isIndexing ? "Đang dựng chỉ mục…" : "Dựng lại chỉ mục"}</button>
      <button disabled={rag.isIndexing} className="rounded border border-border px-3 py-2 text-sm disabled:opacity-50" onClick={() => void rag.clearIndex()}>Xóa chỉ mục</button>
    </div>
    {rag.embeddingStatus && <p role="status" className="text-sm">Embedding: {rag.embeddingStatus.connected ? "Sẵn sàng" : "Chưa kết nối"}</p>}
    {rag.indexProgress && <p role="status" className="text-sm">{rag.indexProgress.status} · {rag.indexProgress.filesDone}/{rag.indexProgress.filesTotal}</p>}
    <form className="space-y-2" onSubmit={e => { e.preventDefault(); if (query.trim()) void rag.testSearch(query.trim()); }}><label className="block text-sm">Thử tìm trong tài liệu<input className="mt-1 w-full rounded border border-border bg-background p-2" value={query} onChange={e => setQuery(e.target.value)} /></label><button disabled={rag.isSearching || !query.trim()} className="rounded border border-border px-3 py-2 text-sm disabled:opacity-50">{rag.isSearching ? "Đang tìm…" : "Tìm thử"}</button></form>
    {rag.testSearchResults.map(result => <div key={result.chunk_id} className="rounded border border-border p-3 text-sm"><p className="mb-2 text-xs text-muted-foreground">{result.source_file} · {result.normalized_score.toFixed(2)}</p><p className="whitespace-pre-wrap">{result.text}</p></div>)}
    {rag.error && <p role="alert" className="text-sm text-red-400">{rag.error}</p>}
  </div>;
}
