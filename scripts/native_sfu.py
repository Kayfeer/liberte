"""Native CEF + real Rust admission server + LiveKit E2EE integration.

Positive calls use only the shipped UI. The separate adversarial browser receives
an authentic Bob token for Alice's room and one deliberately incompatible key;
no application API or encryption publication flag is mocked. Credentials travel
only through local process memory/stdin, never through reports or command lines.
"""
import argparse
import base64
import json
import math
import os
from pathlib import Path
import queue
import re
import shutil
import subprocess
import tempfile
import threading
import time

from native_smoke import until, NativeSession, isolated_environment
from native_integration import LocalServer, channel_id
from native_media import (run_media, execute, start_call, click_voice, capture_state, VOICE,
                          require_cef_qa_configuration)


NEGATIVE_PROBE = r"""
if (window.__codexSfuFailureProbe) throw new Error('SFU probe already installed');
const context = new AudioContext(), analyser = context.createAnalyser();
analyser.fftSize = 2048;
const silent = context.createGain(); silent.gain.value = 0;
analyser.connect(silent).connect(context.destination);
const sources = new Map(), videos = new Set(), callbacks = new Map();
const samples = new Float32Array(analyser.fftSize);
let closed = false, sampleCount = 0, maxRms = 0, frameCallbacks = 0;
function remember() {
    for (const video of document.querySelectorAll('section[aria-label="Appel audio et vidéo"] video')) {
        if (video.muted) continue;
        if (!videos.has(video)) {
            videos.add(video);
            if (video.requestVideoFrameCallback) {
                const frame = () => {
                    if (closed) return;
                    frameCallbacks++;
                    callbacks.set(video, video.requestVideoFrameCallback(frame));
                };
                callbacks.set(video, video.requestVideoFrameCallback(frame));
            }
        }
        const stream = video.srcObject;
        if (stream?.getAudioTracks().length && !sources.has(stream)) {
            const source = context.createMediaStreamSource(stream);
            source.connect(analyser); sources.set(stream, source);
        }
    }
}
const resume = () => { void context.resume().catch(() => {}); };
document.addEventListener('click', resume, true);
const observer = new MutationObserver(remember);
observer.observe(document.body, {subtree: true, childList: true});
const timer = setInterval(() => {
    remember();
    if (context.state !== 'running') return;
    analyser.getFloatTimeDomainData(samples);
    const rms = Math.sqrt(samples.reduce((sum, value) => sum + value * value, 0) / samples.length);
    if (!Number.isFinite(rms)) throw new Error('Non-finite SFU audio observation');
    sampleCount++; maxRms = Math.max(maxRms, rms);
}, 25);
window.__codexSfuFailureProbe = {
    snapshot() {
        remember();
        return {context: context.state, sampleCount, maxRms, frameCallbacks,
            decodedFrames: Math.max(0, ...[...videos].map(video => video.getVideoPlaybackQuality?.().totalVideoFrames || 0)),
            remoteVideos: videos.size, remoteAudioStreams: sources.size};
    },
    close() {
        closed = true; clearInterval(timer); observer.disconnect();
        document.removeEventListener('click', resume, true);
        for (const [video, callback] of callbacks) video.cancelVideoFrameCallback?.(callback);
        for (const source of sources.values()) source.disconnect();
        analyser.disconnect(); silent.disconnect(); void context.close();
        delete window.__codexSfuFailureProbe;
    },
};
return true;
"""


def token_identity(connection):
    try:
        payload = connection["token"].split(".")[1]
        claims = json.loads(base64.urlsafe_b64decode(payload + "=" * (-len(payload) % 4)))
        identity = claims["sub"]
        if (not re.fullmatch(r"[a-f0-9]{64}", identity)
                or claims["video"]["room"] != connection["roomName"]
                or claims["video"]["roomJoin"] is not True):
            raise ValueError()
        return identity
    except (ValueError, KeyError, IndexError, TypeError):
        raise AssertionError("Native admission did not issue an identity- and room-scoped token") from None


