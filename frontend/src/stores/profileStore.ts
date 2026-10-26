import { create } from "zustand";
import type { UserStatus } from "@/lib/types";

export interface PeerProfile {
  userId: string;
  displayName?: string | null;
  bio?: string | null;
  status: UserStatus;
}
const statuses = new Set<string>(["online", "dnd", "idle", "invisible"]);
interface ProfileState {
  profiles: Record<string, PeerProfile>;
  upsert: (profile: PeerProfile) => void;
  setStatus: (userId: string, status: string) => void;
}
export const useProfileStore = create<ProfileState>((set) => ({
  profiles: {},
  upsert: (profile) => {
    if (!statuses.has(profile.status)) return;
    set((state) => ({ profiles: { ...state.profiles, [profile.userId]: profile } }));
  },
  setStatus: (userId, status) => {
    if (!statuses.has(status)) return;
    set((state) => ({ profiles: { ...state.profiles, [userId]: {
      ...(state.profiles[userId] ?? { userId }), status: status as UserStatus,
    } } }));
  },
}));
