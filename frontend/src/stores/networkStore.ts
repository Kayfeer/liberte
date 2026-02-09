import { create } from "zustand";
import type { ConnectionMode } from "@/lib/types";
import * as tauri from "@/lib/tauri";

interface NetworkState {
  peers: string[];
  connectionMode: ConnectionMode;
  loading: boolean;
  connectPeer: (multiaddr: string) => Promise<void>;
  refreshPeers: () => Promise<void>;
  refreshConnectionMode: () => Promise<void>;
  setPeers: (peers: string[]) => void;
  setConnectionMode: (mode: ConnectionMode) => void;
}

export const useNetworkStore = create<NetworkState>((set) => ({
  peers: [],
  connectionMode: "disconnected",
  loading: false,

  connectPeer: async (multiaddr: string) => {
    set({ loading: true });
    try {
      await tauri.connectPeer(multiaddr);
      const peers = await tauri.listPeers();
      set({ peers, loading: false });
    } catch (e) {
      set({ loading: false });
      throw e;
    }
  },

  refreshPeers: async () => {
    const peers = await tauri.listPeers();
    set({ peers });
  },

  refreshConnectionMode: async () => {
    const mode = await tauri.getConnectionMode();
    set({ connectionMode: mode });
  },

  setPeers: (peers) => set({ peers }),
  setConnectionMode: (mode) => set({ connectionMode: mode }),
}));
