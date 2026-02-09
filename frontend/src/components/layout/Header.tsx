import { useState } from "react";
import { Hash, Phone, PhoneOff, Link } from "lucide-react";
import { useMessageStore } from "../../stores/messageStore";
import { useMediaStore } from "../../stores/mediaStore";
import ConnectionBadge from "../common/ConnectionBadge";
import InviteModal from "../channels/InviteModal";

export default function Header() {
  const { channels, activeChannelId, channelKeys } = useMessageStore();
  const { inCall, startCall, endCall } = useMediaStore();
  const [showInvite, setShowInvite] = useState(false);

  const activeChannel = channels.find((c) => c.id === activeChannelId);
  const activeChannelKey = activeChannelId ? channelKeys[activeChannelId] : undefined;

  return (
    <>
      <div className="h-12 bg-liberte-surface border-b border-liberte-border flex items-center justify-between px-4">
        <div className="flex items-center gap-2">
          {activeChannel ? (
            <>
              <Hash className="w-5 h-5 text-liberte-muted" />
              <span className="font-medium">{activeChannel.name}</span>
              {activeChannelKey && (
                <button
                  onClick={() => setShowInvite(true)}
                  className="p-1.5 hover:bg-liberte-panel rounded transition-colors"
                  title="Inviter dans ce canal"
                >
                  <Link className="w-4 h-4 text-liberte-muted" />
                </button>
              )}
            </>
          ) : (
            <span className="text-liberte-muted">
              Sélectionnez un canal
            </span>
          )}
        </div>

        <div className="flex items-center gap-3">
          <ConnectionBadge />

          {activeChannelId && (
            <button
              onClick={() =>
                inCall ? endCall() : startCall(activeChannelId)
              }
              className={`p-2 rounded-lg transition-colors ${
                inCall
                  ? "bg-red-600 hover:bg-red-700 text-white"
                  : "hover:bg-liberte-panel text-liberte-muted hover:text-liberte-text"
              }`}
            >
              {inCall ? (
                <PhoneOff className="w-4 h-4" />
              ) : (
                <Phone className="w-4 h-4" />
              )}
            </button>
          )}
        </div>
      </div>

      {activeChannel && activeChannelKey && (
        <InviteModal
          isOpen={showInvite}
          onClose={() => setShowInvite(false)}
          channelId={activeChannel.id}
          channelName={activeChannel.name}
          channelKeyHex={activeChannelKey}
        />
      )}
    </>
  );
}
