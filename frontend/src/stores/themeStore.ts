import { create } from "zustand";

export type ThemeName = "dark" | "light" | "midnight" | "custom";

export interface ThemeColors {
  bg: string;
  surface: string;
  panel: string;
  accent: string;
  text: string;
  muted: string;
  success: string;
  warning: string;
  border: string;
}

const DARK_COLORS: ThemeColors = {
  bg: "#1a1a2e",
  surface: "#16213e",
  panel: "#0f3460",
  accent: "#e94560",
  text: "#eaeaea",
  muted: "#8a8a9a",
  success: "#4ade80",
  warning: "#fbbf24",
  border: "#2a2a4a",
};

const LIGHT_COLORS: ThemeColors = {
  bg: "#f5f5f5",
  surface: "#ffffff",
  panel: "#e8edf3",
  accent: "#e94560",
  text: "#1a1a2e",
  muted: "#6b7280",
  success: "#22c55e",
  warning: "#f59e0b",
  border: "#d1d5db",
};

const MIDNIGHT_COLORS: ThemeColors = {
  bg: "#0d0d1a",
  surface: "#111127",
  panel: "#1a1a3e",
  accent: "#8b5cf6",
  text: "#e2e8f0",
  muted: "#64748b",
  success: "#34d399",
  warning: "#fbbf24",
  border: "#1e1e3a",
};

const THEME_MAP: Record<Exclude<ThemeName, "custom">, ThemeColors> = {
  dark: DARK_COLORS,
  light: LIGHT_COLORS,
  midnight: MIDNIGHT_COLORS,
};

const STORAGE_KEY = "liberte-theme";

interface ThemeState {
  themeName: ThemeName;
  colors: ThemeColors;
  customColors: ThemeColors;
  setTheme: (name: ThemeName) => void;
  setCustomColor: (key: keyof ThemeColors, value: string) => void;
}

function loadPersistedTheme(): { name: ThemeName; custom: ThemeColors } {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      return {
        name: parsed.name ?? "dark",
        custom: parsed.custom ?? { ...DARK_COLORS },
      };
    }
  } catch {
    /* ignore */
  }
  return { name: "dark", custom: { ...DARK_COLORS } };
}

function persist(name: ThemeName, custom: ThemeColors) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify({ name, custom }));
}

function resolveColors(name: ThemeName, custom: ThemeColors): ThemeColors {
  if (name === "custom") return custom;
  return THEME_MAP[name];
}

function applyToDOM(colors: ThemeColors) {
  const root = document.documentElement;
  for (const [key, value] of Object.entries(colors)) {
    root.style.setProperty(`--liberte-${key}`, value);
  }
}

export const useThemeStore = create<ThemeState>((set, get) => {
  const saved = loadPersistedTheme();
  const colors = resolveColors(saved.name, saved.custom);

  // Apply immediately on store creation
  setTimeout(() => applyToDOM(colors), 0);

  return {
    themeName: saved.name,
    colors,
    customColors: saved.custom,

    setTheme: (name) => {
      const custom = get().customColors;
      const colors = resolveColors(name, custom);
      applyToDOM(colors);
      persist(name, custom);
      set({ themeName: name, colors });
    },

    setCustomColor: (key, value) => {
      const custom = { ...get().customColors, [key]: value };
      const colors = resolveColors("custom", custom);
      applyToDOM(colors);
      persist("custom", custom);
      set({ themeName: "custom", customColors: custom, colors });
    },
  };
});
