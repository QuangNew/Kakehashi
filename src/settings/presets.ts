// Meeting Audio Presets — one-click configuration profiles.

import type {
  AudioDevice,
  MeetingAudioConfig,
  STTProviderType,
} from "../lib/types";

export interface MeetingPreset {
  name: string;
  description: string;
  badge?: string;
  you: { hint: DeviceHint; stt: STTProviderType; language?: string; model?: string };
  them: { hint: DeviceHint; stt: STTProviderType; language?: string; model?: string };
  requiresKey?: string;
  requiresDownload?: string;
  windowsOnly?: boolean;
}

/** Hints for resolving device IDs at apply time. */
export type DeviceHint = "default_mic" | "default_output" | "mic_2"; // Default output device         // Second mic (for in-person with two mics)

export const BUILT_IN_PRESETS: MeetingPreset[] = [
  {
    name: "Việt ↔ 日本語 (Local)",
    description: "Mic tiếng Việt · System audio tiếng Nhật · Whisper Large v3 Turbo",
    badge: "VI ↔ JA",
    you: { hint: "default_mic", stt: "whisper_cpp", language: "vi-VN", model: "large-v3-turbo" },
    them: { hint: "default_output", stt: "whisper_cpp", language: "ja-JP", model: "large-v3-turbo" },
    requiresDownload: "whisper_cpp",
  },
  {
    name: "日本語 ↔ Việt (Local)", description: "Mic tiếng Nhật · System audio tiếng Việt · Whisper Large v3 Turbo", badge: "JA ↔ VI",
    you: { hint: "default_mic", stt: "whisper_cpp", language: "ja-JP", model: "large-v3-turbo" },
    them: { hint: "default_output", stt: "whisper_cpp", language: "vi-VN", model: "large-v3-turbo" }, requiresDownload: "whisper_cpp",
  },
  {
    name: "Zero Setup",
    description: "Web Speech + Windows built-in — works instantly",
    badge: "Default",
    you: { hint: "default_mic", stt: "web_speech" },
    them: { hint: "default_output", stt: "windows_native" },
  },
  {
    name: "Best Quality",
    description: "Deepgram cloud cho system audio, cần API key",
    badge: "Cloud",
    you: { hint: "default_mic", stt: "web_speech" },
    them: { hint: "default_output", stt: "deepgram" },
    requiresKey: "deepgram",
  },
  {
    name: "Fully Offline",
    description: "Sherpa-ONNX — no internet needed",
    badge: "Free",
    you: { hint: "default_mic", stt: "sherpa_onnx" },
    them: { hint: "default_output", stt: "sherpa_onnx" },
    requiresDownload: "sherpa_onnx",
  },
  {
    name: "All Cloud",
    description: "Deepgram for both parties",
    you: { hint: "default_mic", stt: "deepgram" },
    them: { hint: "default_output", stt: "deepgram" },
    requiresKey: "deepgram",
  },
  {
    name: "Local Only (Whisper)",
    description: "Whisper.cpp for both parties — offline, free",
    you: { hint: "default_mic", stt: "whisper_cpp" },
    them: { hint: "default_output", stt: "whisper_cpp" },
    requiresDownload: "whisper_cpp",
  },
  {
    name: "In-Person",
    description: "Both parties use mic input — Web Speech + Whisper.cpp",
    you: { hint: "default_mic", stt: "web_speech" },
    them: { hint: "mic_2", stt: "whisper_cpp" },
    requiresDownload: "whisper_cpp",
  },
];

/** Resolve a device hint to an actual device ID. */
function resolveHint(hint: DeviceHint, devices: AudioDevice[]): { device_id: string; is_input: boolean } {
  switch (hint) {
    case "default_mic": {
      const mic = devices.find((d) => d.is_input && d.is_default)
        ?? devices.find((d) => d.is_input);
      return { device_id: mic?.id ?? "default", is_input: true };
    }
    case "default_output": {
      const out = devices.find((d) => !d.is_input && d.is_default)
        ?? devices.find((d) => !d.is_input);
      return { device_id: out?.id ?? "default", is_input: false };
    }
    case "mic_2": {
      // Second microphone — skip the default
      const mics = devices.filter((d) => d.is_input);
      const second = mics.find((d) => !d.is_default) ?? mics[0];
      return { device_id: second?.id ?? "default", is_input: true };
    }
  }
}

/** Apply a preset using the current list of audio devices. */
export function applyPreset(
  preset: MeetingPreset,
  devices: AudioDevice[]
): MeetingAudioConfig {
  const youDevice = resolveHint(preset.you.hint, devices);
  const themDevice = resolveHint(preset.them.hint, devices);

  return {
    you: {
      role: "You",
      device_id: youDevice.device_id,
      is_input_device: youDevice.is_input,
      stt_provider: preset.you.stt,
      ...(preset.you.language && { language: preset.you.language }),
      ...(preset.you.model && { local_model_id: preset.you.model }),
    },
    them: {
      role: "Them",
      device_id: themDevice.device_id,
      is_input_device: themDevice.is_input,
      stt_provider: preset.them.stt,
      ...(preset.them.language && { language: preset.them.language }),
      ...(preset.them.model && { local_model_id: preset.them.model }),
    },
    audio_mode: "online",
    recording_enabled: false,
    preset_name: preset.name,
  };
}
