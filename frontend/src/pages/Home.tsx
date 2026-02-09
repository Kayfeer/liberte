import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { EVENTS } from "../lib/constants";
import { useMessageStore } from "../stores/messageStore";
import { useNetworkStore } from "../stores/networkStore";
import { useNavigationStore } from "../stores/navigationStore";
import Sidebar from "../components/layout/Sidebar";
import Header from "../components/layout/Header";
import MainPanel from "../components/layout/MainPanel";
import Settings from "./Settings";

export default function Home() {
  const { loadChannels, loadMessages } = useMessageStore();
  const { refreshPeers } = useNetworkStore();
  const currentPage = useNavigationStore((s) => s.currentPage);

  useEffect(() => {
    loadChannels();
    refreshPeers();

    // Listen for real-time events from Tauri backend
    const unlisten: (() => void)[] = [];

    listen<{ channelId: string }>(
      EVENTS.NEW_MESSAGE,
      (event) => {
        // Reload messages for the channel that received a new message
        const channelId = event.payload.channelId;
        if (channelId) {
          loadMessages(channelId);
        }
      }
    ).then((u) => unlisten.push(u));

    listen<{ peerId: string }>(EVENTS.PEER_CONNECTED, () => {
      refreshPeers();
    }).then((u) => unlisten.push(u));

    listen<{ peerId: string }>(EVENTS.PEER_DISCONNECTED, () => {
      refreshPeers();
    }).then((u) => unlisten.push(u));

    return () => {
      unlisten.forEach((u) => u());
    };
  }, [loadChannels, loadMessages, refreshPeers]);

  return (
    <div className="flex h-screen bg-liberte-bg overflow-hidden">
      <Sidebar />
      <div className="flex flex-col flex-1 min-w-0">
        {currentPage === "settings" ? (
          <Settings />
        ) : (
          <>
            <Header />
            <MainPanel />
          </>
        )}
      </div>
    </div>
  );
}
