import { useState, useEffect, useCallback, useRef } from "react";
import { applyMediaPreferences } from "@/lib/mediaSession";

export interface MediaDeviceInfo {
  deviceId: string;
  label: string;
  kind: "audioinput" | "audiooutput" | "videoinput";
}

export interface MediaDevicesState {
  audioInputs: MediaDeviceInfo[];
  audioOutputs: MediaDeviceInfo[];
  videoInputs: MediaDeviceInfo[];
  selectedAudioInput: string;
  selectedAudioOutput: string;
  selectedVideoInput: string;
  inputVolume: number;
  outputVolume: number;
  noiseSuppression: boolean;
  echoCancellation: boolean;
  autoGainControl: boolean;
  voiceIsolation: boolean;
  iceServers: RTCIceServer[];
  loading: boolean;
  error: string | null;
}

const STORAGE_KEY = "liberte-media-settings";

function loadSaved(): Partial<MediaDevicesState> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
}

function savePersistent(state: Partial<MediaDevicesState>) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify({
    ...loadSaved(),
    selectedAudioInput: state.selectedAudioInput,
    selectedAudioOutput: state.selectedAudioOutput,
    selectedVideoInput: state.selectedVideoInput,
    inputVolume: state.inputVolume,
    outputVolume: state.outputVolume,
    noiseSuppression: state.noiseSuppression,
    echoCancellation: state.echoCancellation,
    autoGainControl: state.autoGainControl,
    voiceIsolation: state.voiceIsolation,
    iceServers: state.iceServers,
  }));
}

export function useMediaDevices() {
  const saved = loadSaved();

  const [state, setState] = useState<MediaDevicesState>({
    audioInputs: [],
    audioOutputs: [],
    videoInputs: [],
    selectedAudioInput: saved.selectedAudioInput ?? "default",
    selectedAudioOutput: saved.selectedAudioOutput ?? "default",
    selectedVideoInput: saved.selectedVideoInput ?? "",
    inputVolume: saved.inputVolume ?? 100,
    outputVolume: saved.outputVolume ?? 100,
    noiseSuppression: saved.noiseSuppression ?? true,
    echoCancellation: saved.echoCancellation ?? true,
    autoGainControl: saved.autoGainControl ?? true,
    voiceIsolation: saved.voiceIsolation ?? false,
    iceServers: Array.isArray(saved.iceServers) ? saved.iceServers : [],
    loading: true,
    error: null,
  });
  const stateRef = useRef(state);
  stateRef.current = state;
  const mounted = useRef(false);

  const enumerateDevices = useCallback(async () => {
    let stream: MediaStream | null = null;
    try {
      // Request permission first so labels are available
      stream = await navigator.mediaDevices.getUserMedia({
        audio: true,
        video: true,
      }).catch(() =>
        // Fallback: audio only if no camera
        navigator.mediaDevices.getUserMedia({ audio: true }).catch(() => null)
      );

      const devices = await navigator.mediaDevices.enumerateDevices();

      const audioInputs: MediaDeviceInfo[] = [];
      const audioOutputs: MediaDeviceInfo[] = [];
      const videoInputs: MediaDeviceInfo[] = [];

      for (const d of devices) {
        const info: MediaDeviceInfo = {
          deviceId: d.deviceId,
          label: d.label || `${d.kind} (${d.deviceId.slice(0, 8)})`,
          kind: d.kind as MediaDeviceInfo["kind"],
        };
        if (d.kind === "audioinput") audioInputs.push(info);
        else if (d.kind === "audiooutput") audioOutputs.push(info);
        else if (d.kind === "videoinput") videoInputs.push(info);
      }

      if (!mounted.current) return;
      setState((prev) => ({
        ...prev,
        audioInputs,
        audioOutputs,
        videoInputs,
        loading: false,
        error: null,
      }));
    } catch (err) {
      if (!mounted.current) return;
      setState((prev) => ({
        ...prev,
        loading: false,
        error: err instanceof Error ? err.message : "Impossible d'accéder aux périphériques",
      }));
    } finally {
      stream?.getTracks().forEach((track) => track.stop());
    }
  }, []);

  useEffect(() => {
    mounted.current = true;
    void enumerateDevices();

    // Listen for device changes (plug/unplug)
    navigator.mediaDevices.addEventListener("devicechange", enumerateDevices);
    return () => {
      mounted.current = false;
      navigator.mediaDevices.removeEventListener("devicechange", enumerateDevices);
    };
  }, [enumerateDevices]);

  const update = useCallback(async (patch: Partial<MediaDevicesState>): Promise<boolean> => {
    const next = { ...stateRef.current, ...patch, error: null };
    try { savePersistent(next); }
    catch (cause) { setState((previous) => ({ ...previous, error: String(cause) })); return false; }
    stateRef.current = next;
    setState(next);
    try {
      await applyMediaPreferences();
      return true;
    } catch (cause) {
      if (mounted.current) setState((previous) => ({ ...previous, error: String(cause) }));
      return false;
    }
  }, []);

  return { ...state, update, refresh: enumerateDevices };
}