def adversary_connection(alice, bob):
    # LiveKit validates signatures. This local inspection only checks that the
    # test preserves the issued room and identities when replacing one key.
    for value in (alice, bob):
        if (not isinstance(value, dict) or not isinstance(value.get("encryptionKey"), str)
                or not re.fullmatch(r"[a-f0-9]{64}", value["encryptionKey"])):
            raise AssertionError("Native SFU did not return a 32-byte encryption key")
    if (not isinstance(alice.get("roomName"), str)
            or not re.fullmatch(r"[a-f0-9]{64}", alice["roomName"])):
        raise AssertionError("Native SFU room must be an opaque channel-derived identifier")
    if (alice.get("roomName") != bob.get("roomName") or alice.get("url") != bob.get("url")
            or alice["encryptionKey"] != bob["encryptionKey"]):
        raise AssertionError("Native peers must first agree on the same SFU room and key")
    identity = token_identity(bob)
    if identity == token_identity(alice):
        raise AssertionError("Adversarial participant must have its own native identity")
    wrong = bytearray.fromhex(bob["encryptionKey"])
    wrong[0] ^= 1
    return {"url": bob["url"], "token": bob["token"], "roomName": bob["roomName"],
            "encryptionKey": wrong.hex(), "identity": identity}


def fixture_metadata(value):
    if (not isinstance(value, dict) or value.get("runtime") != "CEF"
            or not re.fullmatch(r"(?:[A-Za-z]+/)?[0-9]+(?:\.[0-9]+){2,3}", str(value.get("browserVersion", "")))
            or not re.fullmatch(r"[a-f0-9]{64}", str(value.get("applicationSha256", "")))
            or not isinstance(value.get("sourceSha256"), dict)
            or any(not re.fullmatch(r"[a-f0-9]{64}", str(value["sourceSha256"].get(name, "")))
                   for name in ("launcher", "page", "participant"))):
        raise AssertionError("Adversarial CEF application and fixture hashes are required")
    return {"runtime": "CEF", "browserVersion": value["browserVersion"],
            "applicationSha256": value["applicationSha256"], "sourceSha256": {
                name: value["sourceSha256"][name] for name in ("launcher", "page", "participant")}}


def publisher_metrics(value):
    names = ("audioBytes", "videoBytes", "encodedFrames")
    if (value.get("e2ee") is not True
            or any(type(value.get(name)) is not int or value[name] <= 0 for name in names)):
        raise AssertionError("The independent publisher must have active E2EE and nonzero RTP counters")
    return {name: value[name] for name in names}


