import { create } from "zustand";

type Page = "home" | "settings";

interface NavigationState {
  currentPage: Page;
  navigate: (page: Page) => void;
}

export const useNavigationStore = create<NavigationState>((set) => ({
  currentPage: "home",
  navigate: (page) => set({ currentPage: page }),
}));
