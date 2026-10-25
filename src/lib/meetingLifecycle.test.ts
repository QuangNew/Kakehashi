import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
const meeting = vi.hoisted(() => ({activeMeeting:null as {id:string} | null,overlayMeetingId:null as string | null,endMeetingFlow:vi.fn()}));
const label = vi.hoisted(() => ({value:"launcher"}));
const listeners = vi.hoisted(() => new Set<(event:{window:string}) => void>());
const stop = vi.hoisted(() => vi.fn(async () => {}));
vi.mock("../stores/meetingStore", () => ({useMeetingStore:{getState:() => meeting}}));
vi.mock("@tauri-apps/api/webviewWindow", () => ({getCurrentWebviewWindow:() => ({label:label.value})}));
vi.mock("./events", () => ({emitStopMeeting:stop,onWindowMeetingEnded:async (callback:(event:{window:string}) => void) => {listeners.add(callback);return () => {listeners.delete(callback);};}}));
import { endMeetingFromThisWindow, MEETING_END_TIMEOUT_MS } from "./meetingLifecycle";
describe("meeting save flow after removing updater", () => {
  beforeEach(() => { vi.useFakeTimers(); listeners.clear(); stop.mockClear(); meeting.endMeetingFlow.mockReset().mockResolvedValue(undefined); meeting.activeMeeting={id:"m1"}; meeting.overlayMeetingId="m1";label.value="launcher"; });
  afterEach(() => vi.useRealTimers());
  it("waits for both launcher and overlay to finish saving", async () => {
    let ended:boolean | undefined;
    const pending=endMeetingFromThisWindow().then(result => {ended=result;});
    await Promise.resolve(); expect(stop).toHaveBeenCalledOnce();
    listeners.forEach(callback => callback({window:"launcher"}));
    await Promise.resolve(); expect(ended).toBeUndefined();
    listeners.forEach(callback => callback({window:"overlay"}));
    await pending;expect(ended).toBe(true);expect(listeners.size).toBe(0);
  });
  it("reports an unfinished save on timeout and removes its listeners", async () => {
    const pending=endMeetingFromThisWindow();await Promise.resolve();
    listeners.forEach(callback => callback({window:"launcher"}));
    await vi.advanceTimersByTimeAsync(MEETING_END_TIMEOUT_MS);
    expect(await pending).toBe(false);expect(listeners.size).toBe(0);
  });
  it("ends locally when the overlay has not taken the meeting", async () => {
    meeting.overlayMeetingId=null;expect(await endMeetingFromThisWindow()).toBe(true);
    expect(meeting.endMeetingFlow).toHaveBeenCalledOnce();expect(stop).not.toHaveBeenCalled();
  });
  it("lets the overlay end its own meeting", async () => {
    label.value="overlay";expect(await endMeetingFromThisWindow()).toBe(true);expect(meeting.endMeetingFlow).toHaveBeenCalledOnce();
  });
  it("returns immediately if there is no active meeting", async () => {
    meeting.activeMeeting=null;expect(await endMeetingFromThisWindow()).toBe(true);expect(stop).not.toHaveBeenCalled();expect(meeting.endMeetingFlow).not.toHaveBeenCalled();
  });
});
