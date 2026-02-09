import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { EVENTS } from "../lib/constants";
import { useMessageStore } from "../stores/messageStore";
import { useNetworkStore } from "../stores/networkStore";
import Sidebar from "../components/layout/Sidebar";
import Header from "../components/layout/Header";
import MainPanel from "../components/layout/MainPanel";

export default function Home() {
  const { loadChannels, addMessage } = useMessageStore();
  const { refreshPeers, setPeers } = useNetworkStore();

  useEffect(() => {
    loadChannels();
    refreshPeers();

    // Listen for real-time events from Tauri backend
    const unlisten: (() => void)[] = [];

    listen<{ message: import("../lib/types").Message }>(
      EVENTS.NEW_MESSAGE,
      (event) => {
        addMessage(event.payload.message);
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
  }, [loadChannels, addMessage, refreshPeers, setPeers]);

  return (
    <div className="flex h-screen bg-liberte-bg overflow-hidden">
      <Sidebar />
      <div className="flex flex-col flex-1 min-w-0">
        <Header />
        <MainPanel />
      </div>
    </div>
  );
}
