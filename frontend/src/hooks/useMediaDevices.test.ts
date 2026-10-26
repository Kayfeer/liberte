import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useMediaDevices } from "./useMediaDevices";

const mocks = vi.hoisted(() => ({ applyMediaPreferences: vi.fn() }));
vi.mock("@/lib/mediaSession", () => ({ applyMediaPreferences: mocks.applyMediaPreferences }));

describe("Media settings persistence result", () => {
  beforeEach(() => {
    localStorage.clear();
    mocks.applyMediaPreferences.mockResolvedValue(undefined);
    Object.defineProperty(navigator, "mediaDevices", { configurable: true, value: {
      getUserMedia: vi.fn().mockResolvedValue({ getTracks: () => [] }),
      enumerateDevices: vi.fn().mockResolvedValue([]), addEventListener: vi.fn(), removeEventListener: vi.fn(),
    } });
  });
  it("keeps previous values and reports a failed storage write", async () => {
    const { result } = renderHook(() => useMediaDevices());
    await waitFor(() => expect(result.current.loading).toBe(false));
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => { throw new Error("storage unavailable"); });
    let saved = true;
    await act(async () => { saved = await result.current.update({ inputVolume: 80 }); });
    expect(saved).toBe(false);
    expect(result.current.inputVolume).toBe(100);
    expect(result.current.error).toContain("storage unavailable");
    expect(mocks.applyMediaPreferences).not.toHaveBeenCalled();
  });
  it("returns failure with the actual media application error after persisting", async () => {
    const { result } = renderHook(() => useMediaDevices());
    await waitFor(() => expect(result.current.loading).toBe(false));
    mocks.applyMediaPreferences.mockRejectedValue(new Error("audio device unavailable"));
    let saved = true;
    await act(async () => { saved = await result.current.update({ selectedAudioInput: "mic2" }); });
    expect(saved).toBe(false);
    expect(result.current.error).toContain("audio device unavailable");
    expect(JSON.parse(localStorage.getItem("liberte-media-settings")!).selectedAudioInput).toBe("mic2");
  });
});
