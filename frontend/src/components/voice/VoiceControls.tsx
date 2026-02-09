import { Mic, MicOff, Video, VideoOff, PhoneOff } from "lucide-react";
import { useMediaStore } from "../../stores/mediaStore";

export default function VoiceControls() {
  const { isMuted, isVideoEnabled, toggleMute, toggleVideo, endCall } =
    useMediaStore();

  return (
    <div className="flex items-center justify-center gap-3">
      <button
        onClick={toggleMute}
        className={`p-3 rounded-full transition-colors ${
          isMuted
            ? "bg-red-600 text-white"
            : "bg-liberte-panel text-liberte-text hover:bg-liberte-bg"
        }`}
      >
        {isMuted ? (
          <MicOff className="w-5 h-5" />
        ) : (
          <Mic className="w-5 h-5" />
        )}
      </button>

      <button
        onClick={toggleVideo}
        className={`p-3 rounded-full transition-colors ${
          !isVideoEnabled
            ? "bg-liberte-panel text-liberte-muted"
            : "bg-liberte-panel text-liberte-text hover:bg-liberte-bg"
        }`}
      >
        {isVideoEnabled ? (
          <Video className="w-5 h-5" />
        ) : (
          <VideoOff className="w-5 h-5" />
        )}
      </button>

      <button
        onClick={endCall}
        className="p-3 rounded-full bg-red-600 text-white hover:bg-red-700 transition-colors"
      >
        <PhoneOff className="w-5 h-5" />
      </button>
    </div>
  );
}
