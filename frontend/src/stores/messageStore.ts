import { create } from "zustand";
import type { Message, Channel } from "@/lib/types";
import { MESSAGE_PAGE_SIZE } from "@/lib/constants";
import * as tauri from "@/lib/tauri";

interface MessageState {
  channels: Channel[];
  activeChannelId: string | null;
  messages: Record<string, Message[]>;
  channelKeys: Record<string, string>;
  loading: boolean;

  loadChannels: () => Promise<void>;
  setActiveChannel: (channelId: string) => void;
  loadMessages: (channelId: string) => Promise<void>;
  sendMessage: (channelId: string, content: string) => Promise<void>;
  addMessage: (message: Message) => void;
  createChannel: (name: string) => Promise<void>;
  setChannelKey: (channelId: string, keyHex: string) => void;
}

export const useMessageStore = create<MessageState>((set, get) => ({
  channels: [],
  activeChannelId: null,
  messages: {},
  channelKeys: {},
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
      const keyHex = get().channelKeys[channelId] || "";
      const msgs = await tauri.getMessages(channelId, keyHex, MESSAGE_PAGE_SIZE, 0);
      set((state) => ({
        messages: { ...state.messages, [channelId]: msgs },
        loading: false,
      }));
    } catch {
      set({ loading: false });
    }
  },

  sendMessage: async (channelId: string, content: string) => {
    const keyHex = get().channelKeys[channelId] || "";
    await tauri.sendMessage(channelId, content, keyHex);
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

  createChannel: async (name: string) => {
    const result = await tauri.createChannel(name);
    set((state) => ({
      channels: [
        { id: result.id, name: result.name, createdAt: new Date().toISOString() },
        ...state.channels,
      ],
      channelKeys: { ...state.channelKeys, [result.id]: result.channelKeyHex },
      activeChannelId: result.id,
    }));
  },

  setChannelKey: (channelId: string, keyHex: string) => {
    set((state) => ({
      channelKeys: { ...state.channelKeys, [channelId]: keyHex },
    }));
  },
}));
