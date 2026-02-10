import { useState, useRef, useEffect } from "react";
import { SmilePlus } from "lucide-react";
import { QUICK_REACTIONS } from "../../lib/constants";
import type { ReactionGroup } from "../../lib/types";
import { useIdentityStore } from "../../stores/identityStore";
import * as tauri from "../../lib/tauri";

interface Props {
  messageId: string;
  channelId: string;
  reactions: ReactionGroup[];
  onReactionChange?: () => void;
}

export default function ReactionBar({
  messageId,
  channelId,
  reactions,
  onReactionChange,
}: Props) {
  const [showPicker, setShowPicker] = useState(false);
  const pickerRef = useRef<HTMLDivElement>(null);
  const identity = useIdentityStore((s) => s.identity);
  const myPubkey = identity?.publicKey || "";

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (pickerRef.current && !pickerRef.current.contains(e.target as Node)) {
        setShowPicker(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  const handleToggleReaction = async (emoji: string) => {
    const group = reactions.find((r) => r.emoji === emoji);
    const hasReacted = group?.users.includes(myPubkey);

    try {
      if (hasReacted) {
        await tauri.removeReaction(messageId, emoji);
      } else {
        await tauri.addReaction(channelId, messageId, emoji);
      }
      onReactionChange?.();
    } catch (e) {
      console.error("Failed to toggle reaction:", e);
    }
    setShowPicker(false);
  };

  return (
    <div className="flex items-center gap-1 flex-wrap mt-1">
      {reactions.map((r) => {
        const hasReacted = r.users.includes(myPubkey);
        return (
          <button
            key={r.emoji}
            onClick={() => handleToggleReaction(r.emoji)}
            className={`inline-flex items-center gap-1 px-1.5 py-0.5 rounded-full text-xs transition-colors border ${
              hasReacted
                ? "bg-liberte-accent/20 border-liberte-accent/50 text-liberte-accent"
                : "bg-liberte-bg border-liberte-border text-liberte-muted hover:bg-liberte-panel"
            }`}
            title={`${r.users.length} réaction${r.users.length !== 1 ? "s" : ""}`}
          >
            <span>{r.emoji}</span>
            <span className="font-medium">{r.users.length}</span>
          </button>
        );
      })}

      {/* Add reaction button */}
      <div className="relative" ref={pickerRef}>
        <button
          onClick={() => setShowPicker(!showPicker)}
          className="p-1 rounded hover:bg-liberte-panel text-liberte-muted hover:text-liberte-text transition-colors opacity-0 group-hover:opacity-100"
          title="Ajouter une réaction"
        >
          <SmilePlus className="w-4 h-4" />
        </button>

        {showPicker && (
          <div className="absolute bottom-full left-0 mb-1 bg-liberte-surface border border-liberte-border rounded-lg shadow-xl p-2 z-50">
            <div className="grid grid-cols-4 gap-1">
              {QUICK_REACTIONS.map((emoji) => (
                <button
                  key={emoji}
                  onClick={() => handleToggleReaction(emoji)}
                  className="w-8 h-8 flex items-center justify-center rounded hover:bg-liberte-panel text-lg transition-colors"
                >
                  {emoji}
                </button>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
