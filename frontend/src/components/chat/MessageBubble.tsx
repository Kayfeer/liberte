import type { Message } from "../../lib/types";

interface Props {
  message: Message;
}

export default function MessageBubble({ message }: Props) {
  const shortId = message.senderId.slice(0, 8);
  const time = new Date(message.timestamp).toLocaleTimeString("fr-FR", {
    hour: "2-digit",
    minute: "2-digit",
  });

  return (
    <div className="group flex items-start gap-3 py-1 px-2 rounded hover:bg-liberte-surface/50 transition-colors">
      {/* Avatar placeholder */}
      <div
        className="w-8 h-8 rounded-full flex-shrink-0 flex items-center justify-center text-xs font-bold"
        style={{
          backgroundColor: `hsl(${hashCode(message.senderId) % 360}, 60%, 40%)`,
        }}
      >
        {shortId.slice(0, 2).toUpperCase()}
      </div>

      <div className="min-w-0 flex-1">
        <div className="flex items-baseline gap-2">
          <span
            className="text-sm font-medium"
            style={{
              color: `hsl(${hashCode(message.senderId) % 360}, 70%, 70%)`,
            }}
          >
            {shortId}
          </span>
          <span className="text-xs text-liberte-muted">{time}</span>
        </div>
        <p className="text-sm text-liberte-text break-words">
          {message.content}
        </p>
      </div>
    </div>
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
