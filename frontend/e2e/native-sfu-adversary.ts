/** Independent SDK participant: no application session, IPC mocks or encryption overrides. */
import {
  ExternalE2EEKeyProvider, isE2EESupported, LocalAudioTrack, LocalVideoTrack,
  Room, RoomEvent, Track,
} from "livekit-client";
import E2EEWorker from "livekit-client/e2ee-worker?worker";

interface Connection {
  url: string;
  token: string;
  roomName: string;
  encryptionKey: string;
  identity: string;
}
type Status = "idle" | "configured" | "starting" | "published" | "failed" | "stopped";
interface Snapshot {
  status: Status;
  phase: string;
  e2ee: boolean;
  audio: boolean;
  video: boolean;
  audioBytes: number;
  videoBytes: number;
  encodedFrames: number;
  localEnded: boolean;
}

let connection: Connection | null = null;
let status: Status = "idle";
let phase = "idle";
let room: Room | null = null;
let worker: Worker | null = null;
let context: AudioContext | null = null;
let oscillator: OscillatorNode | null = null;
let analyser: AnalyserNode | null = null;
let audio: LocalAudioTrack | null = null;
let video: LocalVideoTrack | null = null;
let rawTracks: MediaStreamTrack[] = [];
let paintTimer: ReturnType<typeof setInterval> | null = null;
let paintCount = 0;
let stopping = false;
let encryptedPublications = false;
let sourceRms = 0;
let audioBytes = 0;
let videoBytes = 0;
let encodedFrames = 0;

async function updateMediaProof() {
  if (!audio || !video || !analyser) throw new Error("Synthetic sources are unavailable");
  const samples = new Float32Array(analyser.fftSize);
  analyser.getFloatTimeDomainData(samples);
  sourceRms = Math.sqrt(samples.reduce((sum, sample) => sum + sample * sample, 0) / samples.length);
  const reports = await Promise.all([audio.getRTCStatsReport(), video.getRTCStatsReport()]);
  for (const report of reports) report?.forEach((entry) => {
    if (entry.type !== "outbound-rtp") return;
    if ((entry.kind ?? entry.mediaType) === "audio") audioBytes = Math.max(audioBytes, entry.bytesSent ?? 0);
    if ((entry.kind ?? entry.mediaType) === "video") {
      videoBytes = Math.max(videoBytes, entry.bytesSent ?? 0);
      encodedFrames = Math.max(encodedFrames, entry.framesEncoded ?? 0);
    }
  });
}

