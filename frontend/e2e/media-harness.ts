import { MediaSession, type CallSignal, type CallSignalEvent } from "../src/lib/mediaSession";
import type { PeerInfo } from "../src/lib/types";

declare global {
  interface Window {
    transport: (target: string, signal: CallSignal) => Promise<void>;
    call: ReturnType<typeof createCall>;
    initializeCall: (userId: string) => void;
  }
}

const channelId = "00000000-0000-4000-8000-000000000001";
function createCall(userId: string) {
  const peers = new Map<string, Partial<PeerInfo>>();
  const remote = new Map<string, MediaStream>();
  const localTracks = new Set<MediaStreamTrack>();
  const errors: string[] = [];
  let local: MediaStream | null = null;
  const session = new MediaSession({
    userId, channelId,
    sendSignal: (target, signal) => window.transport(target, signal),
    announce: async () => {},
    onLocalStream: (stream) => {
      local = stream;
      stream?.getTracks().forEach((track) => localTracks.add(track));
    },
    onRemoteStream: (id, stream) => {
      const elementId = `peer-${id}`;
      if (!stream) { remote.delete(id); document.getElementById(elementId)?.remove(); return; }
      remote.set(id, stream);
      let element = document.getElementById(elementId) as HTMLVideoElement | null;
      if (!element) {
        element = document.createElement("video");
        element.id = elementId;
        element.autoplay = true;
        element.playsInline = true;
        document.getElementById("media")!.appendChild(element);
      }
      element.srcObject = stream;
      void element.play().catch((error) => errors.push(String(error)));
    },
    onPeer: (id, patch) => { if (patch) peers.set(id, { ...peers.get(id), ...patch }); else peers.delete(id); },
    onError: (error) => errors.push(error),
    onCameraEnded: () => {},
  });
  return {
    session,
    receive: (sender: string, signalType: CallSignal) => session.receive({ sender, signalType, target: userId, channelId } satisfies CallSignalEvent),
    state: () => ({
      peers: Object.fromEntries(peers), errors,
      localAudioEnabled: local?.getAudioTracks()[0]?.enabled,
      localVideoCount: local?.getVideoTracks().length ?? 0,
      remoteTracks: [...remote.values()].flatMap((stream) => stream.getTracks().map((track) => ({ kind: track.kind, readyState: track.readyState }))),
      allLocalTracksEnded: [...localTracks].every((track) => track.readyState === "ended"),
    }),
    stats: async () => {
      const reports = await session.getStats();
      return reports.flatMap((report) => [...report.values()].filter((item) => item.type === "inbound-rtp").map((item) => ({ kind: item.kind, bytesReceived: item.bytesReceived, framesDecoded: item.framesDecoded })));
    },
  };
}
window.initializeCall = (userId) => { window.call = createCall(userId); };
