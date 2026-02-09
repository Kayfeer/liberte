import { create } from "zustand";
import type { Message, Channel } from "@/lib/types";
import { MESSAGE_PAGE_SIZE } from "@/lib/constants";
import * as tauri from "@/lib/tauri";

interface MessageState {
  channels: Channel[];
  activeChannelId: string | null;
  messages: Record<string, Message[]>;
  loading: boolean;

  loadChannels: () => Promise<void>;
  setActiveChannel: (channelId: string) => void;
  loadMessages: (channelId: string) => Promise<void>;
  sendMessage: (channelId: string, content: string) => Promise<void>;
  addMessage: (message: Message) => void;
}

export const useMessageStore = create<MessageState>((set, get) => ({
  channels: [],
  activeChannelId: null,
  messages: {},
  loading: false,

  loadChannels: async () => {
    const channels = await tauri.listChannels();
    set({ channels });
  },

  setActiveChannel: (channelId: string) => {
    set({ activeChannelId: channelId });
    get().loadMessages(channelId);
  },

  loadMessages: async (channelId: string) => {
    set({ loading: true });
    try {
      const msgs = await tauri.getMessages(channelId, MESSAGE_PAGE_SIZE, 0);
      set((state) => ({
        messages: { ...state.messages, [channelId]: msgs },
        loading: false,
      }));
    } catch {
      set({ loading: false });
    }
  },

  sendMessage: async (channelId: string, content: string) => {
    await tauri.sendMessage(channelId, content);
  },

  addMessage: (message: Message) => {
    set((state) => {
      const existing = state.messages[message.channelId] || [];
      return {
        messages: {
          ...state.messages,
          [message.channelId]: [...existing, message],
        },
      };
    });
  },
}));
