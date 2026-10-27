import { invoke } from "@tauri-apps/api/core";

export const API_PROVIDERS = [
  ["groq", "Groq"], ["gemini", "Google AI Studio / Gemini"], ["openai", "OpenAI"],
  ["anthropic", "Anthropic"], ["openrouter", "OpenRouter"], ["deepgram", "Deepgram"],
  ["azure_speech", "Azure Speech"], ["microsoft", "Microsoft Translator"],
  ["google_translation", "Google Cloud Translation"], ["deepl", "DeepL"], ["custom", "Custom API"],
] as const;
export type ApiUsage = "copilot" | "translation" | "stt";
export interface ApiPoolEntry { id: string; provider: string; label: string; region: string | null; key_present: boolean }
export interface ApiPoolSnapshot { entries: ApiPoolEntry[]; defaults: Record<string,string>; bindings: Record<string,string> }
export interface ApiPoolInput { id?: string; provider: string; label: string; key?: string; region?: string }
export const poolProvider = (provider: string) => (({ whisper_api:"openai", groq_whisper:"groq", google:"google_translation" } as Record<string,string>)[provider] ?? provider);
export function selectedCredential(snapshot: ApiPoolSnapshot, usage: ApiUsage, provider: string) {
  const canonical = poolProvider(provider);
  const id = snapshot.bindings[`${usage}.${canonical}`] ?? snapshot.defaults[canonical];
  return snapshot.entries.find(entry => entry.id === id && entry.provider === canonical && entry.key_present);
}
export const getApiPool = () => invoke<ApiPoolSnapshot>("get_api_pool");
export const saveApiPoolEntry = (input: ApiPoolInput) => invoke<string>("save_api_pool_entry",{input});
export const deleteApiPoolEntry = (id: string) => invoke<void>("delete_api_pool_entry",{id});
export const bindApiPoolEntry = (usage: ApiUsage, provider: string, id: string | null) => invoke<void>("bind_api_pool_entry",{usage,provider:poolProvider(provider),id});
