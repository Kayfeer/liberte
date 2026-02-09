import { useState } from "react";
import { Hash, Plus, Settings, Users, UserPlus } from "lucide-react";
import { useMessageStore } from "../../stores/messageStore";
import { useNetworkStore } from "../../stores/networkStore";
import { useNavigationStore } from "../../stores/navigationStore";
import CreateChannelModal from "../channels/CreateChannelModal";
import JoinChannelModal from "../channels/JoinChannelModal";

export default function Sidebar() {
  const { channels, activeChannelId, setActiveChannel } = useMessageStore();
  const { peers } = useNetworkStore();
  const { currentPage, navigate } = useNavigationStore();
  const [showCreateChannel, setShowCreateChannel] = useState(false);
  const [showJoinChannel, setShowJoinChannel] = useState(false);

  return (
    <div className="w-60 bg-liberte-surface flex flex-col border-r border-liberte-border">
      {/* Header */}
      <div className="p-4 border-b border-liberte-border">
        <div className="flex items-center gap-2">
          <img src="/logo.png" alt="Liberté" className="w-8 h-8 rounded-lg" />
          <h1 className="text-lg font-bold text-liberte-accent">Liberté</h1>
        </div>
        <div className="flex items-center gap-1 mt-1">
          <Users className="w-3 h-3 text-liberte-muted" />
          <span className="text-xs text-liberte-muted">
            {peers.length} pair{peers.length !== 1 ? "s" : ""} connecté
            {peers.length !== 1 ? "s" : ""}
          </span>
        </div>
      </div>

      {/* Channels */}
      <div className="flex-1 overflow-y-auto p-2">
        <div className="flex items-center justify-between px-2 mb-2">
          <span className="text-xs font-semibold text-liberte-muted uppercase tracking-wider">
            Canaux
          </span>
          <button
            onClick={() => setShowCreateChannel(true)}
            className="p-1 hover:bg-liberte-panel rounded transition-colors"
            title="Créer un canal"
          >
            <Plus className="w-3 h-3 text-liberte-muted" />
          </button>
        </div>

        {channels.map((channel) => (
          <button
            key={channel.id}
            onClick={() => setActiveChannel(channel.id)}
            className={`w-full flex items-center gap-2 px-2 py-1.5 rounded text-sm transition-colors ${
              activeChannelId === channel.id
                ? "bg-liberte-panel text-liberte-text"
                : "text-liberte-muted hover:text-liberte-text hover:bg-liberte-bg"
            }`}
          >
            <Hash className="w-4 h-4 flex-shrink-0" />
            <span className="truncate">{channel.name}</span>
          </button>
        ))}

        {channels.length === 0 && (
          <p className="text-xs text-liberte-muted px-2 py-4 text-center">
            Aucun canal. Créez-en un ou rejoignez une invitation.
          </p>
        )}
      </div>

      {/* Bottom bar */}
      <div className="p-2 border-t border-liberte-border space-y-1">
        <button
          onClick={() => setShowJoinChannel(true)}
          className="w-full flex items-center gap-2 px-2 py-1.5 rounded text-sm text-liberte-muted hover:text-liberte-text hover:bg-liberte-bg transition-colors"
        >
          <UserPlus className="w-4 h-4" />
          <span>Rejoindre un canal</span>
        </button>
        <button
          onClick={() => navigate(currentPage === "settings" ? "home" : "settings")}
          className={`w-full flex items-center gap-2 px-2 py-1.5 rounded text-sm transition-colors ${
            currentPage === "settings"
              ? "bg-liberte-panel text-liberte-text"
              : "text-liberte-muted hover:text-liberte-text hover:bg-liberte-bg"
          }`}
        >
          <Settings className="w-4 h-4" />
          <span>Paramètres</span>
        </button>
      </div>

      <CreateChannelModal
        isOpen={showCreateChannel}
        onClose={() => setShowCreateChannel(false)}
      />
      <JoinChannelModal
        isOpen={showJoinChannel}
        onClose={() => setShowJoinChannel(false)}
      />
    </div>
  );
}
