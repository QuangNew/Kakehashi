import { describe, expect, it } from "vitest";
import { applyPreset, BUILT_IN_PRESETS } from "./presets";

describe("local Vietnamese-Japanese audio preset", () => {
  it("selects multilingual Whisper Small with independent recognition languages", () => {
    const config = applyPreset(BUILT_IN_PRESETS.find((preset) => preset.you.language === "vi-VN")!, []);
    expect(config.you).toMatchObject({ role: "You", stt_provider: "whisper_cpp", language: "vi-VN", local_model_id: "large-v3-turbo", is_input_device: true });
    expect(config.them).toMatchObject({ role: "Them", stt_provider: "whisper_cpp", language: "ja-JP", local_model_id: "large-v3-turbo", is_input_device: false });
  });
  it("provides the reverse Japanese-to-Vietnamese source preset", () => {
    const config = applyPreset(BUILT_IN_PRESETS.find((preset) => preset.you.language === "ja-JP")!, []);
    expect(config.you.language).toBe("ja-JP");
    expect(config.them.language).toBe("vi-VN");
  });
});
