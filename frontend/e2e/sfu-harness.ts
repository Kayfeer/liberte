import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import { SfuSession, type SfuConnection } from "../src/lib/sfuSession";

let session: SfuSession | null = null;
const tracks = new Set<MediaStreamTrack>();
const errors: string[] = [];
const videos = new Map<string, HTMLVideoElement>();

const harness = {
  async start(connection: SfuConnection, userId: string) {
    localStorage.setItem("liberte-media-settings", JSON.stringify({ iceServers: [] }));
    mockIPC((command) => {
      if (command === "get_sfu_connection") return { ...connection };
      throw new Error("Unexpected IPC: " + command);
    });
    session = new SfuSession({
      userId, channelId: connection.roomName, sendSignal: async () => {}, announce: async () => {},
      onLocalStream(stream) { stream?.getTracks().forEach((track) => tracks.add(track)); },
      onRemoteStream(id, stream) {
        let video = videos.get(id);
        if (!video && stream) {
          video = document.createElement("video");
          video.autoplay = true;
          video.playsInline = true;
          // Muting playback avoids acoustic feedback; decoding still happens.
          video.muted = true;
          document.querySelector("#streams")!.append(video);
          videos.set(id, video);
        }
        if (video) {
          video.srcObject = stream;
          if (stream) void video.play().catch(() => {});
        }
      },
      onPeer() {}, onError(message) { errors.push(message); },
      onCameraEnded() {}, onClosed(message) { errors.push(message); },
    });
    await session.start();
  },
  async camera(enabled: boolean) { await session!.setVideoEnabled(enabled); },
  async audioDecoded() {
    const reports = await session!.getStats();
    return reports.some((report) => [...report.values()].some((entry) =>
      entry.type === "inbound-rtp" && entry.kind === "audio" && entry.totalSamplesReceived > 0));
  },
  snapshot() {
    return {
      errors: [...errors],
      frames: [...videos.values()].map((video) => ({
        width: video.videoWidth, decoded: video.getVideoPlaybackQuality().totalVideoFrames,
      })),
      localEnded: [...tracks].every((track) => track.readyState === "ended"),
    };
  },
  async stop() { await session?.stop(); clearMocks(); },
};
Object.assign(window, { sfuHarness: harness });

