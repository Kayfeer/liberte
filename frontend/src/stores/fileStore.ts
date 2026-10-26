import { create } from "zustand";
import { listChannelFiles, type FileTransferDto } from "@/lib/tauri";

interface FileState {
  transfers: Record<string, FileTransferDto>;
  errors: Record<string, string | null>;
  load: (channelId: string) => Promise<void>;
  upsert: (transfer: FileTransferDto) => void;
  remove: (fileId: string) => void;
}

// Ignore late progress events or in-flight list responses for dismissed IDs.
const dismissed = new Set<string>();
const requestVersions = new Map<string, number>();
const eventVersions = new Map<string, number>();
let eventVersion = 0;
export const useFileStore = create<FileState>((set) => ({
  transfers: {}, errors: {},
  load: async (channelId) => {
    const request = (requestVersions.get(channelId) || 0) + 1;
    requestVersions.set(channelId, request);
    const startedAtVersion = eventVersion;
    try {
      const transfers = await listChannelFiles(channelId);
      if (requestVersions.get(channelId) !== request) return;
      set((state) => ({
        transfers: { ...state.transfers, ...Object.fromEntries(transfers.filter((file) => !dismissed.has(file.fileId) && (eventVersions.get(file.fileId) || 0) <= startedAtVersion).map((file) => [file.fileId, file])) },
        errors: { ...state.errors, [channelId]: null },
      }));
    } catch (error) {
      if (requestVersions.get(channelId) !== request) return;
      set((state) => ({ errors: { ...state.errors, [channelId]: String(error) } }));
    }
  },
  upsert: (transfer) => {
    eventVersions.set(transfer.fileId, ++eventVersion);
    set((state) => ({
      transfers: dismissed.has(transfer.fileId) ? state.transfers : { ...state.transfers, [transfer.fileId]: transfer },
    }));
  },
  remove: (fileId) => {
    dismissed.add(fileId);
    set((state) => {
      const transfers = { ...state.transfers };
      delete transfers[fileId];
      return { transfers };
    });
  },
}));
