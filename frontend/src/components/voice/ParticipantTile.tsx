import { Mic, MicOff } from "lucide-react";
import type { PeerInfo } from "../../lib/types";

interface Props {
  participant: PeerInfo;
}

export default function ParticipantTile({ participant }: Props) {
  const shortId = participant.userId.slice(0, 8);

  return (
    <div className="flex items-center gap-2 bg-liberte-bg rounded-lg px-3 py-2">
      <div
        className="w-6 h-6 rounded-full flex items-center justify-center text-xs font-bold"
        style={{
          backgroundColor: `hsl(${hashCode(participant.userId) % 360}, 60%, 40%)`,
        }}
      >
        {shortId.slice(0, 2).toUpperCase()}
      </div>
      <span className="text-sm">{participant.displayName || shortId}</span>
      {participant.isMuted ? (
        <MicOff className="w-3 h-3 text-red-400" />
      ) : (
        <Mic className="w-3 h-3 text-liberte-success" />
      )}
      <div
        className={`w-2 h-2 rounded-full ${
          participant.state === "connected"
            ? "bg-liberte-success"
            : participant.state === "connecting"
              ? "bg-liberte-warning animate-pulse"
              : "bg-red-500"
        }`}
      />
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
