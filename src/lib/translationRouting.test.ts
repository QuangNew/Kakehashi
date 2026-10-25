import { describe, expect, it } from "vitest";
import { routeTranslation, selectTranslation, translationKey } from "./translationRouting";
import type { TranscriptSegment, TranslationResult } from "./types";

const segment: TranscriptSegment = { id: "line", text: "Xin chào", speaker: "User", timestamp_ms: 0, confidence: 1, is_final: true };
const result = (id: string, text: string, target: string, translated: string): TranslationResult => ({ segment_id: id, original_text: text, source_lang: "vi", target_lang: target, translated_text: translated, provider: "Hy-MT Local" });

describe("bilingual subtitles", () => {
  it("routes audio sides independently of diarization identities", () => {
    const renamed = { ...segment, speaker_id: "renamed-person" };
    expect(routeTranslation(renamed, true, "vi-ja", "auto", "vi")).toEqual({ source: "vi", target: "ja" });
    expect(routeTranslation({ speaker: "Them" }, true, "vi-ja", "auto", "vi")).toEqual({ source: "ja", target: "vi" });
    expect(routeTranslation({ speaker: "Interviewer" }, true, "ja-vi", "auto", "vi")).toEqual({ source: "vi", target: "ja" });
    expect(routeTranslation({ speaker: "User" }, true, "ja-vi", "auto", "vi")).toEqual({ source: "ja", target: "vi" });
  });
  it("keeps unknown audio unattributed and preserves single-target mode", () => {
    expect(routeTranslation({ speaker: "Unknown" }, true, "vi-ja", "auto", "vi")).toBeNull();
    expect(routeTranslation({ speaker: "Unknown" }, false, "vi-ja", "en", "vi")).toEqual({ source: "en", target: "vi" });
  });
  it("keeps two target languages for the same segment", () => {
    const japanese = result("line", "Xin chào", "ja", "こんにちは");
    const english = result("line", "Xin chào", "en", "Hello");
    const map = new Map([[translationKey("line", "ja"), japanese], [translationKey("line", "en"), english]]);
    expect(selectTranslation(map, segment, true, "vi-ja", "auto", "vi")).toBe(japanese);
    expect(selectTranslation(map, segment, false, "vi-ja", "vi", "en")).toBe(english);
  });
  it("does not display a translation of an older final revision", () => {
    const map = new Map([[translationKey("line", "ja"), result("line", "Xin", "ja", "古い結果")]]);
    expect(selectTranslation(map, segment, true, "vi-ja", "auto", "vi")).toBeUndefined();
  });
  it("combines every original translation when consecutive lines are merged", () => {
    const map = new Map([
      [translationKey("a", "ja"), result("a", "Xin chào", "ja", "こんにちは。")],
      [translationKey("b", "ja"), result("b", "Cảm ơn", "ja", "ありがとう。")],
    ]);
    const merged = { ...segment, id: "a", originalIds: ["a", "b"], text: "Xin chào Cảm ơn" };
    expect(selectTranslation(map, merged, true, "vi-ja", "auto", "vi")?.translated_text).toBe("こんにちは。 ありがとう。");
    map.delete(translationKey("b", "ja"));
    expect(selectTranslation(map, merged, true, "vi-ja", "auto", "vi")).toBeUndefined();
  });
});
