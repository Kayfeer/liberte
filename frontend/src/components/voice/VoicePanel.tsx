import { useMediaStore } from "../../stores/mediaStore";
import VoiceControls from "./VoiceControls";
import ParticipantTile from "./ParticipantTile";

export default function VoicePanel() {
  const { participants, mode } = useMediaStore();

  return (
    <div className="bg-liberte-surface border-b border-liberte-border p-4">
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <div className="w-2 h-2 rounded-full bg-liberte-success animate-pulse" />
          <span className="text-sm font-medium">Appel en cours</span>
          <span className="text-xs text-liberte-muted px-2 py-0.5 bg-liberte-bg rounded-full">
            {mode === "mesh" ? "P2P Mesh" : "SFU Relayé"}
          </span>
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
