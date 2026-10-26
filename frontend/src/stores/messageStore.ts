import { create } from "zustand";
import type { Message, Channel } from "@/lib/types";
import { MESSAGE_PAGE_SIZE } from "@/lib/constants";
import * as tauri from "@/lib/tauri";

interface MessageState {
  channels: Channel[];
  activeChannelId: string | null;
  messages: Record<string, Message[]>;
  channelKeys: Record<string, string>;
  loadingByChannel: Record<string, boolean>;
  loadingMore: Record<string, boolean>;
  hasMore: Record<string, boolean>;
  errors: Record<string, string | null>;
  error: string | null;
  focusedMessageId: string | null;
  revealMessage: (channelId: string, messageId: string) => Promise<boolean>;
  refreshReactions: (channelId: string, messageId: string) => Promise<void>;
  loadChannels: () => Promise<void>;
  setActiveChannel: (channelId: string) => void;
  loadMessages: (channelId: string) => Promise<void>;
  loadMoreMessages: (channelId: string) => Promise<void>;
  sendMessage: (channelId: string, content: string) => Promise<void>;
  addMessage: (message: Message) => void;
  createChannel: (name: string) => Promise<void>;
  setChannelKey: (channelId: string, keyHex: string) => void;
}

function mergeMessages(existing: Message[], incoming: Message[]) {
  const byId = new Map(existing.map((message) => [message.id, message]));
  for (const message of incoming) byId.set(message.id, message);
  return [...byId.values()].sort((a, b) =>
    a.timestamp.localeCompare(b.timestamp) || a.id.localeCompare(b.id));
}

let channelsRequest: Promise<void> | null = null;
const refreshVersions = new Map<string, number>();
const reactionVersions = new Map<string, number>();
const historyRequests = new Map<string, Promise<void>>();
let focusGeneration = 0;

export const useMessageStore = create<MessageState>((set, get) => ({
  channels: [], activeChannelId: null, messages: {}, channelKeys: {},
  loadingByChannel: {}, loadingMore: {}, hasMore: {}, errors: {}, error: null, focusedMessageId: null,

  loadChannels: () => {
    if (channelsRequest) return channelsRequest;
    channelsRequest = (async () => {
      try {
        const [channels, channelKeys] = await Promise.all([
          tauri.listChannels(), tauri.getAllChannelKeys(),
        ]);
        set({ channels, channelKeys, error: null });
      } catch (error) {
        set({ error: `Impossible de charger les canaux : ${String(error)}` });
      } finally {
        channelsRequest = null;
      }
    })();
    return channelsRequest;
  },

  setActiveChannel: (channelId) => {
    ++focusGeneration;
    set({ activeChannelId: channelId, focusedMessageId: null });
    void get().loadMessages(channelId);
  },

  loadMessages: async (channelId) => {
    const version = (refreshVersions.get(channelId) ?? 0) + 1;
    refreshVersions.set(channelId, version);
    set((state) => ({
      loadingByChannel: { ...state.loadingByChannel, [channelId]: true },
      errors: { ...state.errors, [channelId]: null },
    }));
    try {
      const messages = await tauri.getMessages(channelId, get().channelKeys[channelId] || "", MESSAGE_PAGE_SIZE, 0);
      if (refreshVersions.get(channelId) !== version) return;
      set((state) => ({
        messages: { ...state.messages, [channelId]: mergeMessages(state.messages[channelId] || [], messages) },
        hasMore: { ...state.hasMore, [channelId]: state.hasMore[channelId] ?? messages.length === MESSAGE_PAGE_SIZE },
        loadingByChannel: { ...state.loadingByChannel, [channelId]: false },
      }));
    } catch (error) {
      if (refreshVersions.get(channelId) !== version) return;
      set((state) => ({
        loadingByChannel: { ...state.loadingByChannel, [channelId]: false },
        errors: { ...state.errors, [channelId]: String(error) },
      }));
    }
  },

  loadMoreMessages: (channelId) => {
    const pending = historyRequests.get(channelId);
    if (pending) return pending;
    if (get().hasMore[channelId] === false) return Promise.resolve();
    const offset = get().messages[channelId]?.length || 0;
    set((state) => ({ loadingMore: { ...state.loadingMore, [channelId]: true } }));
    const request = (async () => {
      try {
        const messages = await tauri.getMessages(channelId, get().channelKeys[channelId] || "", MESSAGE_PAGE_SIZE, offset);
        set((state) => ({
          messages: { ...state.messages, [channelId]: mergeMessages(state.messages[channelId] || [], messages) },
          hasMore: { ...state.hasMore, [channelId]: messages.length === MESSAGE_PAGE_SIZE },
          errors: { ...state.errors, [channelId]: null },
        }));
      } catch (error) {
        set((state) => ({ errors: { ...state.errors, [channelId]: String(error) } }));
      } finally {
        set((state) => ({ loadingMore: { ...state.loadingMore, [channelId]: false } }));
      }
    })();
    historyRequests.set(channelId, request);
    void request.finally(() => { if (historyRequests.get(channelId) === request) historyRequests.delete(channelId); });
    return request;
  },

  refreshReactions: async (channelId, messageId) => {
    if (!get().messages[channelId]?.some((message) => message.id === messageId)) return;
    const version = (reactionVersions.get(messageId) ?? 0) + 1;
    reactionVersions.set(messageId, version);
    try {
      const reactions = await tauri.getReactions(messageId);
      if (reactionVersions.get(messageId) !== version) return;
      set((state) => ({ messages: { ...state.messages,
        [channelId]: (state.messages[channelId] || []).map((message) => message.id === messageId ? { ...message, reactions } : message),
      } }));
    } catch (error) {
      if (reactionVersions.get(messageId) !== version) return;
      set((state) => ({ errors: { ...state.errors, [channelId]: String(error) } }));
    }
  },

  revealMessage: async (channelId, messageId) => {
    const generation = ++focusGeneration;
    set({ activeChannelId: channelId, focusedMessageId: null });
    if (!get().messages[channelId]?.some((message) => message.id === messageId)) await get().loadMessages(channelId);
    while (!get().messages[channelId]?.some((message) => message.id === messageId)) {
      if (generation !== focusGeneration) return false;
      if (get().errors[channelId]) throw new Error(get().errors[channelId]!);
      if (get().hasMore[channelId] === false) throw new Error("Le message recherché n'est plus disponible.");
      const previousCount = get().messages[channelId]?.length || 0;
      await get().loadMoreMessages(channelId);
      if (get().errors[channelId]) throw new Error(get().errors[channelId]!);
      if ((get().messages[channelId]?.length || 0) <= previousCount) {
        throw new Error("Impossible de retrouver ce message dans l'historique.");
      }
    }
    if (generation !== focusGeneration) return false;
    set({ focusedMessageId: messageId });
    return true;
  },

  sendMessage: async (channelId, content) => {
    await tauri.sendMessage(channelId, content, get().channelKeys[channelId] || "");
    await get().loadMessages(channelId);
  },

  addMessage: (message) => set((state) => ({
    messages: { ...state.messages, [message.channelId]: mergeMessages(state.messages[message.channelId] || [], [message]) },
  })),

  createChannel: async (name) => {
    const result = await tauri.createChannel(name);
    set((state) => ({
      channels: [{ id: result.id, name: result.name, createdAt: new Date().toISOString() }, ...state.channels],
      channelKeys: { ...state.channelKeys, [result.id]: result.channelKeyHex },
      activeChannelId: result.id,
    }));
  },

  setChannelKey: (channelId, keyHex) => set((state) => ({ channelKeys: { ...state.channelKeys, [channelId]: keyHex } })),
}));
