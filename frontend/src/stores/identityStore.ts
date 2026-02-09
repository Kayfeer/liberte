import { create } from "zustand";
import type { IdentityInfo } from "@/lib/types";
import * as tauri from "@/lib/tauri";

interface IdentityState {
  identity: IdentityInfo | null;
  loading: boolean;
  error: string | null;
  createIdentity: (displayName?: string) => Promise<void>;
  loadIdentity: () => Promise<void>;
  setDisplayName: (name: string) => Promise<void>;
}

export const useIdentityStore = create<IdentityState>((set, get) => ({
  identity: null,
  loading: false,
  error: null,

  createIdentity: async (displayName?: string) => {
    set({ loading: true, error: null });
    try {
      const identity = await tauri.createIdentity(displayName);
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

  setDisplayName: async (name: string) => {
    try {
      await tauri.setDisplayName(name);
      const current = get().identity;
      if (current) {
        set({
          identity: { ...current, displayName: name || undefined },
        });
      }
    } catch (e) {
      console.error("Failed to set display name:", e);
    }
  },
}));
