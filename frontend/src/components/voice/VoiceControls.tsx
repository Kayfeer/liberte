import { Mic, MicOff, Video, VideoOff, PhoneOff } from "lucide-react";
import { useMediaStore } from "../../stores/mediaStore";

export default function VoiceControls() {
  const { isMuted, isVideoEnabled, toggleMute, toggleVideo, endCall, busy } = useMediaStore();
  return (
    <div className="flex items-center justify-center gap-3">
      <button disabled={busy} aria-label={isMuted ? "Activer le microphone" : "Couper le microphone"} aria-pressed={isMuted} title={isMuted ? "Activer le microphone" : "Couper le microphone"}
        onClick={() => { void toggleMute().catch(() => {}); }} className={`p-3 rounded-full transition-colors disabled:opacity-50 ${isMuted ? "bg-red-600 text-white" : "bg-liberte-panel text-liberte-text hover:bg-liberte-bg"}`}>
        {isMuted ? <MicOff className="w-5 h-5" /> : <Mic className="w-5 h-5" />}
      </button>
      <button disabled={busy} aria-label={isVideoEnabled ? "Désactiver la caméra" : "Activer la caméra"} aria-pressed={isVideoEnabled} title={isVideoEnabled ? "Désactiver la caméra" : "Activer la caméra"}
        onClick={() => { void toggleVideo().catch(() => {}); }} className={`p-3 rounded-full transition-colors disabled:opacity-50 ${isVideoEnabled ? "bg-liberte-panel text-liberte-text" : "bg-liberte-panel text-liberte-muted"}`}>
        {isVideoEnabled ? <Video className="w-5 h-5" /> : <VideoOff className="w-5 h-5" />}
      </button>
      <button aria-label="Quitter l'appel" title="Quitter l'appel" onClick={() => { void endCall().catch(() => {}); }} className="p-3 rounded-full bg-red-600 text-white hover:bg-red-700 transition-colors">
        <PhoneOff className="w-5 h-5" />
      </button>
    </div>
  );
}
