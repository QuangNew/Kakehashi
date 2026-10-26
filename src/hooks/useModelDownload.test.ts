import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ModelDownloadProgress } from "../lib/types";

const mocks = vi.hoisted(() => ({ listen: vi.fn(), snapshot: vi.fn(), start: vi.fn(), cancel: vi.fn(), toast: vi.fn(),
  event: undefined as undefined | ((event: { payload: ModelDownloadProgress }) => void), stop: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));
vi.mock("../lib/ipc", () => ({ getModelDownloads: mocks.snapshot, downloadLocalSTTModel: mocks.start, cancelModelDownload: mocks.cancel }));
vi.mock("../stores/toastStore", () => ({ showToast: mocks.toast }));
import { useModelDownload } from "./useModelDownload";
import { useModelDownloadStore } from "../stores/modelDownloadStore";

const progress = (revision: number, status: ModelDownloadProgress["status"] = "downloading"): ModelDownloadProgress => ({
  engine: "whisper_cpp", model_id: "large-v3-turbo", downloaded_bytes: revision * 1000,
  total_bytes: 100000, percent: revision, status, revision,
});
const key = "whisper_cpp:large-v3-turbo";
beforeEach(() => {
  vi.useFakeTimers(); vi.clearAllMocks();
  useModelDownloadStore.setState({ downloads: {} });
  mocks.listen.mockImplementation(async (_event, callback) => { mocks.event = callback; return mocks.stop; });
  mocks.snapshot.mockResolvedValue([]); mocks.start.mockResolvedValue(undefined);
});
afterEach(() => { cleanup(); vi.useRealTimers(); });

describe("model download state across settings tabs", () => {
  it("recovers the background download after unmount and remount without another click", async () => {
    mocks.snapshot.mockResolvedValueOnce([progress(10)]);
    const first = renderHook(() => useModelDownload());
    await act(async () => {});
    expect(first.result.current.downloads[key].downloaded_bytes).toBe(10000);
    first.unmount();
    mocks.snapshot.mockResolvedValueOnce([progress(40)]);
    const second = renderHook(() => useModelDownload());
    await act(async () => {});
    expect(second.result.current.syncing).toBe(false);
    expect(second.result.current.downloads[key].downloaded_bytes).toBe(40000);
    expect(mocks.start).not.toHaveBeenCalled();
  });
  it("does not let a slow snapshot overwrite a newer event", async () => {
    let resolve!: (value: ModelDownloadProgress[]) => void;
    mocks.snapshot.mockImplementationOnce(() => new Promise((done) => { resolve = done; }));
    const hook = renderHook(() => useModelDownload());
    await act(async () => {});
    await act(async () => { mocks.event?.({ payload: progress(80, "complete") }); resolve([progress(20)]); });
    expect(hook.result.current.downloads[key].status).toBe("complete");
    expect(hook.result.current.downloads[key].revision).toBe(80);
  });
  it("blocks a double click while the first download request is pending", async () => {
    let finish!: () => void;
    mocks.start.mockImplementationOnce(() => new Promise<void>((resolve) => { finish = resolve; }));
    const hook = renderHook(() => useModelDownload()); await act(async () => {});
    await act(async () => {
      void hook.result.current.startDownload("whisper_cpp", "large-v3-turbo");
      void hook.result.current.startDownload("whisper_cpp", "large-v3-turbo");
    });
    expect(mocks.start).toHaveBeenCalledTimes(1);
    await act(async () => { finish(); });
  });
  it("shows and retains a failed download, then permits resuming it", async () => {
    mocks.snapshot.mockResolvedValue([progress(5, "error")]);
    const hook = renderHook(() => useModelDownload()); await act(async () => {});
    await act(async () => { await vi.advanceTimersByTimeAsync(5000); });
    expect(hook.result.current.downloads[key].status).toBe("error");
    mocks.snapshot.mockResolvedValue([progress(6)]);
    await act(async () => { await hook.result.current.startDownload("whisper_cpp", "large-v3-turbo"); });
    expect(hook.result.current.downloads[key].status).toBe("downloading");
    expect(mocks.start).toHaveBeenCalledTimes(1);
  });
  it("cleans up a listener that resolves after the tab has closed", async () => {
    let resolve!: (stop: () => void) => void;
    mocks.listen.mockImplementationOnce(() => new Promise((done) => { resolve = done; }));
    const hook = renderHook(() => useModelDownload()); hook.unmount();
    await act(async () => { resolve(mocks.stop); });
    expect(mocks.stop).toHaveBeenCalledTimes(1);
    expect(mocks.snapshot).not.toHaveBeenCalled();
  });
});
