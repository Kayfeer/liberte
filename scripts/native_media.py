"""Native media QA: real clients/signaling/WebRTC, synthetic WebKit capture.

Requires the separate debug-only qa-media-devices binary. WebDriver engine consent
is not evidence about physical devices, the GTK prompt, or release media runtime.
Diagnostic JavaScript only observes real DOM streams; it replaces no app API.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile

from native_smoke import NativeSession, isolated_environment, until
from native_integration import ALICE, BOB, CHANNEL, identity_ui, network_ready, select_channel, xpath_text

VOICE = 'section[aria-label="Appel audio et vid\u00e9o"]'
INSTALL_PROBE = r"""
const root = document.querySelector('section[aria-label="Appel audio et vidéo"]');
const videos = root ? [...root.querySelectorAll('video')] : [];
const local = videos.find(video => video.muted);
const remote = videos.find(video => !video.muted && video.srcObject);
if (!local?.srcObject || !remote?.srcObject?.getAudioTracks().length) return false;
if (window.__codexNativeMediaProbe) throw new Error('Media probe already installed');
const context = new AudioContext();
const source = context.createMediaStreamSource(remote.srcObject);
const analyser = context.createAnalyser(); analyser.fftSize = 2048;
const silent = context.createGain(); silent.gain.value = 0;
source.connect(analyser).connect(silent).connect(context.destination);
const samples = new Float32Array(analyser.fftSize);
const localTracks = new Set(), remoteTracks = new Set();
let closed = false, frameCallbacks = 0, callbackId = null;
let sampleCount = 0, maxRms = 0, history = [];
function remember() {
    local.srcObject?.getTracks().forEach(track => localTracks.add(track));
    remote.srcObject?.getTracks().forEach(track => remoteTracks.add(track));
}
const resume = () => { void context.resume().catch(() => {}); };
// The next real camera UI click provides a gesture for this diagnostic context.
document.addEventListener('click', resume, true);
const timer = setInterval(() => {
    remember();
    if (context.state !== 'running') return;
    analyser.getFloatTimeDomainData(samples);
    const rms = Math.sqrt(samples.reduce((sum, value) => sum + value * value, 0) / samples.length);
    if (!Number.isFinite(rms)) return;
    sampleCount++; maxRms = Math.max(maxRms, rms);
    const now = performance.now();
    history.push({time: now, rms}); history = history.filter(sample => now - sample.time < 4000);
}, 50);
function frame() {
    if (closed) return;
    frameCallbacks++;
    callbackId = remote.requestVideoFrameCallback(frame);
}
if (typeof remote.requestVideoFrameCallback === 'function') callbackId = remote.requestVideoFrameCallback(frame);
function trackInfo(tracks) {
    return [...tracks].map(track => ({kind: track.kind, enabled: track.enabled,
        readyState: track.readyState, muted: track.muted}));
}
window.__codexNativeMediaProbe = {
    snapshot() {
        remember();
        const now = performance.now();
        const recent = history.filter(sample => now - sample.time < 2000);
        const quality = remote.getVideoPlaybackQuality?.();
        const tile = remote.parentElement?.parentElement;
        return {
            context: context.state, sampleCount, maxRms,
            recentRms: recent.length ? Math.max(...recent.map(sample => sample.rms)) : null,
            recentSamples: recent.length, frameCallbacks,
            decodedFrames: Math.max(quality?.totalVideoFrames || 0, remote.webkitDecodedFrameCount || 0),
            width: remote.videoWidth, height: remote.videoHeight,
            local: trackInfo(local.srcObject?.getTracks() || []),
            remote: trackInfo(remote.srcObject?.getTracks() || []),
            heldLocal: trackInfo(localTracks), heldRemote: trackInfo(remoteTracks),
            remoteMutedIndicator: !!tile?.querySelector('[aria-label="Micro coupé"]'),
            remoteCameraOff: !!tile?.querySelector('[aria-label="Caméra désactivée"]'),
            visibleVideos: document.querySelectorAll('section[aria-label="Appel audio et vidéo"] video').length,
        };
    },
    close() {
        closed = true; clearInterval(timer); document.removeEventListener('click', resume, true);
        if (callbackId !== null && remote.cancelVideoFrameCallback) remote.cancelVideoFrameCallback(callbackId);
        source.disconnect(); analyser.disconnect(); silent.disconnect();
        void context.close();
        delete window.__codexNativeMediaProbe;
    },
};
return true;
"""


def execute(client, script):
    # Script execution is never automatically retried, even for diagnostics.
    return client.command("POST", "/execute/sync", {"script": script, "args": []})


def snapshot(client):
    result = execute(client, "return window.__codexNativeMediaProbe.snapshot();")
    if not isinstance(result, dict):
        raise RuntimeError("Native media observation did not return an object")
    return result


def frames(value):
    return max(value.get("decodedFrames", 0), value.get("frameCallbacks", 0))


def received_media(value, baseline):
    kinds = {track.get("kind") for track in value.get("remote", []) if track.get("readyState") == "live"}
    return (value.get("context") == "running" and value.get("sampleCount", 0) >= 3
            and value.get("maxRms", 0) > 0.0001 and {"audio", "video"}.issubset(kinds)
            and value.get("width", 0) > 0 and value.get("height", 0) > 0
            and frames(value) >= baseline + 2)


def tracks_ended(tracks):
    return bool(tracks) and all(track.get("readyState") == "ended" for track in tracks)


def observe_until(client, predicate, description, timeout=45):
    latest = {}
    def check():
        nonlocal latest
        client.assert_no_alert()
        latest = snapshot(client)
        return predicate(latest)
    try:
        until(check, description, timeout=timeout)
    except TimeoutError as error:
        raise TimeoutError(f"{description}; last media observation: {latest}") from error
    return latest


def click_voice(client, label):
    client.click(f'{VOICE} button[aria-label="{label}"]')


def start_call(client):
    client.click('button[aria-label="Rejoindre l\'appel"]')
    client.wait_element(VOICE)
    state = client.invoke("get_call_state")
    if state.get("inCall") is not True or state.get("mode") != "mesh":
        raise AssertionError("The native client did not enter the requested P2P call")


def capture_state(client, expected=None):
    latest = {}
    def observed():
        nonlocal latest
        latest = client.invoke("qa_media_capture_state")
        if not isinstance(latest, dict) or latest.get("mockCaptureDevices") is not True or latest.get("automationControlled") is not True:
            raise AssertionError("QA capture must use synthetic devices under WebDriver automation")
        return all(latest.get(kind) == state for kind, state in (expected or {}).items())
    try:
        until(observed, "WebKit capture state", timeout=20)
    except TimeoutError as error:
        raise TimeoutError(f"Expected WebKit capture {expected}; observed {latest}") from error
    return latest


def stop_probe(client):
    try:
        execute(client, "window.__codexNativeMediaProbe?.close(); return null;")
    except Exception as error:
        print(f"Native media diagnostic cleanup: {type(error).__name__}", file=sys.stderr)


def run_media(application, driver, artifacts):
    if sys.platform != "linux":
        raise RuntimeError("Native media QA requires Linux")
    application = Path(application).resolve(strict=True)
    if not application.is_file() or not os.access(application, os.X_OK):
        raise ValueError("The separate QA client binary must be executable")
    driver = shutil.which(driver)
    if not driver:
        raise RuntimeError("Pinned tauri-driver was not found")
    artifacts = Path(artifacts).resolve()
    artifacts.mkdir(parents=True, exist_ok=True)
    report = {"status": "running", "driver": Path(driver).name, "commit": os.environ.get("GITHUB_SHA"), "checks": [],
              "build": "separate debug QA binary, qa-media-devices, bundled frontend",
              "capture": "synthetic WebKit devices", "consent": "WebDriver engine automation",
              "not_validated": ["physical microphone/camera/speakers", "GTK consent dialog", "release media runtime"],
              "ipc_mocked": False, "signaling_mocked": False, "webrtc_mocked": False}
    with application.open("rb") as stream:
        report["qa_application_sha256"] = hashlib.file_digest(stream, "sha256").hexdigest()
    def record(check):
        report["checks"].append(check)
        print("CODEX_NATIVE_MEDIA " + json.dumps({"check": check, "status": "passed"}), flush=True)
    try:
        with tempfile.TemporaryDirectory(prefix="liberte-media-qa-", dir=os.environ.get("RUNNER_TEMP")) as temporary:
            directory = Path(temporary)
            with NativeSession(application, driver, isolated_environment(directory / "alice"), artifacts, "media-alice") as alice, \
                 NativeSession(application, driver, isolated_environment(directory / "bob"), artifacts, "media-bob") as bob:
                identity_ui(alice, ALICE)
                identity_ui(bob, BOB)
                report["capture_states"] = {"initial": {"alice": capture_state(alice, {"microphone": "none", "camera": "none"}),
                                                       "bob": capture_state(bob, {"microphone": "none", "camera": "none"})}}
                try:
                    alice.invoke("get_sfu_connection", {"channelId": "invalid-on-purpose"})
                except RuntimeError as error:
                    if "Linux" not in str(error) or "P2P direct" not in str(error):
                        raise AssertionError("Native SFU must refuse the unsupported engine before parsing a channel") from error
                else:
                    raise AssertionError("Native Linux SFU unexpectedly supplied an E2EE connection")
                capture_state(alice, {"microphone": "none", "camera": "none"})
                record("Native SFU refuses the unsupported Linux E2EE engine before channel lookup or capture")
                alice.click('button[title="Cr\u00e9er un canal"]')
                alice.fill('input[placeholder="ex: general"]', CHANNEL)
                alice.click(xpath_text("button", "Cr\u00e9er"), "xpath")
                select_channel(alice)
                alice.click('button[title="Inviter dans ce canal"]')
                invite = until(lambda: alice.value('textarea[readonly]'), "real native media invitation")
                alice.click("//h3[starts-with(.,'Invitation -')]/../button", "xpath")
                bob.click("//button[span[normalize-space(.)='Rejoindre un canal']]", "xpath")
                bob.fill('textarea[placeholder="Collez le code d\'invitation ici..."]', invite)
                bob.click(xpath_text("button", "Rejoindre"), "xpath")
                select_channel(bob)
                network_ready(alice, bob)
                start_call(alice)
                start_call(bob)
                report["capture_states"]["audio_call"] = {"alice": capture_state(alice, {"microphone": "active"}),
                                                         "bob": capture_state(bob, {"microphone": "active"})}
                record("Both clients joined through UI and production Rust IPC/signaling")
                try:
                    for client in (alice, bob):
                        until(lambda: execute(client, INSTALL_PROBE), "remote audio stream for native observation", timeout=45)
                    baselines = {"alice": frames(snapshot(alice)), "bob": frames(snapshot(bob))}
                    for client in (alice, bob):
                        click_voice(client, "Activer la cam\u00e9ra")
                        client.wait_element(f'{VOICE} button[aria-label="D\u00e9sactiver la cam\u00e9ra"]')
                    observations = {}
                    for name, client in (("alice", alice), ("bob", bob)):
                        observations[name] = observe_until(client, lambda value, name=name: received_media(value, baselines[name]),
                                                          "received audio samples and progressing decoded video frames", timeout=60)
                        if client.invoke("get_call_state").get("isVideoEnabled") is not True:
                            raise AssertionError("Video UI and native state disagree")
                    report["received"] = observations
                    report["capture_states"]["video_call"] = {"alice": capture_state(alice, {"microphone": "active", "camera": "active"}),
                                                             "bob": capture_state(bob, {"microphone": "active", "camera": "active"})}
                    record("Bidirectional real WebRTC audio energy and decoded video frames in WebKit")
                    alice.screenshot("06-native-media-alice.png")
                    bob.screenshot("07-native-media-bob.png")
                    click_voice(alice, "Couper le microphone")
                    observe_until(alice, lambda value: bool([track for track in value["local"] if track["kind"] == "audio"])
                                  and all(not track["enabled"] for track in value["local"] if track["kind"] == "audio"),
                                  "Alice outgoing audio tracks disabled")
                    if alice.invoke("get_call_state").get("isMuted") is not True:
                        raise AssertionError("Mute UI and native state disagree")
                    silence = max(0.0001, observations["bob"]["maxRms"] * 0.02)
                    observe_until(bob, lambda value: value["remoteMutedIndicator"] and value["recentSamples"] >= 2
                                  and value["recentRms"] is not None and value["recentRms"] < silence,
                                  "Bob receives silence and sees Alice muted", timeout=30)
                    click_voice(alice, "Activer le microphone")
                    observe_until(bob, lambda value: not value["remoteMutedIndicator"] and value["recentRms"] is not None
                                  and value["recentRms"] > silence, "remote audio returns after unmute", timeout=30)
                    if alice.invoke("get_call_state").get("isMuted") is not False:
                        raise AssertionError("Unmute UI and native state disagree")
                    record("UI mute disabled outgoing tracks and silenced the receiver; unmute restored audio")
                    click_voice(alice, "D\u00e9sactiver la cam\u00e9ra")
                    observe_until(alice, lambda value: tracks_ended([track for track in value["heldLocal"] if track["kind"] == "video"])
                                  and not any(track["kind"] == "video" and track["readyState"] == "live" for track in value["local"]),
                                  "camera-off stops Alice video track")
                    observe_until(bob, lambda value: value["remoteCameraOff"], "Bob UI reflects Alice camera-off")
                    if alice.invoke("get_call_state").get("isVideoEnabled") is not False:
                        raise AssertionError("Camera-off UI and native state disagree")
                    report["capture_states"]["alice_camera_off"] = capture_state(alice, {"camera": "none"})
                    baseline = frames(snapshot(bob))
                    click_voice(alice, "Activer la cam\u00e9ra")
                    observe_until(bob, lambda value: not value["remoteCameraOff"] and frames(value) >= baseline + 2,
                                  "new video frames after camera is enabled again")
                    observe_until(alice, lambda value: any(track["kind"] == "video" and track["readyState"] == "live"
                                  for track in value["local"]), "the replacement local camera track is observed")
                    record("UI camera off stopped the exposed capture track; camera on resumed received frames")
                    report["capture_states"]["after_hangup"] = {}
                    for name, client in (("alice", alice), ("bob", bob)):
                        click_voice(client, "Quitter l'appel")
                        until(lambda: client.invoke("get_call_state").get("inCall") is False,
                              "native call state cleared")
                        observe_until(client, lambda value: tracks_ended(value["heldLocal"])
                                      and tracks_ended(value["heldRemote"]) and value["visibleVideos"] == 0,
                                      "leaving releases exposed local/remote tracks and removes video elements")
                        report["capture_states"]["after_hangup"][name] = capture_state(client, {"microphone": "none", "camera": "none"})
                    record("UI hangup cleared native state, ended observed tracks and stopped WebKit microphone/camera capture")
                finally:
                    for client in (alice, bob):
                        stop_probe(client)
        report["status"] = "passed"
    except BaseException as error:
        report["status"] = "failed"
        report["error"] = str(error)
        raise
    finally:
        (artifacts / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        print("CODEX_NATIVE_MEDIA_RESULT " + json.dumps(report), flush=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--application", required=True)
    parser.add_argument("--driver", default="tauri-driver")
    parser.add_argument("--artifacts", default=".ci-output/native-media")
    args = parser.parse_args()
    run_media(args.application, args.driver, args.artifacts)


if __name__ == "__main__":
    main()
