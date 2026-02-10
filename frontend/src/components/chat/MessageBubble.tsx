import { useState } from "react";
import type { Message } from "../../lib/types";
import ReactionBar from "./ReactionBar";
import ProfileModal from "../identity/ProfileModal";

interface Props {
  message: Message;
  onReactionChange?: () => void;
}

export default function MessageBubble({ message, onReactionChange }: Props) {
  const shortId = message.senderId.slice(0, 8);
  const displayLabel = message.senderDisplayName || shortId;
  const avatarLetters = message.senderDisplayName
    ? message.senderDisplayName.slice(0, 2).toUpperCase()
    : shortId.slice(0, 2).toUpperCase();
  const time = new Date(message.timestamp).toLocaleTimeString("fr-FR", {
    hour: "2-digit",
    minute: "2-digit",
  });
  const [showProfile, setShowProfile] = useState(false);

  return (
    <>
      <div className="group flex items-start gap-3 py-1 px-2 rounded hover:bg-liberte-surface/50 transition-colors">
        {/* Avatar — clickable for profile */}
        <button
          onClick={() => setShowProfile(true)}
          className="w-8 h-8 rounded-full flex-shrink-0 flex items-center justify-center text-xs font-bold cursor-pointer hover:opacity-80 transition-opacity"
          style={{
            backgroundColor: `hsl(${hashCode(message.senderId) % 360}, 60%, 40%)`,
          }}
        >
          {avatarLetters}
        </button>

        <div className="min-w-0 flex-1">
          <div className="flex items-baseline gap-2">
            <button
              onClick={() => setShowProfile(true)}
              className="text-sm font-medium hover:underline cursor-pointer"
              style={{
                color: `hsl(${hashCode(message.senderId) % 360}, 70%, 70%)`,
              }}
            >
              {displayLabel}
            </button>
            {message.senderDisplayName && (
              <span className="text-xs text-liberte-muted font-mono opacity-50">
                {shortId}
              </span>
            )}
            <span className="text-xs text-liberte-muted">{time}</span>
          </div>
          <p className="text-sm text-liberte-text break-words">
            {message.content}
          </p>

          {/* Reactions */}
          <ReactionBar
            messageId={message.id}
            channelId={message.channelId}
            reactions={message.reactions || []}
            onReactionChange={onReactionChange}
          />
        </div>
      </div>

      <ProfileModal
        isOpen={showProfile}
        onClose={() => setShowProfile(false)}
        userId={message.senderId}
        displayName={message.senderDisplayName}
      />
    </>
  );
}

function hashCode(str: string): number {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    hash = (hash << 5) - hash + str.charCodeAt(i);
    hash |= 0;
  }
  return Math.abs(hash);
}
