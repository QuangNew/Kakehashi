import type { TranscriptSegment, TranslationResult } from "./types";

export interface TranslationRoute { source: string; target: string }
export type TranslationDirection = "vi-ja" | "ja-vi";

export function translationKey(segmentId: string, target: string): string { return `${segmentId}:${target}`; }

/** Display names and diarization IDs do not change the original audio side. */
export function routeTranslation(
  segment: Pick<TranscriptSegment, "speaker">,
  bidirectional: boolean,
  direction: TranslationDirection,
  source: string,
  target: string,
): TranslationRoute | null {
  if (!bidirectional) return { source, target };
  const you = segment.speaker === "User";
  if (!you && segment.speaker !== "Them" && segment.speaker !== "Interviewer") return null;
  const mic = direction === "vi-ja" ? "vi" : "ja";
  const remote = mic === "vi" ? "ja" : "vi";
  return you ? { source: mic, target: remote } : { source: remote, target: mic };
}

export function selectTranslation(
  translations: Map<string, TranslationResult>, segment: TranscriptSegment,
  bidirectional: boolean, direction: TranslationDirection, source: string, target: string,
): TranslationResult | undefined {
  const route = routeTranslation(segment, bidirectional, direction, source, target);
  if (!route) return undefined;
  const ids = (segment as TranscriptSegment & { originalIds?: string[] }).originalIds;
  if (ids && ids.length > 1) {
    const parts = ids.map((id) => translations.get(translationKey(id, route.target)) ?? translations.get(id));
    if (parts.some((part) => !part || part.target_lang !== route.target)) return undefined;
    const ready = parts as TranslationResult[];
    if (ready.map((part) => part.original_text.trim()).join(" ") !== segment.text.trim()) return undefined;
    return { ...ready[0], segment_id: segment.id, original_text: segment.text, translated_text: ready.map((part) => part.translated_text).join(" ") };
  }
  const result = translations.get(translationKey(segment.id, route.target)) ?? translations.get(segment.id);
  return result?.target_lang === route.target && result.original_text === segment.text ? result : undefined;
}
