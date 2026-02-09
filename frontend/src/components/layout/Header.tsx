import { useState, useRef } from "react";
import { Hash, Phone, PhoneOff, Link, Search, X } from "lucide-react";
import { useMessageStore } from "../../stores/messageStore";
import { useMediaStore } from "../../stores/mediaStore";
import ConnectionBadge from "../common/ConnectionBadge";
import InviteModal from "../channels/InviteModal";
import * as tauri from "../../lib/tauri";
import type { Message } from "../../lib/types";

export default function Header() {
  const { channels, activeChannelId, channelKeys } = useMessageStore();
  const { inCall, startCall, endCall } = useMediaStore();
  const [showInvite, setShowInvite] = useState(false);
  const [showSearch, setShowSearch] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<Message[]>([]);
  const [searching, setSearching] = useState(false);
  const searchTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const activeChannel = channels.find((c) => c.id === activeChannelId);
  const activeChannelKey = activeChannelId ? channelKeys[activeChannelId] : undefined;

  const handleSearch = (q: string) => {
    setSearchQuery(q);
    if (searchTimer.current) clearTimeout(searchTimer.current);
    if (q.trim().length < 2) {
      setSearchResults([]);
      return;
    }
    searchTimer.current = setTimeout(async () => {
      setSearching(true);
      try {
        const results = await tauri.searchMessages(q, activeChannelId ?? undefined);
        setSearchResults(results);
      } catch {
        setSearchResults([]);
      }
      setSearching(false);
    }, 300);
  };

  const closeSearch = () => {
    setShowSearch(false);
    setSearchQuery("");
    setSearchResults([]);
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

      {/* Search panel */}
      {showSearch && (
        <div className="bg-liberte-surface border-b border-liberte-border px-4 py-2 space-y-2">
          <div className="flex items-center gap-2">
            <Search className="w-4 h-4 text-liberte-muted flex-shrink-0" />
            <input
              autoFocus
              type="text"
              placeholder="Rechercher dans les messages…"
              value={searchQuery}
              onChange={(e) => handleSearch(e.target.value)}
              className="flex-1 bg-transparent text-sm focus:outline-none placeholder-liberte-muted"
            />
            {searching && (
              <span className="text-xs text-liberte-muted">…</span>
            )}
            <button onClick={closeSearch} className="p-1 hover:bg-liberte-panel rounded">
              <X className="w-3.5 h-3.5 text-liberte-muted" />
            </button>
          </div>

          {searchResults.length > 0 && (
            <div className="max-h-60 overflow-y-auto space-y-1">
              {searchResults.map((msg) => {
                const ch = channels.find((c) => c.id === msg.channelId);
                return (
                  <div
                    key={msg.id}
                    className="text-xs p-2 bg-liberte-bg rounded cursor-pointer hover:bg-liberte-panel transition-colors"
                    onClick={() => {
                      useMessageStore.getState().setActiveChannel(msg.channelId);
                      closeSearch();
                    }}
                  >
                    <div className="flex items-center gap-2 text-liberte-muted mb-0.5">
                      <span className="font-mono">
                        {msg.senderId.slice(0, 8)}…
                      </span>
                      {ch && (
                        <span className="text-liberte-accent">#{ch.name}</span>
                      )}
                      <span className="ml-auto">
                        {new Date(msg.timestamp).toLocaleString()}
                      </span>
                    </div>
                    <p className="text-liberte-text truncate">{msg.content}</p>
                  </div>
                );
              })}
            </div>
          )}

          {searchQuery.length >= 2 && !searching && searchResults.length === 0 && (
            <p className="text-xs text-liberte-muted py-1">Aucun résultat</p>
          )}
        </div>
      )}

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
