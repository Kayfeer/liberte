import { useEffect, useRef, useState } from "react";
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
import { EVENTS } from "../lib/constants";
import { createEventSubscriptions } from "../lib/eventSubscriptions";
import type { FileTransferDto } from "../lib/tauri";
import { useMessageStore } from "../stores/messageStore";
import { useNetworkStore } from "../stores/networkStore";
import { useNavigationStore } from "../stores/navigationStore";
import { useIdentityStore } from "../stores/identityStore";
import { useBackupStore } from "../stores/backupStore";
import { useMediaStore } from "../stores/mediaStore";
import { useFileStore } from "../stores/fileStore";
import { useProfileStore, type PeerProfile } from "../stores/profileStore";
import Sidebar from "../components/layout/Sidebar";
import Header from "../components/layout/Header";
import MainPanel from "../components/layout/MainPanel";
import VoicePanel from "../components/voice/VoicePanel";
import UpdateBanner from "../components/common/UpdateBanner";
import Settings from "./Settings";

export default function Home() {
  const currentPage = useNavigationStore((state) => state.currentPage);
  const channelError = useMessageStore((state) => state.error);
  const networkError = useNetworkStore((state) => state.error);
  const identityError = useIdentityStore((state) => state.error);
  const inCall = useMediaStore((state) => state.inCall);
  const mediaError = useMediaStore((state) => state.error);
  const autoBackupEnabled = useBackupStore((state) => state.autoBackupEnabled);
  const intervalMinutes = useBackupStore((state) => state.intervalMinutes);
  const [eventError, setEventError] = useState<string | null>(null);
  const notifPermRef = useRef(false);

  useEffect(() => {
    let disposed = false;
    void (async () => {
      try {
        const granted = await isPermissionGranted() || await requestPermission() === "granted";
        if (!disposed) notifPermRef.current = granted;
      } catch {
        // Notifications are optional; messaging remains available without permission.
      }
    })();
    return () => { disposed = true; };
  }, []);

  useEffect(() => {
    let disposed = false;
    const subscriptions = createEventSubscriptions((error) => setEventError(String(error)));
    subscriptions.add<{ channelId: string; sender?: string; senderDisplayName?: string }>(EVENTS.NEW_MESSAGE, (event) => {
      const { channelId, sender, senderDisplayName } = event.payload;
      if (channelId) void useMessageStore.getState().loadMessages(channelId);
      const identity = useIdentityStore.getState().identity;
      if (notifPermRef.current && document.hidden && identity?.status !== "dnd" && sender !== identity?.publicKey) {
        const channel = useMessageStore.getState().channels.find((item) => item.id === channelId);
        sendNotification({
          title: `${senderDisplayName || sender?.slice(0, 8) || "Quelqu'un"} : #${channel?.name || "Liberté"}`,
          body: "Nouveau message chiffré",
        });
      }
    });
    const refreshNetwork = () => { void useNetworkStore.getState().refreshNetwork(); };
    subscriptions.add(EVENTS.PEER_CONNECTED, refreshNetwork);
    subscriptions.add(EVENTS.PEER_DISCONNECTED, refreshNetwork);
    subscriptions.add(EVENTS.CONNECTION_MODE_CHANGED, refreshNetwork);
    subscriptions.add<{ channelId: string }>(EVENTS.MESSAGE_REACTION, (event) => {
      if (event.payload.channelId) void useMessageStore.getState().loadMessages(event.payload.channelId);
    });
    subscriptions.add<FileTransferDto>("file-transfer-updated", (event) => useFileStore.getState().upsert(event.payload));
    subscriptions.add<PeerProfile>("profile-updated", (event) => useProfileStore.getState().upsert(event.payload));
    subscriptions.add<{ userId: string; status: string }>(EVENTS.STATUS_CHANGED, (event) => useProfileStore.getState().setStatus(event.payload.userId, event.payload.status));

    void useMessageStore.getState().loadChannels();
    refreshNetwork();
    void useMediaStore.getState().setupListeners().catch((error) => { if (!disposed) setEventError(String(error)); });
    const poll = setInterval(refreshNetwork, 5000);
    return () => {
      disposed = true;
      subscriptions.dispose();
      clearInterval(poll);
      useMediaStore.getState().cleanup();
    };
  }, []);

  useEffect(() => {
    if (!autoBackupEnabled || intervalMinutes <= 0) return;
    const timer = setInterval(() => {
      void useBackupStore.getState().runAutoBackup();
    }, intervalMinutes * 60000);
    return () => clearInterval(timer);
  }, [autoBackupEnabled, intervalMinutes]);

  const error = eventError || channelError || networkError || identityError || (!inCall ? mediaError : null);
  return (
    <div className="flex h-screen bg-liberte-bg overflow-hidden">
      <Sidebar />
      <div className="flex flex-col flex-1 min-w-0">
        <UpdateBanner />
        {error && <div role="alert" className="bg-red-900/20 text-red-300 px-4 py-2 text-sm">{error}</div>}
        {inCall && <VoicePanel />}
        {currentPage === "settings" ? <Settings /> : <><Header /><MainPanel /></>}
      </div>
    </div>
  );
}
