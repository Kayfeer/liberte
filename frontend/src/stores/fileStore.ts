import { create } from "zustand";
import { listChannelFiles, type FileTransferDto } from "@/lib/tauri";

interface FileState {
  transfers: Record<string, FileTransferDto>;
  errors: Record<string, string | null>;
  load: (channelId: string) => Promise<void>;
  upsert: (transfer: FileTransferDto) => void;
}

export const useFileStore = create<FileState>((set) => ({
  transfers: {}, errors: {},
  load: async (channelId) => {
    try {
      const transfers = await listChannelFiles(channelId);
      set((state) => ({
        transfers: { ...state.transfers, ...Object.fromEntries(transfers.map((file) => [file.fileId, file])) },
        errors: { ...state.errors, [channelId]: null },
      }));
    } catch (error) {
      set((state) => ({ errors: { ...state.errors, [channelId]: String(error) } }));
    }
  },
  upsert: (transfer) => set((state) => ({
    transfers: { ...state.transfers, [transfer.fileId]: transfer },
  })),
}));
