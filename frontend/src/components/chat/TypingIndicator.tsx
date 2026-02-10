import { useEffect, useState, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { EVENTS, TYPING_TIMEOUT } from "../../lib/constants";
import type { TypingEvent } from "../../lib/types";

interface Props {
  channelId: string;
}

interface TypingUser {
  userId: string;
  displayName?: string;
  expiresAt: number;
}

export default function TypingIndicator({ channelId }: Props) {
  const [typingUsers, setTypingUsers] = useState<TypingUser[]>([]);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    const unlisten = listen<TypingEvent>(EVENTS.TYPING_INDICATOR, (event) => {
      const { userId, displayName, channelId: evtChannel } = event.payload;
      if (evtChannel !== channelId) return;

      setTypingUsers((prev) => {
        const filtered = prev.filter((u) => u.userId !== userId);
        return [
          ...filtered,
          { userId, displayName, expiresAt: Date.now() + TYPING_TIMEOUT },
        ];
      });
    });

    // Cleanup expired typing indicators every second
    timerRef.current = setInterval(() => {
      setTypingUsers((prev) => prev.filter((u) => u.expiresAt > Date.now()));
    }, 1000);

    return () => {
      unlisten.then((u) => u());
      if (timerRef.current) clearInterval(timerRef.current);
    };
  }, [channelId]);

  if (typingUsers.length === 0) return null;

  const names = typingUsers.map(
    (u) => u.displayName || `${u.userId.slice(0, 8)}…`
  );

  let text: string;
  if (names.length === 1) {
    text = `${names[0]} est en train d'écrire`;
  } else if (names.length === 2) {
    text = `${names[0]} et ${names[1]} sont en train d'écrire`;
  } else {
    text = `${names.length} personnes sont en train d'écrire`;
  }

  return (
    <div className="px-4 py-1 text-xs text-liberte-muted flex items-center gap-2">
      {/* Animated dots */}
      <span className="flex gap-0.5">
        <span className="w-1.5 h-1.5 bg-liberte-muted rounded-full animate-bounce" style={{ animationDelay: "0ms" }} />
        <span className="w-1.5 h-1.5 bg-liberte-muted rounded-full animate-bounce" style={{ animationDelay: "150ms" }} />
        <span className="w-1.5 h-1.5 bg-liberte-muted rounded-full animate-bounce" style={{ animationDelay: "300ms" }} />
      </span>
      <span>{text}…</span>
    </div>
  );
}
