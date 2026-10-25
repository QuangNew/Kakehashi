// Display helpers for API token usage and response latency.
export function formatTokens(n: number): string {
  return new Intl.NumberFormat("en-US").format(Math.round(n));
}

export function formatDuration(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) return "—";
  if (ms < 1000) return `${Math.round(ms)} ms`;
  const s = ms / 1000;
  if (s < 10) return `${(Math.round(s * 10) / 10).toFixed(1)} s`;
  if (s < 60) return `${Math.round(s)} s`;
  const m = Math.floor(s / 60);
  const rest = Math.round(s - m * 60);
  return rest > 0 ? `${m} min ${rest} s` : `${m} min`;
}

export function lastAnswerTooltip(latencyMs: number, firstTokenMs: number | null): string {
  const total = `Last answer: ${formatDuration(latencyMs)} total`;
  return firstTokenMs != null ? `${total}, first words after ${formatDuration(firstTokenMs)}` : total;
}
