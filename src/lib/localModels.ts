import { invoke } from "@tauri-apps/api/core";

export type ModelRole = "translation" | "copilot" | "embeddings";
export interface ModelSettings { file: string; context: number; gpu_layers: number }
export interface LocalModelsConfig {
  models_dir: string;
  server_path: string;
  translation: ModelSettings;
  copilot: ModelSettings;
  embeddings: ModelSettings;
}
export interface RuntimeStatus { role: ModelRole; state: string; pid?: number; endpoint: string; error?: string; device?: string }
export const MANAGED_COPILOT_URL = "http://127.0.0.1:18182/v1";
export const MANAGED_EMBEDDING_URL = "http://127.0.0.1:18183/v1";
export const getLocalModelsConfig = () => invoke<LocalModelsConfig>("get_local_models_config");
export const saveLocalModelsConfig = (config: LocalModelsConfig) => invoke<void>("save_local_models_config", { config });
export const getLocalModelsStatus = () => invoke<RuntimeStatus[]>("get_local_models_status");
export const startLocalModel = (role: ModelRole) => invoke<string>("start_local_model", { role });
export const stopLocalModel = (role: ModelRole) => invoke<void>("stop_local_model", { role });
