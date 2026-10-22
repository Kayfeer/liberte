import { create } from "zustand";
import type { ConnectionMode } from "@/lib/types";
import * as tauri from "@/lib/tauri";

interface NetworkState {
  peers: string[];
  connectionMode: ConnectionMode;
  listenAddresses: string[];
  externalAddresses: string[];
  localPeerId: string | null;
  error: string | null;
  loading: boolean;
  connectPeer: (multiaddr: string) => Promise<void>;
  refreshPeers: () => Promise<void>;
  refreshConnectionMode: () => Promise<void>;
  refreshNetwork: () => Promise<void>;
}

let refreshRequest: Promise<void> | null = null;
export const useNetworkStore = create<NetworkState>((set, get) => ({
  peers: [], connectionMode: "disconnected", listenAddresses: [], externalAddresses: [], localPeerId: null,
  error: null, loading: false,
  connectPeer: async (multiaddr) => {
    set({ loading: true, error: null });
    try {
      await tauri.connectPeer(multiaddr.trim());
      await get().refreshNetwork();
    } catch (error) {
      set({ error: String(error) });
      throw error;
    } finally {
      set({ loading: false });
    }
  },
  refreshPeers: () => get().refreshNetwork(),
  refreshConnectionMode: () => get().refreshNetwork(),
  refreshNetwork: () => {
    if (refreshRequest) return refreshRequest;
    refreshRequest = (async () => {
      try {
        set(await tauri.getNetworkStatus());
      } catch (error) {
        set({ error: String(error), connectionMode: "disconnected" });
      } finally {
        refreshRequest = null;
      }
    })();
    return refreshRequest;
  },
}));
