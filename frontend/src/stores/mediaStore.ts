import { create } from "zustand";
import type { PeerInfo } from "@/lib/types";
import * as tauri from "@/lib/tauri";

interface MediaState {
  inCall: boolean;
  channelId: string | null;
  participants: PeerInfo[];
  isMuted: boolean;
  isVideoEnabled: boolean;
  mode: "mesh" | "sfu";

  startCall: (channelId: string) => Promise<void>;
  endCall: () => Promise<void>;
  toggleMute: () => Promise<void>;
  toggleVideo: () => Promise<void>;
  setParticipants: (participants: PeerInfo[]) => void;
  setMode: (mode: "mesh" | "sfu") => void;
}

export const useMediaStore = create<MediaState>((set) => ({
  inCall: false,
  channelId: null,
  participants: [],
  isMuted: false,
  isVideoEnabled: false,
  mode: "mesh",

  startCall: async (channelId: string) => {
    await tauri.startCall(channelId);
    set({ inCall: true, channelId });
  },

  endCall: async () => {
    await tauri.endCall();
    set({ inCall: false, channelId: null, participants: [] });
  },

  toggleMute: async () => {
    const muted = await tauri.toggleMute();
    set({ isMuted: muted });
  },

  toggleVideo: async () => {
    const enabled = await tauri.toggleVideo();
    set({ isVideoEnabled: enabled });
  },

  setParticipants: (participants) => set({ participants }),
  setMode: (mode) => set({ mode }),
}));
