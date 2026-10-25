import type { UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useMeetingStore } from "../stores/meetingStore";
import { meetingEndRoute } from "./windows";
import { onWindowMeetingEnded, emitStopMeeting } from "./events";
const MEETING_WINDOWS = ["launcher", "overlay"];
export const MEETING_END_TIMEOUT_MS = 60_000;
export async function endMeetingInAllWindows(
  timeoutMs: number = MEETING_END_TIMEOUT_MS,
): Promise<boolean> {
  const waiting = new Set(MEETING_WINDOWS);
  let finish: (ok: boolean) => void = () => {};
  const done = new Promise<boolean>((resolve) => {
    finish = resolve;
  });

  let unlisten: UnlistenFn | undefined;
  const timer = setTimeout(() => finish(false), timeoutMs);
  try {
    unlisten = await onWindowMeetingEnded((e) => {
      waiting.delete(e.window);
      if (waiting.size === 0) finish(true);
    });
    await emitStopMeeting();
    return await done;
  } catch (err) {
    console.error("[meeting] Failed to end the meeting:", err);
    return false;
  } finally {
    clearTimeout(timer);
    unlisten?.();
  }
}

/**
 * End the running meeting from this window. When the overlay took the
 * meeting and this is another window, it ends through every window so the
 * overlay saves the data only it holds; resolves once both are done (false
 * on timeout). Otherwise this window ends it by itself.
 */
export async function endMeetingFromThisWindow(): Promise<boolean> {
  const state = useMeetingStore.getState();
  const meeting = state.activeMeeting;
  if (!meeting) return true;
  let label = "";
  try {
    label = getCurrentWebviewWindow().label;
  } catch {
    /* not running in Tauri */
  }
  if (meetingEndRoute(label, state.overlayMeetingId === meeting.id) === "all_windows") {
    return endMeetingInAllWindows();
  }
  await state.endMeetingFlow();
  return true;
}