class AdversarialPeer:
    """Run an actual SDK peer, with bounded and credential-free diagnostics."""
    def __init__(self, script, connection):
        self.script, self.connection = Path(script), connection
        self.process = None
        self.fixture_metadata = None
        self.initial_metrics = None
        self.messages = queue.Queue()

    def __enter__(self):
        node = shutil.which("node")
        if not node or not self.script.is_file():
            raise RuntimeError("The native SFU peer fixture and Node.js are required")
        env = os.environ.copy()
        for name in ("LIVEKIT_API_SECRET", "LIVEKIT_API_KEY", "ADMIN_TOKEN"):
            env.pop(name, None)
        self.process = subprocess.Popen([node, str(self.script.resolve())], env=env,
                                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                        stderr=subprocess.DEVNULL, text=True, encoding="utf-8")
        process = self.process
        def read_messages():
            try:
                for line in process.stdout:
                    try:
                        if len(line) > 8192:
                            raise ValueError()
                        value = json.loads(line)
                        # Never forward arbitrary SDK output, which can contain URLs/tokens.
                        if not isinstance(value, dict):
                            raise ValueError()
                        self.messages.put({key: value.get(key) for key in ("status", "stage", "e2ee", "audio", "video", "fixture", "audioBytes", "videoBytes", "encodedFrames")})
                    except (ValueError, TypeError):
                        self.messages.put({"status": "invalid-output"})
            finally:
                self.messages.put({"status": "exited"})
        threading.Thread(target=read_messages, daemon=True).start()
        try:
            self.process.stdin.write(json.dumps(self.connection) + "\n")
            self.process.stdin.flush()
            self.connection = None
            deadline = time.monotonic() + 100
            while True:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise TimeoutError("The real adversarial SDK peer did not publish encrypted media")
                try:
                    value = self.messages.get(timeout=remaining)
                except queue.Empty:
                    raise TimeoutError("The real adversarial SDK peer did not publish encrypted media") from None
                if value.get("status") == "published":
                    if any(value.get(name) is not True for name in ("e2ee", "audio", "video")):
                        raise AssertionError("The adversarial SDK peer did not publish both tracks with actual E2EE")
                    self.fixture_metadata = fixture_metadata(value.get("fixture"))
                    self.initial_metrics = publisher_metrics(value)
                    return self
                if value.get("status") in ("error", "failed", "invalid-output", "exited"):
                    stage = value.get("stage")
                    known_stages = {"configuration", "fixture-server", "browser", "browser-sandbox", "fixture-load",
                                    "publication", "sources", "encryption", "connect", "publish", "outbound-media",
                                    "encryption-error", "disconnected", "waiting"}
                    phase = f" at {stage}" if isinstance(stage, str) and stage in known_stages else ""
                    raise RuntimeError(f"The adversarial SDK peer failed{phase} before encrypted publication; credentials omitted")
        except BaseException:
            self.__exit__(True, None, None)
            raise

    def prove_continued_publication(self):
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                raise AssertionError("The encrypted publisher exited before native failure was verified")
            self.process.stdin.write("snapshot\n")
            self.process.stdin.flush()
            try:
                value = self.messages.get(timeout=min(3, max(0.1, deadline - time.monotonic())))
            except queue.Empty:
                raise TimeoutError("The independent publisher did not answer its live RTP observation") from None
            if value.get("status") != "publishing":
                raise AssertionError("The independent encrypted publisher failed during native rejection")
            current = publisher_metrics(value)
            if all(current[name] > self.initial_metrics[name] for name in current):
                return {"initial": self.initial_metrics, "after_native_failure": current}
            time.sleep(0.1)
        raise AssertionError("Encrypted audio/video RTP did not keep progressing through the native failure")

    def __exit__(self, error_type, *_):
        if not self.process:
            return
        process, self.process = self.process, None
        try:
            if process.poll() is None:
                try:
                    process.stdin.write("stop\n")
                    process.stdin.flush()
                except (BrokenPipeError, OSError):
                    pass
                try:
                    process.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=5)
            if error_type is None and process.returncode != 0:
                raise RuntimeError("The adversarial SDK peer did not shut down cleanly")
        finally:
            process.stdin.close()
            process.stdout.close()


def failure_snapshot(client):
    value = execute(client, "return window.__codexSfuFailureProbe.snapshot();")
    if not isinstance(value, dict):
        raise AssertionError("Native wrong-key media observation returned an invalid result")
    return value


def require_encrypted_failure(value, errors):
    if not any("chiffrement" in str(error).casefold() for error in errors):
        raise AssertionError("The native app must report an explicit encryption failure")
    if not isinstance(value, dict) or value.get("context") != "running" or value.get("sampleCount", 0) < 3:
        raise AssertionError("An active audio observer is required to assess wrong-key media")
    rms = value.get("maxRms")
    if (not isinstance(rms, (int, float)) or not math.isfinite(rms) or rms < 0
            or rms > 0.0001 or value.get("decodedFrames", 1) != 0
            or value.get("frameCallbacks", 1) != 0):
        raise AssertionError("Media with an incompatible SFU key must never produce playable audio or decoded video")


