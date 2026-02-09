import { create } from "zustand";
import * as tauri from "@/lib/tauri";

interface BackupState {
  lastBackupTime: string | null;
  lastBackupPath: string | null;
  backups: tauri.BackupFileInfo[];
  autoBackupEnabled: boolean;
  intervalMinutes: number;
  isBackingUp: boolean;
  isRestoring: boolean;
  error: string | null;

  runAutoBackup: () => Promise<void>;
  saveToFile: (filePath: string) => Promise<void>;
  importFromFile: (json: string) => Promise<tauri.ImportStats>;
  loadBackupList: () => Promise<void>;
  setAutoBackup: (enabled: boolean) => void;
  setInterval: (minutes: number) => void;
}

const STORAGE_KEY = "liberte-backup-config";

function loadConfig(): { enabled: boolean; interval: number } {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return JSON.parse(raw);
  } catch { /* ignore */ }
  return { enabled: true, interval: 30 };
}

function saveConfig(enabled: boolean, interval: number) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify({ enabled, interval }));
}

export const useBackupStore = create<BackupState>((set, get) => {
  const config = loadConfig();

  return {
    lastBackupTime: null,
    lastBackupPath: null,
    backups: [],
    autoBackupEnabled: config.enabled,
    intervalMinutes: config.interval,
    isBackingUp: false,
    isRestoring: false,
    error: null,

    runAutoBackup: async () => {
      if (get().isBackingUp) return;
      set({ isBackingUp: true, error: null });
      try {
        const path = await tauri.autoBackup();
        set({
          lastBackupTime: new Date().toISOString(),
          lastBackupPath: path,
          isBackingUp: false,
        });
      } catch (e) {
        set({ error: String(e), isBackingUp: false });
      }
    },

    saveToFile: async (filePath: string) => {
      set({ isBackingUp: true, error: null });
      try {
        await tauri.saveBackupToFile(filePath);
        set({ isBackingUp: false, lastBackupTime: new Date().toISOString() });
      } catch (e) {
        set({ error: String(e), isBackingUp: false });
      }
    },

    importFromFile: async (json: string) => {
      set({ isRestoring: true, error: null });
      try {
        const stats = await tauri.importBackup(json);
        set({ isRestoring: false });
        return stats;
      } catch (e) {
        set({ error: String(e), isRestoring: false });
        throw e;
      }
    },

    loadBackupList: async () => {
      try {
        const backups = await tauri.listBackups();
        set({ backups });
      } catch {
        // ignore — dir may not exist yet
      }
    },

    setAutoBackup: (enabled: boolean) => {
      set({ autoBackupEnabled: enabled });
      saveConfig(enabled, get().intervalMinutes);
    },

    setInterval: (minutes: number) => {
      set({ intervalMinutes: minutes });
      saveConfig(get().autoBackupEnabled, minutes);
    },
  };
});
