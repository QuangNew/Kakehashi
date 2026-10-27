import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import { useConfigStore } from "./configStore";
import { useTranslationStore } from "./translationStore";

export interface PreparationStatus {
  stage: string;
  elapsed_ms: number;
  translation_ms: number;
  whisper_ms: number;
  error: string | null;
  cached_whisper_models: number;
}
export const preparing = (stage: string) => ["translation", "whisper", "starting"].includes(stage);
const idle: PreparationStatus = {stage:"idle",elapsed_ms:0,translation_ms:0,whisper_ms:0,error:null,cached_whisper_models:0};
export const usePreparationStore = create<{
  status: PreparationStatus;
  prepare: () => Promise<void>;
  release: () => Promise<void>;
}>((set,get)=>({
  status: idle,
  prepare: async()=>{
    if(preparing(get().status.stage)) return;
    const config=useConfigStore.getState();
    const audio=config.meetingAudioConfig;
    const translation=useTranslationStore.getState();
    set({status:{...idle,stage:"starting"}});
    try {
      const status=await invoke<PreparationStatus>("prepare_meeting_models",{
        youModel: audio?.you.stt_provider === "whisper_cpp" ? audio.you.local_model_id || config.activeWhisperModel : null,
        themModel: audio?.them.stt_provider === "whisper_cpp" ? audio.them.local_model_id || config.activeWhisperModel : null,
        youLanguage: audio?.you.language ?? null,
        themLanguage: audio?.them.language ?? null,
        translation: translation.autoTranslateEnabled && translation.provider === "hy-mt",
      });
      set({status});
    }catch(error){set({status:{...get().status,stage:"error",error:String(error)}});}
  },
  release: async()=>{
    try{await invoke("release_prepared_models");set({status:idle});}
    catch(error){set({status:{...get().status,error:String(error)}});}
  },
}));