def wrong_key_call(alice, bob, report, record, peer_script, application, driver, artifacts):
    channel = channel_id(alice)
    left = alice.invoke("get_sfu_connection", {"channelId": channel})
    right = bob.invoke("get_sfu_connection", {"channelId": channel})
    connection = adversary_connection(left, right)
    del left, right
    start_call(alice, "sfu")
    execute(alice, NEGATIVE_PROBE)
    try:
        click_voice(alice, "Activer la caméra")
        alice.wait_element(f'{VOICE} button[aria-label="Désactiver la caméra"]')
        before = capture_state(alice, {"microphone": "active", "camera": "active"})
        until(lambda: failure_snapshot(alice).get("context") == "running"
              and failure_snapshot(alice).get("sampleCount", 0) >= 3, "running native wrong-key media observer")
        with tempfile.TemporaryDirectory(prefix="liberte-sfu-adversary-", dir=os.environ.get("RUNNER_TEMP")) as temporary:
            with NativeSession(Path(application).resolve(), driver, isolated_environment(Path(temporary)),
                               Path(artifacts), "sfu-adversary") as adversary:
                configuration = adversary.invoke("qa_runtime_configuration")
                require_cef_qa_configuration(configuration, adversary.native_port)
                connection.update(cdpPort=adversary.native_port, applicationPath=str(Path(application).resolve()))
                with AdversarialPeer(peer_script, connection) as peer:
                    connection = None
                    def encryption_error():
                        errors = execute(alice, "return [...document.querySelectorAll('[role=\"alert\"]')].map(node => node.textContent.trim()).filter(Boolean);")
                        return errors if any("chiffrement" in error.casefold() for error in errors) else False
                    errors = until(encryption_error, "explicit native SFU encryption failure", timeout=45)
                    until(lambda: alice.invoke("get_call_state").get("inCall") is False,
                          "native call terminated after decryption failure")
                    after = capture_state(alice, {"microphone": "none", "camera": "none"})
                    if not after.get("allObservedRawTracksEnded"):
                        raise AssertionError("Encryption failure must end every previously observed raw capture track")
                    publisher_progress = peer.prove_continued_publication()
                    observation = failure_snapshot(alice)
                    require_encrypted_failure(observation, errors)
                    if execute(alice, f"return !!document.querySelector({json.dumps(VOICE)});"):
                        raise AssertionError("The failed encrypted call must leave the native call UI")
                    wrong_key_result = {"same_native_room": True, "real_native_token": True,
                                           "separate_sdk_peer": True, "key_mismatch": True,
                                           "encryption_error_visible": True, "observation": observation,
                                           "capture_before": before, "capture_after": after,
                                           "fixture": peer.fixture_metadata,
                                           "publisher_progress": publisher_progress}
        report["wrong_key"] = wrong_key_result
        record("Same-room peer with a real Rust-issued token and an incompatible key caused encryption failure, zero playable media and capture cleanup")
    finally:
        try:
            execute(alice, "window.__codexSfuFailureProbe?.close(); return null;")
        except Exception:
            # NativeSession still closes the process; preserve the primary failure.
            pass


def run_sfu(application, server_binary, driver, artifacts, peer_script):
    if driver not in ("cef", "cdp"):
        raise ValueError("Native SFU proof requires the CEF prototype")
    for name in ("LIVEKIT_URL", "LIVEKIT_API_KEY", "LIVEKIT_API_SECRET"):
        if not os.environ.get(name):
            raise RuntimeError(f"{name} must configure the real local LiveKit service")
    server_binary = Path(server_binary).resolve(strict=True)
    if not server_binary.is_file() or not os.access(server_binary, os.X_OK):
        raise ValueError("The real admission server must be executable")
    artifacts = Path(artifacts).resolve()
    artifacts.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="liberte-sfu-qa-", dir=os.environ.get("RUNNER_TEMP")) as temporary:
        with LocalServer(server_binary, Path(temporary), artifacts) as server:
            return run_media(application, driver, artifacts, server_url=server.base, mode="sfu",
                             after_call=lambda alice, bob, report, record:
                             wrong_key_call(alice, bob, report, record, peer_script,
                                            application, "cef", artifacts))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--application", required=True)
    parser.add_argument("--server", required=True)
    parser.add_argument("--driver", default="cef")
    parser.add_argument("--artifacts", default=".ci-output/native-sfu")
    parser.add_argument("--peer-script", default=str(Path(__file__).with_name("native_sfu_peer.mjs")))
    args = parser.parse_args()
    run_sfu(args.application, args.server, args.driver, args.artifacts, args.peer_script)


if __name__ == "__main__":
    main()
