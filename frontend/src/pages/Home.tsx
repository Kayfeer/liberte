import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import { EVENTS } from "../lib/constants";
import { useMessageStore } from "../stores/messageStore";
import { useNetworkStore } from "../stores/networkStore";
import { useNavigationStore } from "../stores/navigationStore";
import { useBackupStore } from "../stores/backupStore";
import Sidebar from "../components/layout/Sidebar";
import Header from "../components/layout/Header";
import MainPanel from "../components/layout/MainPanel";
import UpdateBanner from "../components/common/UpdateBanner";
import Settings from "./Settings";

export default function Home() {
  const { loadChannels, loadMessages, channels } = useMessageStore();
  const { refreshPeers } = useNetworkStore();
  const currentPage = useNavigationStore((s) => s.currentPage);
  const { autoBackupEnabled, intervalMinutes, runAutoBackup } = useBackupStore();
  const backupTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const notifPermRef = useRef<boolean>(false);

  // Request notification permission once
  useEffect(() => {
    (async () => {
      let granted = await isPermissionGranted();
      if (!granted) {
        const result = await requestPermission();
        granted = result === "granted";
      }
      notifPermRef.current = granted;
    })();
  }, []);

  useEffect(() => {
    loadChannels();
    refreshPeers();

    // Listen for real-time events from Tauri backend
    const unlisten: (() => void)[] = [];

    listen<{ channelId: string; senderShort?: string }>(
      EVENTS.NEW_MESSAGE,
      (event) => {
        const channelId = event.payload.channelId;
        if (channelId) {
          loadMessages(channelId);
        }

        // Send OS notification when window is not focused
        if (notifPermRef.current && document.hidden) {
          const ch = channels.find((c) => c.id === channelId);
          const channelName = ch?.name ?? "Liberté";
          sendNotification({
            title: `Nouveau message — #${channelName}`,
            body: event.payload.senderShort
              ? `De ${event.payload.senderShort}`
              : "Vous avez reçu un nouveau message",
          });
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
  }, [loadChannels, loadMessages, refreshPeers, channels]);

  // Auto-backup timer
  useEffect(() => {
    if (backupTimerRef.current) {
      clearInterval(backupTimerRef.current);
      backupTimerRef.current = null;
    }

    if (autoBackupEnabled && intervalMinutes > 0) {
      backupTimerRef.current = setInterval(
        () => { runAutoBackup(); },
        intervalMinutes * 60 * 1000,
      );
    }

    return () => {
      if (backupTimerRef.current) {
        clearInterval(backupTimerRef.current);
      }
    };
  }, [autoBackupEnabled, intervalMinutes, runAutoBackup]);

  return (
    <div className="flex h-screen bg-liberte-bg overflow-hidden">
      <Sidebar />
      <div className="flex flex-col flex-1 min-w-0">
        <UpdateBanner />
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
