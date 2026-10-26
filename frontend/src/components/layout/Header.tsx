import { useState, useRef } from "react";
import { Hash, Phone, PhoneOff, Link, Search } from "lucide-react";
import { useMessageStore } from "../../stores/messageStore";
import { useMediaStore } from "../../stores/mediaStore";
import ConnectionBadge from "../common/ConnectionBadge";
import InviteModal from "../channels/InviteModal";
import MessageSearch from "../chat/MessageSearch";

export default function Header() {
  const { channels, activeChannelId, channelKeys } = useMessageStore();
  const { inCall, startCall, endCall, mode, setMode, busy } = useMediaStore();
  const [showInvite, setShowInvite] = useState(false);
  const [showSearch, setShowSearch] = useState(false);
  const [callError, setCallError] = useState<string | null>(null);
  const [callPending, setCallPending] = useState(false);
  const [modePending, setModePending] = useState(false);
  const callGeneration = useRef(0);

  const activeChannel = channels.find((c) => c.id === activeChannelId);
  const activeChannelKey = activeChannelId ? channelKeys[activeChannelId] : undefined;

  const toggleCall = async () => {
    if (!activeChannelId || callPending) return;
    const generation = ++callGeneration.current;
    setCallPending(true);
    setCallError(null);
    try {
      if (inCall) await endCall();
      else await startCall(activeChannelId);
    } catch (error) { if (generation === callGeneration.current) setCallError(String(error)); }
    finally { if (generation === callGeneration.current) setCallPending(false); }
  };

  const cancelCall = async () => {
    ++callGeneration.current;
    try { await endCall(); }
    catch (error) { setCallError(String(error)); }
    finally { setCallPending(false); }
  };

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
          <button
            onClick={() => setShowSearch(!showSearch)}
            className="p-2 hover:bg-liberte-panel rounded-lg transition-colors text-liberte-muted hover:text-liberte-text"
            title="Rechercher"
          >
            <Search className="w-4 h-4" />
          </button>

          <ConnectionBadge />

          {activeChannelId && (
            <select
              aria-label="Transport de l'appel"
              title="Choisissez le transport avant de rejoindre l'appel"
              value={mode}
              disabled={inCall || busy || callPending || modePending}
              onChange={(event) => {
                setCallError(null);
                setModePending(true);
                void setMode(event.target.value as "mesh" | "sfu")
                  .catch((error) => setCallError(String(error)))
                  .finally(() => setModePending(false));
              }}
              className="bg-liberte-panel text-xs rounded px-2 py-1 disabled:opacity-60"
            >
              <option value="mesh">P2P direct</option>
              <option value="sfu">SFU chiffré</option>
            </select>
          )}

          {callPending && !inCall && <button onClick={() => void cancelCall()} className="text-xs text-red-300">Annuler la connexion</button>}

          {activeChannelId && (
            <button
              onClick={() => void toggleCall()}
              disabled={callPending || busy || modePending}
              aria-label={inCall ? "Quitter l'appel" : "Rejoindre l'appel"}
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

      {callError && <p role="alert" className="text-red-300 text-sm px-4 py-2">{callError}</p>}

      {showSearch && <MessageSearch channelId={activeChannelId} onClose={() => setShowSearch(false)} />}

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
