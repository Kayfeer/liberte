import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { applyMediaPreferences, configurePlayback, MediaSession, releasePlayback } from "./mediaSession";

class Track extends EventTarget {
  enabled = true;
  readyState = "live";
  muted = false;
  stop = vi.fn(() => { this.readyState = "ended"; });
  constructor(public kind: string) { super(); }
}
class Stream {
  constructor(private tracks: Track[] = []) {}
  getTracks() { return this.tracks; }
  getAudioTracks() { return this.tracks.filter((track) => track.kind === "audio"); }
  getVideoTracks() { return this.tracks.filter((track) => track.kind === "video"); }
  addTrack(track: Track) { this.tracks.push(track); }
}
class Audio {
  static instances: Audio[] = [];
  destination = new Stream([new Track("audio")]);
  gain = { gain: { value: 1 }, connect: vi.fn() };
  resume = vi.fn().mockResolvedValue(undefined);
  close = vi.fn().mockResolvedValue(undefined);
  constructor() { Audio.instances.push(this); }
  createGain() { return this.gain; }
  createMediaStreamSource() { return { connect: () => this.gain }; }
  createMediaStreamDestination() { return { stream: this.destination }; }
}
class Connection {
  static instances: Connection[] = [];
  connectionState = "new";
  signalingState = "stable";
  onnegotiationneeded: (() => Promise<void>) | null = null;
  remoteDescription: unknown = null;
  localDescription = { type: "answer", sdp: "answer-sdp" };
  senders: { track: Track | null; replaceTrack: ReturnType<typeof vi.fn> }[] = [];
  transceivers: { sender: { track: Track | null; replaceTrack: ReturnType<typeof vi.fn> }; receiver: { track: Track }; mid: string | null; stopped: boolean; direction: string }[] = [];
  data = { readyState: "open", send: vi.fn(), close: vi.fn(), onopen: null, onmessage: null };
  close = vi.fn();
  addIceCandidate = vi.fn().mockResolvedValue(undefined);
  restartIce = vi.fn();
  setConfiguration = vi.fn();
  setLocalDescription = vi.fn().mockResolvedValue(undefined);
  setRemoteDescription = vi.fn(async (description: { type: string }) => {
    this.remoteDescription = description;
    if (description.type === "offer") {
      // Explicit addTransceiver-created entries are not adopted by an offer.
      // This models the browser association rule that caused one-way media.
      for (const [index, kind] of ["audio", "video"].entries()) {
        const transceiver = this.addTransceiver(kind);
        transceiver.mid = String(index);
        transceiver.direction = "recvonly";
      }
    }
  });
  constructor() { Connection.instances.push(this); }
  addTransceiver(track: Track | string) {
    const sender: { track: Track | null; replaceTrack: ReturnType<typeof vi.fn> } = { track: typeof track === "string" ? null : track, replaceTrack: vi.fn() };
    sender.replaceTrack.mockImplementation(async (next: Track | null) => { sender.track = next; });
    const transceiver = { sender, receiver: { track: new Track(typeof track === "string" ? track : track.kind) }, mid: null as string | null, stopped: false, direction: "sendrecv" };
    this.senders.push(sender);
    this.transceivers.push(transceiver);
    return transceiver;
  }
  getTransceivers() { return this.transceivers; }
  createDataChannel() { return this.data; }
}

const userId = "a".repeat(64);
const remoteId = "b".repeat(64);
const channelId = "00000000-0000-4000-8000-000000000001";
let captures: Stream[];
let getUserMedia: ReturnType<typeof vi.fn>;
let sessions: MediaSession[];

function fixture(localUserId = userId) {
  const callbacks = {
    userId: localUserId, channelId, sendSignal: vi.fn().mockResolvedValue(undefined), announce: vi.fn().mockResolvedValue(undefined),
    onLocalStream: vi.fn(), onRemoteStream: vi.fn(), onPeer: vi.fn(), onError: vi.fn(), onCameraEnded: vi.fn(),
  };
  const session = new MediaSession(callbacks);
  sessions.push(session);
  return { session, callbacks };
}

