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
  remoteDescription: unknown = null;
  localDescription = { type: "answer", sdp: "answer-sdp" };
  senders: { replaceTrack: ReturnType<typeof vi.fn> }[] = [];
  data = { readyState: "open", send: vi.fn(), close: vi.fn(), onopen: null, onmessage: null };
  close = vi.fn();
  addIceCandidate = vi.fn().mockResolvedValue(undefined);
  restartIce = vi.fn();
  setConfiguration = vi.fn();
  setLocalDescription = vi.fn().mockResolvedValue(undefined);
  setRemoteDescription = vi.fn(async (description: unknown) => { this.remoteDescription = description; });
  constructor() { Connection.instances.push(this); }
  addTransceiver() { const sender = { replaceTrack: vi.fn().mockResolvedValue(undefined) }; this.senders.push(sender); return { sender }; }
  createDataChannel() { return this.data; }
}

const userId = "a".repeat(64);
const remoteId = "b".repeat(64);
const channelId = "00000000-0000-4000-8000-000000000001";
let captures: Stream[];
let getUserMedia: ReturnType<typeof vi.fn>;
let sessions: MediaSession[];

function fixture() {
  const callbacks = {
    userId, channelId, sendSignal: vi.fn().mockResolvedValue(undefined), announce: vi.fn().mockResolvedValue(undefined),
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
  vi.stubGlobal("MediaStream", Stream);
  vi.stubGlobal("AudioContext", Audio);
  vi.stubGlobal("RTCPeerConnection", Connection);
  Object.defineProperty(navigator, "mediaDevices", { configurable: true, value: { getUserMedia, getSupportedConstraints: () => ({}) } });
});
afterEach(async () => {
  await Promise.all(sessions.map((session) => session.stop()));
  vi.unstubAllGlobals();
});

describe("WebRTC call lifecycle", () => {
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
