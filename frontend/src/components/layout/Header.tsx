import { Hash, Phone, PhoneOff } from "lucide-react";
import { useMessageStore } from "../../stores/messageStore";
import { useMediaStore } from "../../stores/mediaStore";
import ConnectionBadge from "../common/ConnectionBadge";

export default function Header() {
  const { channels, activeChannelId } = useMessageStore();
  const { inCall, startCall, endCall } = useMediaStore();

  const activeChannel = channels.find((c) => c.id === activeChannelId);

  return (
    <div className="h-12 bg-liberte-surface border-b border-liberte-border flex items-center justify-between px-4">
      <div className="flex items-center gap-2">
        {activeChannel ? (
          <>
            <Hash className="w-5 h-5 text-liberte-muted" />
            <span className="font-medium">{activeChannel.name}</span>
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
  );
}
