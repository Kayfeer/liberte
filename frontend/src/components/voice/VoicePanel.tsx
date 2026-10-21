import { useMediaStore } from "../../stores/mediaStore";
import { useIdentityStore } from "../../stores/identityStore";
import VoiceControls from "./VoiceControls";
import ParticipantTile from "./ParticipantTile";

export default function VoicePanel() {
  const { participants, localStream, remoteStreams, isMuted, isVideoEnabled, error, retryConnections } = useMediaStore();
  const identity = useIdentityStore((state) => state.identity);
  return (
    <section aria-label="Appel audio et vidéo" className="bg-liberte-surface border-b border-liberte-border p-4">
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <span className="w-2 h-2 rounded-full bg-liberte-success" />
          <span className="text-sm font-medium">Appel P2P</span>
          <span className="text-xs text-liberte-muted">Audio et vidéo chiffrés</span>
        </div>
        <span className="text-xs text-liberte-muted">{participants.length + 1}/8 participants</span>
      </div>
      {error && <div role="alert" className="mb-3 rounded-lg bg-red-500/10 p-3 text-sm text-red-300">
        {error}
        <button onClick={retryConnections} className="ml-2 underline">Reconnecter</button>
      </div>}
      <div className="flex flex-wrap gap-2 mb-3">
        <ParticipantTile local stream={localStream} participant={{ userId: identity?.publicKey ?? "local", displayName: identity?.displayName || "Vous", isMuted, isVideoEnabled, state: "connected" }} />
        {participants.map((participant) => <ParticipantTile key={participant.userId} participant={participant} stream={remoteStreams[participant.userId]} />)}
      </div>
      {participants.length === 0 && <p className="mb-3 text-xs text-liberte-muted text-center">En attente des autres membres du salon.</p>}
      <VoiceControls />
    </section>
  );
}
