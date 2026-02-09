import { useMediaStore } from "../../stores/mediaStore";
import VoiceControls from "./VoiceControls";
import ParticipantTile from "./ParticipantTile";

export default function VoicePanel() {
  const { participants, mode, setMode } = useMediaStore();

  return (
    <div className="bg-liberte-surface border-b border-liberte-border p-4">
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <div className="w-2 h-2 rounded-full bg-liberte-success animate-pulse" />
          <span className="text-sm font-medium">Appel en cours</span>
          <div className="flex bg-liberte-bg rounded-full p-0.5 text-xs">
            <button
              onClick={() => setMode("mesh")}
              className={`px-2 py-0.5 rounded-full transition-colors ${
                mode === "mesh"
                  ? "bg-liberte-panel text-liberte-text"
                  : "text-liberte-muted hover:text-liberte-text"
              }`}
            >
              P2P
            </button>
            <button
              onClick={() => setMode("sfu")}
              className={`px-2 py-0.5 rounded-full transition-colors ${
                mode === "sfu"
                  ? "bg-liberte-panel text-liberte-text"
                  : "text-liberte-muted hover:text-liberte-text"
              }`}
            >
              SFU
            </button>
          </div>
        </div>
        <span className="text-xs text-liberte-muted">
          {participants.length + 1} participant
          {participants.length > 0 ? "s" : ""}
        </span>
      </div>

      <div className="flex flex-wrap gap-2 mb-3">
        {participants.map((p) => (
          <ParticipantTile key={p.userId} participant={p} />
        ))}
      </div>

      <VoiceControls />
    </div>
  );
}
