import {describe,expect,it} from "vitest";
import {isChatModel} from "./chatModels";
describe("copilot model capabilities",()=>{
  it.each(["whisper-large-v3-turbo","openai/whisper-large-v3","gpt-4o-mini-transcribe","tts-1","text-embedding-3-small"])("excludes %s from chat selection",id=>expect(isChatModel(id)).toBe(false));
  it.each(["llama-3.3-70b-versatile","gemini-2.5-flash","copilot","my-custom-model"])("keeps %s available",id=>expect(isChatModel(id)).toBe(true));
});
