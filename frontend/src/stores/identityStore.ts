import { create } from "zustand";
import type { IdentityInfo } from "@/lib/types";
import * as tauri from "@/lib/tauri";

interface IdentityState {
  identity: IdentityInfo | null;
  loading: boolean;
  error: string | null;
  createIdentity: () => Promise<void>;
  loadIdentity: () => Promise<void>;
}

export const useIdentityStore = create<IdentityState>((set) => ({
  identity: null,
  loading: false,
  error: null,

  createIdentity: async () => {
    set({ loading: true, error: null });
    try {
      const identity = await tauri.createIdentity();
      set({ identity, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  loadIdentity: async () => {
    set({ loading: true, error: null });
    try {
      const identity = await tauri.loadIdentity();
      set({ identity, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },
}));