beforeEach(() => {
  localStorage.clear();
  captures = [];
  sessions = [];
  Audio.instances = [];
  Connection.instances = [];
  getUserMedia = vi.fn(async ({ video }: MediaStreamConstraints) => {
    const stream = new Stream([new Track(video ? "video" : "audio")]);
    captures.push(stream);
    return stream;
  });
  vi.stubGlobal("isSecureContext", true);
  vi.stubGlobal("MediaStream", Stream);
  vi.stubGlobal("AudioContext", Audio);
  vi.stubGlobal("RTCPeerConnection", Connection);
  Object.defineProperty(navigator, "mediaDevices", { configurable: true, value: { getUserMedia, getSupportedConstraints: () => ({}) } });
});
afterEach(async () => {
  await Promise.all(sessions.map((session) => session.stop()));
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("WebRTC call lifecycle", () => {
  it.each([
    ["an insecure context", () => { vi.stubGlobal("isSecureContext", false); }, "contexte sécurisé"],
    ["missing capture support", () => { Object.defineProperty(navigator, "mediaDevices", { configurable: true, value: undefined }); }, "getUserMedia"],
    ["missing peer connection support", () => { vi.stubGlobal("RTCPeerConnection", undefined); }, "RTCPeerConnection"],
  ] as const)("identifies %s before capture or signaling", async (_name, configure, message) => {
    configure();
    const { session, callbacks } = fixture();
    await expect(session.start()).rejects.toThrow(message);
    expect(getUserMedia).not.toHaveBeenCalled();
    expect(Audio.instances).toHaveLength(0);
    expect(Connection.instances).toHaveLength(0);
    expect(callbacks.sendSignal).not.toHaveBeenCalled();
    expect(callbacks.onLocalStream).not.toHaveBeenCalled();
  });

  it("starts audio only, mutes the source and sent track, and releases every resource", async () => {
    const { session, callbacks } = fixture();
    await session.start();
    expect(getUserMedia).toHaveBeenCalledWith(expect.objectContaining({ video: false }));
    session.ensurePeer(remoteId);
    session.setMuted(true);
    expect(captures[0].getAudioTracks()[0].enabled).toBe(false);
    expect(Audio.instances[0].destination.getAudioTracks()[0].enabled).toBe(false);
    await session.setVideoEnabled(true);
    const camera = captures[1].getVideoTracks()[0];
    expect(Connection.instances[0].senders[1].replaceTrack).toHaveBeenCalledWith(camera);
    await session.setVideoEnabled(false);
    expect(camera.stop).toHaveBeenCalledOnce();
    await session.stop();
    expect(captures[0].getAudioTracks()[0].stop).toHaveBeenCalledOnce();
    expect(Audio.instances[0].close).toHaveBeenCalledOnce();
    expect(Connection.instances[0].close).toHaveBeenCalledOnce();
    expect(callbacks.sendSignal).toHaveBeenCalledWith(remoteId, "Hangup");
    await session.stop();
    expect(Connection.instances[0].close).toHaveBeenCalledOnce();
  });

  it("releases permission results that arrive after the caller hangs up", async () => {
    let resolve!: (stream: Stream) => void;
    getUserMedia.mockImplementationOnce(() => new Promise<Stream>((done) => { resolve = done; }));
    const { session } = fixture();
    const starting = session.start();
    const rejection = expect(starting).rejects.toThrow("annulé");
    await session.stop();
    const late = new Stream([new Track("audio")]);
    resolve(late);
    await rejection;
    expect(late.getTracks()[0].stop).toHaveBeenCalledOnce();
    expect(Audio.instances).toHaveLength(0);
  });

  it("releases the physical microphone when hanging up while AudioContext resume is blocked", async () => {
    class SuspendedAudio extends Audio {
      resume = vi.fn(() => new Promise<void>(() => {}));
    }
    vi.stubGlobal("AudioContext", SuspendedAudio);
    const { session } = fixture();
    const starting = session.start();
    const rejection = expect(starting).rejects.toThrow("annul");
    await vi.waitFor(() => expect(Audio.instances).toHaveLength(1));
    await session.stop();
    await rejection;
    expect(captures[0].getAudioTracks()[0].stop).toHaveBeenCalledOnce();
    expect(Audio.instances[0].close).toHaveBeenCalledOnce();
  });

  it("preserves audio when camera access fails", async () => {
    const { session } = fixture();
    await session.start();
    getUserMedia.mockRejectedValueOnce(new DOMException("Permission refusée", "NotAllowedError"));
    await expect(session.setVideoEnabled(true)).rejects.toThrow("Permission refusée");
    expect(captures[0].getAudioTracks()[0].stop).not.toHaveBeenCalled();
  });

  it("stops the camera even when a peer refuses track detachment", async () => {
    const { session } = fixture();
    await session.start();
    session.ensurePeer(remoteId);
    await session.setVideoEnabled(true);
    Connection.instances[0].senders[1].replaceTrack.mockRejectedValueOnce(new Error("Peer closed"));
    await session.setVideoEnabled(false);
    expect(captures[1].getVideoTracks()[0].stop).toHaveBeenCalledOnce();
    expect(getUserMedia).toHaveBeenCalledTimes(2);
  });

  it("replaces microphone without unmuting and applies the configured gain", async () => {
    const { session } = fixture();
    await session.start();
    session.ensurePeer(remoteId);
    session.setMuted(true);
    localStorage.setItem("liberte-media-settings", JSON.stringify({ selectedAudioInput: "usb-mic", inputVolume: 50 }));
    await applyMediaPreferences();
    expect(getUserMedia).toHaveBeenLastCalledWith(expect.objectContaining({ audio: expect.objectContaining({ deviceId: { exact: "usb-mic" } }) }));
    expect(captures[0].getTracks()[0].stop).toHaveBeenCalledOnce();
    expect(captures[1].getTracks()[0].enabled).toBe(false);
    expect(Audio.instances[1].gain.gain.value).toBe(0.5);
    expect(Connection.instances[0].senders[0].replaceTrack).toHaveBeenCalledWith(Audio.instances[1].destination.getTracks()[0]);
  });

  it("gives a participant joining during a microphone change the live replacement", async () => {
    const { session } = fixture();
    await session.start();
    session.ensurePeer(remoteId);
    let finish!: () => void;
    Connection.instances[0].senders[0].replaceTrack.mockImplementationOnce(async () => new Promise<void>((resolve) => { finish = resolve; }));
    localStorage.setItem("liberte-media-settings", JSON.stringify({ selectedAudioInput: "usb-mic" }));
    const changing = session.applyPreferences();
    await vi.waitFor(() => expect(finish).toBeTypeOf("function"));
    session.ensurePeer("c".repeat(64));
    session.setMuted(true);
    const replacement = Audio.instances[1].destination.getAudioTracks()[0];
    expect(Connection.instances[1].senders[0].track).toBe(replacement);
    expect(replacement.enabled).toBe(false);
    finish();
    await changing;
    expect(replacement.readyState).toBe("live");
    expect(replacement.enabled).toBe(false);
    expect(captures[1].getAudioTracks()[0].enabled).toBe(false);
    expect(Audio.instances[0].destination.getAudioTracks()[0].readyState).toBe("ended");
  });

  it("gives a participant joining while the camera starts the pending video track", async () => {
    const { session } = fixture();
    await session.start();
    session.ensurePeer(remoteId);
    let finish!: () => void;
    Connection.instances[0].senders[1].replaceTrack.mockImplementationOnce(async () => new Promise<void>((resolve) => { finish = resolve; }));
    const enabling = session.setVideoEnabled(true);
    await vi.waitFor(() => expect(finish).toBeTypeOf("function"));
    session.ensurePeer("c".repeat(64));
    const camera = captures[1].getVideoTracks()[0];
    expect(Connection.instances[1].senders[1].track).toBe(camera);
    finish();
    await enabling;
    expect(camera.readyState).toBe("live");
  });

  it("stops the camera immediately during a pending device permission and discards its late result", async () => {
    const { session } = fixture();
    await session.start();
    await session.setVideoEnabled(true);
    const previous = captures[1].getVideoTracks()[0];
    let finish!: (stream: Stream) => void;
    getUserMedia.mockImplementationOnce(() => new Promise<Stream>((resolve) => { finish = resolve; }));
    localStorage.setItem("liberte-media-settings", JSON.stringify({ selectedVideoInput: "new-camera" }));
    const changing = session.applyPreferences();
    await vi.waitFor(() => expect(finish).toBeTypeOf("function"));
    const disabling = session.setVideoEnabled(false);
    expect(previous.readyState).toBe("ended");
    await disabling;
    const lateCamera = new Track("video");
    finish(new Stream([lateCamera]));
    await changing;
    expect(lateCamera.readyState).toBe("ended");
    session.ensurePeer(remoteId);
    expect(Connection.instances[0].senders[1].track).toBeNull();
  });

  it.each(["audio", "video"] as const)("hangup immediately releases a pending %s capture without waiting for the sender", async (kind) => {
    const { session } = fixture();
    await session.start();
    session.ensurePeer(remoteId);
    let finish!: () => void;
    Connection.instances[0].senders[kind === "audio" ? 0 : 1].replaceTrack.mockImplementationOnce(async () => new Promise<void>((resolve) => { finish = resolve; }));
    localStorage.setItem("liberte-media-settings", JSON.stringify({ selectedAudioInput: "usb-mic" }));
    const changing = kind === "audio" ? session.applyPreferences() : session.setVideoEnabled(true);
    await vi.waitFor(() => expect(finish).toBeTypeOf("function"));
    await session.stop();
    expect(captures.flatMap((stream) => stream.getTracks()).every((track) => track.readyState === "ended")).toBe(true);
    for (const context of Audio.instances) {
      expect(context.close).toHaveBeenCalledOnce();
      expect(context.destination.getTracks()[0].readyState).toBe("ended");
    }
    finish();
    await changing;
    expect(captures[1].getTracks()[0].stop).toHaveBeenCalledOnce();
  });

  it("resends an outstanding offer when GossipSub publication failed instead of waiting for stable signaling", async () => {
    const { session, callbacks } = fixture();
    await session.start();
    session.ensurePeer(remoteId);
    const connection = Connection.instances[0];
    connection.localDescription = { type: "offer", sdp: "offer-with-gathered-ice" };
    connection.signalingState = "have-local-offer";
    callbacks.sendSignal.mockRejectedValueOnce(new Error("InsufficientPeers"));
    await connection.onnegotiationneeded!();
    expect(callbacks.onError).toHaveBeenCalledWith("InsufficientPeers");
    session.retryConnections();
    await vi.waitFor(() => expect(callbacks.sendSignal).toHaveBeenCalledTimes(2));
    expect(callbacks.sendSignal).toHaveBeenLastCalledWith(remoteId, { Offer: "offer-with-gathered-ice" });
    expect(connection.restartIce).not.toHaveBeenCalled();
  });

  it("keeps connected media alive during a signaling outage and expires disconnected peers", async () => {
    vi.useFakeTimers();
    const { session } = fixture();
    await session.start();
    session.ensurePeer(remoteId);
    const connection = Connection.instances[0];
    connection.connectionState = "connected";
    session.announce();
    await vi.advanceTimersByTimeAsync(40_000);
    expect(connection.close).not.toHaveBeenCalled();
    connection.connectionState = "disconnected";
    await vi.advanceTimersByTimeAsync(8_000);
    expect(connection.close).toHaveBeenCalledOnce();
  });

  it("buffers ICE until SDP arrives and ignores another room or recipient", async () => {
    const { session, callbacks } = fixture();
    await session.start();
    const base = { channelId, sender: remoteId, target: userId };
    await session.receive({ ...base, channelId: "other", signalType: { Offer: "sdp" } });
    await session.receive({ ...base, target: "c".repeat(64), signalType: { Offer: "sdp" } });
    expect(Connection.instances).toHaveLength(0);
    await session.receive({ ...base, signalType: { IceCandidate: JSON.stringify({ candidate: "candidate:1" }) } });
    expect(Connection.instances[0].addIceCandidate).not.toHaveBeenCalled();
    await session.receive({ ...base, signalType: { Offer: "v=0\r\n" } });
    expect(Connection.instances[0].addIceCandidate).toHaveBeenCalledWith({ candidate: "candidate:1" });
    expect(callbacks.sendSignal).toHaveBeenCalledWith(remoteId, { Answer: "answer-sdp" });
    await session.receive({ ...base, signalType: "Hangup" });
    expect(Connection.instances[0].close).toHaveBeenCalledOnce();
  });

  it("attaches answerer audio and camera to the offered m-lines without orphan senders", async () => {
    const { session, callbacks } = fixture(remoteId);
    await session.start();
    session.ensurePeer(userId);
    const connection = Connection.instances[0];
    expect(connection.getTransceivers()).toHaveLength(0);
    await session.receive({ channelId, sender: userId, target: remoteId, signalType: { Offer: "v=0\r\n" } });
    expect(connection.getTransceivers()).toHaveLength(2);
    expect(connection.senders[0].replaceTrack).toHaveBeenCalledWith(Audio.instances[0].destination.getAudioTracks()[0]);
    expect(connection.senders[1].replaceTrack).toHaveBeenCalledWith(null);
    expect(connection.getTransceivers().every((transceiver) => transceiver.direction === "sendrecv")).toBe(true);
    await session.setVideoEnabled(true);
    expect(connection.senders[1].replaceTrack).toHaveBeenLastCalledWith(captures[1].getVideoTracks()[0]);
    expect(callbacks.sendSignal).toHaveBeenCalledWith(userId, { Answer: "answer-sdp" });
  });

  it("applies output volume and selected speaker without feeding back the local preview", async () => {
    localStorage.setItem("liberte-media-settings", JSON.stringify({ selectedAudioOutput: "headphones", outputVolume: 25 }));
    const element = document.createElement("video");
    const setSinkId = vi.fn().mockResolvedValue(undefined);
    Object.assign(element, { setSinkId });
    await configurePlayback(element);
    expect(element.volume).toBe(0.25);
    expect(setSinkId).toHaveBeenCalledWith("headphones");
    await configurePlayback(element, true);
    expect(element.muted).toBe(true);
    expect(setSinkId).toHaveBeenCalledOnce();
    releasePlayback(element);
  });
});