function render() {
  document.querySelector("#status")!.textContent = status + " (" + phase + ")";
}
function requireActive() {
  if (stopping || status === "failed") throw new Error("Test participant is stopped");
}
function requireEncryption() {
  requireActive();
  if (!room?.localParticipant.isE2EEEnabled) throw new Error("Actual SDK encryption is not enabled");
}
async function stop() {
  if (stopping) return;
  stopping = true;
  if (paintTimer !== null) clearInterval(paintTimer);
  paintTimer = null;
  for (const track of rawTracks) track.stop();
  audio?.stop();
  video?.stop();
  try { oscillator?.stop(); } catch { /* already stopped */ }
  oscillator?.disconnect();
  analyser?.disconnect();
  const activeContext = context;
  context = null;
  const activeRoom = room;
  room = null;
  try {
    await Promise.all([
      activeContext?.close(),
      activeRoom?.disconnect(),
    ]);
  } finally {
    worker?.terminate();
    worker = null;
    if (connection) { connection.token = ""; connection.encryptionKey = ""; }
    connection = null;
    if (status !== "failed") status = "stopped";
    render();
  }
}
async function publish() {
  if (status !== "configured" || !connection) throw new Error("Participant is not configured");
  status = "starting";
  phase = "sources";
  render();
  try {
    if (!isE2EESupported()) throw new Error("The browser has no E2EE transform support");
    // Resume inside the real button click, without an autoplay-policy bypass.
    context = new AudioContext();
    const resumed = context.resume();
    const destination = context.createMediaStreamDestination();
    oscillator = context.createOscillator();
    oscillator.frequency.value = 440;
    const gain = context.createGain();
    gain.gain.value = 0.2;
    analyser = context.createAnalyser();
    analyser.fftSize = 1024;
    oscillator.connect(gain);
    gain.connect(analyser);
    analyser.connect(destination);
    oscillator.start();
    const canvas = document.querySelector<HTMLCanvasElement>("#source")!;
    const drawing = canvas.getContext("2d");
    if (!drawing) throw new Error("Canvas is unavailable");
    const paint = () => {
      paintCount += 1;
      drawing.fillStyle = "hsl(" + (paintCount * 7 % 360) + " 80% 45%)";
      drawing.fillRect(0, 0, canvas.width, canvas.height);
      drawing.fillStyle = "#fff";
      drawing.fillRect((paintCount * 9) % 280, 40, 40, 100);
    };
    paint();
    paintTimer = setInterval(paint, 50);
    const canvasStream = canvas.captureStream(20);
    rawTracks = [...destination.stream.getAudioTracks(), ...canvasStream.getVideoTracks()];
    if (rawTracks.length !== 2) throw new Error("Both synthetic source tracks are required");
    audio = new LocalAudioTrack(destination.stream.getAudioTracks()[0], undefined, true);
    video = new LocalVideoTrack(canvasStream.getVideoTracks()[0], undefined, true);
    await resumed;
    requireActive();
    if (context.state !== "running") throw new Error("The source AudioContext did not start");

    phase = "encryption";
    render();
    const key = Uint8Array.from(connection.encryptionKey.match(/../g)!, (byte) => Number.parseInt(byte, 16));
    connection.encryptionKey = "";
    const provider = new ExternalE2EEKeyProvider();
    worker = new E2EEWorker();
    room = new Room({
      encryption: { keyProvider: provider, worker },
      adaptiveStream: false,
      dynacast: false,
      stopLocalTrackOnUnpublish: true,
    });
    room.on(RoomEvent.EncryptionError, () => {
      status = "failed"; phase = "encryption-error"; render();
      void stop().catch(() => {});
    });
    room.on(RoomEvent.Disconnected, () => {
      if (!stopping) { status = "failed"; phase = "disconnected"; render(); void stop().catch(() => {}); }
    });
    try { await provider.setKey(key.buffer); } finally { key.fill(0); }
    requireActive();
    await room.setE2EEEnabled(true);
    requireEncryption();
    phase = "connect";
    render();
    await room.connect(connection.url, connection.token, {
      autoSubscribe: false,
      rtcConfig: { iceServers: [] },
    });
    connection.token = "";
    requireEncryption();
    if (room.name !== connection.roomName || room.localParticipant.identity !== connection.identity) {
      throw new Error("The actual SFU room or identity differs from the authorized configuration");
    }
    phase = "publish";
    render();
    const audioPublication = await room.localParticipant.publishTrack(audio, {
      source: Track.Source.Microphone, dtx: false,
    });
    requireEncryption();
    const videoPublication = await room.localParticipant.publishTrack(video, {
      source: Track.Source.Camera, simulcast: false, videoCodec: "vp8",
    });
    requireEncryption();
    if (!audioPublication.isEncrypted || !videoPublication.isEncrypted) throw new Error("SDK published unencrypted media");
    encryptedPublications = true;

    phase = "outbound-media";
    render();
    const deadline = Date.now() + 20_000;
    while (Date.now() < deadline) {
      requireEncryption();
      await updateMediaProof();
      if (sourceRms > 0.01 && audioBytes > 0 && videoBytes > 0 && encodedFrames >= 2 && paintCount >= 3) {
        requireEncryption();
        status = "published";
        render();
        return;
      }
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    throw new Error("Encrypted publication did not produce real outgoing audio and video");
  } catch {
    status = "failed";
    render();
    await stop().catch(() => {});
  }
}

const api = {
  configure(value: Connection) {
    if (status !== "idle") throw new Error("Participant already configured");
    const url = new URL(value.url);
    if (!["localhost", "127.0.0.1", "[::1]"].includes(url.hostname)
        || !["ws:", "wss:"].includes(url.protocol) || url.username || url.password
        || !/^[0-9a-f]{64}$/i.test(value.encryptionKey) || !/^[0-9a-f]{64}$/i.test(value.identity)
        || !value.token || !value.roomName) throw new Error("Invalid synthetic SFU configuration");
    connection = { ...value };
    status = "configured";
    render();
  },
  snapshot(): Snapshot {
    return { status, phase, e2ee: encryptedPublications && room?.localParticipant.isE2EEEnabled === true,
      audio: audioBytes > 0 && sourceRms > 0.01 && audio?.mediaStreamTrack.readyState === "live",
      video: videoBytes > 0 && encodedFrames >= 2 && video?.mediaStreamTrack.readyState === "live",
      audioBytes, videoBytes, encodedFrames,
      localEnded: rawTracks.length > 0 && rawTracks.every((track) => track.readyState === "ended") };
  },
  async sample(): Promise<Snapshot> {
    requireEncryption();
    await updateMediaProof();
    return api.snapshot();
  },
  stop,
};
declare global { interface Window { nativeSfuAdversary: typeof api } }
window.nativeSfuAdversary = api;
document.querySelector<HTMLButtonElement>("#publish")!.addEventListener("click", () => { void publish(); });
