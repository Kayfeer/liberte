import { useEffect, useRef, useState } from "react";
import { Mic, MicOff, Play } from "lucide-react";
import type { PeerInfo } from "../../lib/types";
import { configurePlayback, releasePlayback } from "../../lib/mediaSession";
import { useMediaStore } from "../../stores/mediaStore";

interface Props { participant: PeerInfo; stream?: MediaStream | null; local?: boolean }

export default function ParticipantTile({ participant, stream, local = false }: Props) {
  const video = useRef<HTMLVideoElement>(null);
  const [needsPlayback, setNeedsPlayback] = useState(false);
  const setError = useMediaStore((state) => state.setError);
  useEffect(() => {
    const element = video.current;
    if (!element || !stream) return;
    let disposed = false;
    element.srcObject = stream;
    void configurePlayback(element, local)
      .then(() => element.play())
      .then(() => { if (!disposed) setNeedsPlayback(false); })
      .catch((error) => {
        if (disposed) return;
        setNeedsPlayback(true);
        if (!(error instanceof DOMException && error.name === "NotAllowedError")) setError(String(error));
      });
    return () => { disposed = true; releasePlayback(element); };
  }, [stream, local, setError]);

  const stateLabel = { connecting: "Connexion", connected: "Connecté", disconnected: "Déconnecté", failed: "Connexion échouée" }[participant.state];
  return (
    <div className="relative overflow-hidden bg-liberte-bg rounded-lg min-w-40 flex-1 max-w-sm border border-liberte-border">
      <div className="relative aspect-video flex items-center justify-center bg-black/30">
        <video ref={video} autoPlay playsInline muted={local} className={`absolute inset-0 w-full h-full object-cover ${local ? "-scale-x-100" : ""} ${participant.isVideoEnabled ? "opacity-100" : "opacity-0"}`} />
        {!participant.isVideoEnabled && <span className="text-2xl text-liberte-muted" aria-label="Caméra désactivée">{(participant.displayName || participant.userId).slice(0, 2).toUpperCase()}</span>}
        {needsPlayback && !local && (
          <button className="absolute inset-0 bg-black/60 flex items-center justify-center gap-2 text-sm" onClick={() => {
            void video.current?.play().then(() => setNeedsPlayback(false)).catch((error) => setError(String(error)));
          }}><Play className="w-4 h-4" /> Activer le son</button>
        )}
      </div>
      <div className="flex items-center gap-2 px-3 py-2 text-xs">
        <span className="truncate flex-1">{participant.displayName || participant.userId.slice(0, 8)}{local ? " (vous)" : ""}</span>
        {participant.isMuted ? <MicOff className="w-3 h-3 text-red-400" aria-label="Micro coupé" /> : <Mic className="w-3 h-3 text-liberte-success" aria-label="Micro activé" />}
        <span title={stateLabel} aria-label={stateLabel} className={`w-2 h-2 rounded-full ${participant.state === "connected" ? "bg-liberte-success" : participant.state === "connecting" ? "bg-liberte-warning animate-pulse" : "bg-red-500"}`} />
      </div>
    </div>
  );
}
