"""Real two-client Linux integration; no IPC/network mocks or application hooks.

UI: identities, channel/invitation/join, messages, file acceptance, server settings,
remote sync/restore. Production IPC: read-only diagnostics, reaction, file paths,
portable profile export/import (avoids OS pickers). All profiles/data are temporary.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request

from native_smoke import ELEMENT_KEY, NativeSession, NoRedirects, free_ports, isolated_environment, require_bundled_origin, until

CHANNEL = "native-integration"
ALICE = "Native Alice"
BOB = "Native Bob"
MESSAGE_A = "Alice to Bob through the real native network"
MESSAGE_B = "Bob to Alice through the real native network"
MESSAGE_RESTART = "Bob is back after native process restart"
MESSAGE_REPLY = "Alice confirms the restored native connection"
EMOJI = "\U0001f44d"
INPUT = 'textarea[placeholder="Envoyer un message chiffr\u00e9..."]'


def xpath_text(tag, text):
    # All selectors are fixed synthetic labels, never invitation/user input.
    if "'" in text:
        raise ValueError("XPath labels must not contain single quotes")
    return f"//{tag}[normalize-space(.)='{text}']"


def identity_ui(client, name):
    client.wait_element('input[placeholder="Ex: Kayfeer"]')
    require_bundled_origin(client.command("GET", "/url"))
    client.fill('input[placeholder="Ex: Kayfeer"]', name)
    client.click("//button[span[normalize-space(.)='Cr\u00e9er mon identit\u00e9']]", "xpath")
    client.wait_element(xpath_text("p", name), "xpath")
    client.assert_no_alert()


def select_channel(client):
    client.click(f"//button[span[normalize-space(.)='{CHANNEL}']]", "xpath")
    client.wait_element(INPUT)


def channel_id(client):
    channels = client.invoke("list_channels")
    matches = [item["id"] for item in channels if item.get("name") == CHANNEL]
    if len(matches) != 1:
        raise AssertionError("Expected one native integration channel")
    return matches[0]


def wait_message(client, message):
    def present():
        client.assert_no_alert()
        return client.find(xpath_text("p", message), "xpath")
    until(present, "message rendered by the receiving client", timeout=45)


def send_ui(client, message):
    client.fill(INPUT, message)
    element = client.wait_element(INPUT)
    client.command("POST", f"/element/{element}/value", {"text": "\ue007"})
    def sent():
        client.assert_no_alert()
        return client.value(INPUT) == ""
    until(sent, "message accepted by the real native IPC", timeout=40)
    wait_message(client, message)


def network_ready(alice, bob, timeout=65):
    latest = []
    def connected():
        a, b = alice.invoke("get_network_status"), bob.invoke("get_network_status")
        latest[:] = [{"peers": len(state.get("peers", [])), "mode": state.get("connectionMode"),
                      "listenAddresses": len(state.get("listenAddresses", [])), "error": state.get("error")}
                     for state in (a, b)]
        return (a.get("localPeerId") and b.get("localPeerId")
                and b["localPeerId"] in a.get("peers", [])
                and a["localPeerId"] in b.get("peers", []))
    try:
        until(connected, "mutual real P2P connection", timeout=timeout)
    except TimeoutError as error:
        raise TimeoutError(f"Mutual P2P connection timed out; Alice/Bob status: {latest}") from error
    for client in (alice, bob):
        client.wait_element(xpath_text("span", "1 pair connect\u00e9"), "xpath")


def wait_transfer(client, channel, file_id, expected):
    def done():
        transfers = client.invoke("list_channel_files", {"channelId": channel})
        selected = next((item for item in transfers if item.get("fileId") == file_id), None)
        return transfer_has_status(selected, expected)
    until(done, f"file transfer {expected}", timeout=90)


def transfer_has_status(transfer, expected):
    if transfer is None:
        return False
    if transfer.get("status") == "failed" or transfer.get("error"):
        raise AssertionError(f"Real file transfer failed: {transfer.get('error', 'unknown error')}")
    return transfer.get("status") == expected


def verify_file(source, destination):
    expected = hashlib.sha256(Path(source).read_bytes()).hexdigest()
    actual = hashlib.sha256(Path(destination).read_bytes()).hexdigest()
    if expected != actual or Path(source).stat().st_size != Path(destination).stat().st_size:
        raise AssertionError("Saved file differs from the original transmitted bytes")
    return expected


def settings_ui(client):
    client.click("//button[span[normalize-space(.)='Param\u00e8tres']]", "xpath")
    client.wait_element('section[aria-label="R\u00e9seau"]')


def leave_settings_ui(client):
    client.click("//button[span[normalize-space(.)='Param\u00e8tres']]", "xpath")
    select_channel(client)


def configure_server_ui(client, base):
    settings_ui(client)
    client.fill('input[placeholder="https://votre-serveur.example"]', base)
    client.click(xpath_text("button", "V\u00e9rifier et enregistrer le serveur"), "xpath")
    client.wait_element("//p[@role='status' and starts-with(.,'Serveur enregistr\u00e9 :')]", "xpath")
    client.assert_no_alert()
    if client.invoke("get_settings").get("serverUrl") != base:
        raise AssertionError("The server origin selected through the UI was not persisted")
    leave_settings_ui(client)


def remote_backup_result(state, success_text):
    """Success requires the real UI acknowledgement; errors fail immediately."""
    if not isinstance(state, dict) or not state.get("present"):
        raise AssertionError("The remote backup controls disappeared during the operation")
    errors = [str(error).strip() for error in state.get("errors", []) if str(error).strip()]
    if errors:
        raise AssertionError("Remote backup UI reported: " + "; ".join(errors))
    return any(success_text in str(status) for status in state.get("statuses", []))


def remote_backup_ui(client, label, success_text):
    # Observe/scroll the shipped DOM, then perform exactly one native click.
    # Device enumeration can change the settings layout after the section mounts.
    selector = xpath_text("button", label)
    element = client.wait_element(selector, "xpath")
    client.command("POST", "/execute/sync", {
        "script": "arguments[0].scrollIntoView({block:'center',inline:'nearest',behavior:'instant'}); return null;",
        "args": [{ELEMENT_KEY: urllib.parse.unquote(element)}],
    })
    previous = None
    stable = 0
    def positioned():
        nonlocal previous, stable
        position = client.command("POST", "/execute/sync", {
            "script": """
                const button = arguments[0], box = button.getBoundingClientRect();
                const x = box.left + box.width / 2, y = box.top + box.height / 2;
                const hit = document.elementFromPoint(x, y);
                const ready = !button.disabled && x > 0 && y > 0 && x < innerWidth && y < innerHeight
                    && !!hit && (hit === button || button.contains(hit));
                if (!ready) button.scrollIntoView({block:'center',inline:'nearest',behavior:'instant'});
                return {x: Math.round(x), y: Math.round(y), ready};
            """,
            "args": [{ELEMENT_KEY: urllib.parse.unquote(element)}],
        })
        stable = stable + 1 if position.get("ready") and position == previous else 0
        previous = position
        return stable >= 2
    until(positioned, "stable visible remote backup action", timeout=10)
    client.command("POST", f"/element/{element}/click", {})

    latest = {}
    acknowledged = False
    started = time.monotonic()
    def completed():
        nonlocal latest, acknowledged
        latest = client.command("POST", "/execute/sync", {
            "script": """
                const root = document.querySelector('section[aria-label="Sauvegarde distante"]');
                if (!root) return {present:false};
                return {present:true, busy:root.getAttribute('aria-busy') === 'true',
                    buttons:[...root.querySelectorAll('button')].map(button => button.textContent.trim()),
                    statuses:[...root.querySelectorAll('[role="status"]')].map(node => node.textContent.trim()),
                    errors:[...root.querySelectorAll('[role="alert"]')].map(node => node.textContent.trim())};
            """,
            "args": [],
        })
        if remote_backup_result(latest, success_text):
            return True
        acknowledged = acknowledged or latest.get("busy", False)
        if not acknowledged and time.monotonic() - started > 5:
            raise AssertionError(f"Native backup click was not acknowledged; UI state: {latest}")
        return False
    try:
        # Match the production HTTP timeout (90s) plus bounded local import/refresh.
        until(completed, "remote backup UI acknowledgement", timeout=110)
    except BaseException:
        print("CODEX_REMOTE_BACKUP_UI " + json.dumps({"client": client.label, "state": latest}), flush=True)
        raise


class LocalServer:
    def __init__(self, binary, directory, artifacts):
        self.binary, self.directory, self.artifacts = binary, directory, artifacts
        self.process = self.log = None
        self.port = free_ports()[0]
        self.base = f"http://127.0.0.1:{self.port}"
        self.storage = directory / "server-storage"

    def __enter__(self):
        env = os.environ.copy()
        env.update(HTTP_ADDR=f"127.0.0.1:{self.port}", LISTEN_ADDR="/ip4/127.0.0.1/udp/0/quic-v1",
                   BLOB_STORAGE_PATH=str(self.storage), PREMIUM_REQUIRED="false", REGISTRATION_OPEN="true",
                   ADMIN_TOKEN=secrets.token_hex(32), INSTANCE_NAME="Native integration CI")
        self.log = (self.artifacts / "server.log").open("wb")
        try:
            self.process = subprocess.Popen([str(self.binary)], env=env, stdout=self.log, stderr=self.log)
            opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirects())
            deadline = time.monotonic() + 40
            while True:
                if self.process.poll() is not None:
                    raise RuntimeError(f"Integration server exited: {self.process.returncode}")
                try:
                    with opener.open(self.base + "/health", timeout=2) as response:
                        if response.status == 200:
                            return self
                except (OSError, urllib.error.URLError):
                    pass
                if time.monotonic() >= deadline:
                    raise TimeoutError("Real integration server did not become healthy")
                time.sleep(0.2)
        except BaseException:
            self.__exit__(None, None, None)
            raise

    def __exit__(self, *_):
        if self.process:
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=5)
            self.process = None
        if self.log:
            self.log.close()
            self.log = None


def run_integration(application, server_binary, driver, artifacts):
    if sys.platform != "linux":
        raise RuntimeError("Native integration requires Linux WebKitGTK")
    application, server_binary = Path(application).resolve(strict=True), Path(server_binary).resolve(strict=True)
    for binary in (application, server_binary):
        if not binary.is_file() or not os.access(binary, os.X_OK):
            raise ValueError("Both native client and server must be executable files")
    driver = shutil.which(driver)
    if not driver:
        raise RuntimeError("Pinned tauri-driver executable was not found")
    artifacts = Path(artifacts).resolve()
    artifacts.mkdir(parents=True, exist_ok=True)
    report = {"status": "running", "driver": Path(driver).name, "commit": os.environ.get("GITHUB_SHA"), "mocks": False,
              "checks": [], "ui": ["identity", "channel", "invite", "join", "messages", "file acceptance", "server settings", "backup sync/restore"],
              "ipc": ["read-only status", "reaction", "file send/save paths", "portable profile export/import"]}
    for name, path in (("application", application), ("server", server_binary)):
        with path.open("rb") as stream:
            report[name + "_sha256"] = hashlib.file_digest(stream, "sha256").hexdigest()
    def record(check):
        report["checks"].append(check)
        print("CODEX_NATIVE_INTEGRATION " + json.dumps({"check": check, "status": "passed"}), flush=True)
    try:
        with tempfile.TemporaryDirectory(prefix="liberte-integration-", dir=os.environ.get("RUNNER_TEMP")) as temporary:
            directory = Path(temporary)
            alice_env = isolated_environment(directory / "alice")
            bob_env = isolated_environment(directory / "bob")
            restored_env = isolated_environment(directory / "restored")
            with LocalServer(server_binary, directory, artifacts) as server, NativeSession(application, driver, alice_env, artifacts, "alice") as alice:
                identity_ui(alice, ALICE)
                alice.click('button[title="Cr\u00e9er un canal"]')
                alice.fill('input[placeholder="ex: general"]', CHANNEL)
                alice.click(xpath_text("button", "Cr\u00e9er"), "xpath")
                select_channel(alice)
                alice_id = channel_id(alice)
                # This must work before restart: it exercises the createChannel DTO/key integration.
                alice.click('button[title="Inviter dans ce canal"]')
                invite = until(lambda: alice.value('textarea[readonly]'), "real signed invitation")
                alice.click("//h3[starts-with(.,'Invitation -')]/../button", "xpath")
                with NativeSession(application, driver, bob_env, artifacts, "bob") as bob:
                    identity_ui(bob, BOB)
                    bob.click("//button[span[normalize-space(.)='Rejoindre un canal']]", "xpath")
                    bob.fill('textarea[placeholder="Collez le code d\'invitation ici..."]', invite)
                    bob.click(xpath_text("button", "Rejoindre"), "xpath")
                    select_channel(bob)
                    if channel_id(bob) != alice_id:
                        raise AssertionError("The invitation joined a different channel")
                    network_ready(alice, bob)
                    record("UI identities/channel/invitation/join and mutual real P2P connection")
                    configure_server_ui(bob, server.base)
                    # Export before any message. A later remote restore must supply the history.
                    password = secrets.token_urlsafe(24)
                    portable = bob.invoke("export_profile", {"password": password})
                    if not isinstance(portable, str) or "liberte-encrypted-backup" not in portable:
                        raise AssertionError("Portable profile export did not return an encrypted container")
                    send_ui(alice, MESSAGE_A)
                    wait_message(bob, MESSAGE_A)
                    send_ui(bob, MESSAGE_B)
                    wait_message(alice, MESSAGE_B)
                    record("Messages sent through UI and received/decrypted/rendered in both directions")
                    messages = bob.invoke("search_messages", {"query": MESSAGE_A, "channelId": alice_id})
                    matches = [item for item in messages if item.get("content") == MESSAGE_A]
                    if len(matches) != 1:
                        raise AssertionError("Expected one received message to react to")
                    bob.invoke("add_reaction", {"channelId": alice_id, "messageId": matches[0]["id"], "emoji": EMOJI})
                    alice.wait_element(f"//button[span[normalize-space(.)='{EMOJI}'] and span[normalize-space(.)='1']]", "xpath")
                    record("Real reaction IPC published to peer and rendered in Alice UI")
                    original = directory / "multi-chunk-native.bin"
                    original.write_bytes(bytes(range(256)) * 4096 + b"native-transfer-tail")
                    sent = alice.invoke("send_file", {"channelId": alice_id, "filePath": str(original)})
                    file_id = sent["fileId"]
                    if sent.get("fileSize") != original.stat().st_size:
                        raise AssertionError("Native sender reported the wrong file length")
                    bob.wait_element(xpath_text("p", original.name), "xpath")
                    bob.click(xpath_text("button", "Recevoir"), "xpath")
                    wait_transfer(bob, alice_id, file_id, "received")
                    bob.wait_element(xpath_text("button", "Enregistrer"), "xpath")
                    received = directory / "received-native.bin"
                    bob.invoke("save_received_file", {"fileId": file_id, "filePath": str(received)})
                    report["transferred_bytes"] = original.stat().st_size
                    report["transferred_sha256"] = verify_file(original, received)
                    wait_transfer(alice, alice_id, file_id, "completed")
                    record("Multi-chunk file: real IPC send/save, UI acceptance, exact bytes and sender ACK")
                    bob.assert_no_alert()
                    bob.screenshot("03-bob-messages-file.png")
                # Alice remains alive; Bob must reload history and reconnect without another invitation/dial.
                with NativeSession(application, driver, bob_env, artifacts, "bob-restarted") as bob:
                    bob.wait_element(xpath_text("p", BOB), "xpath")
                    select_channel(bob)
                    wait_message(bob, MESSAGE_A)
                    wait_message(bob, MESSAGE_B)
                    bob.wait_element(f"//button[span[normalize-space(.)='{EMOJI}'] and span[normalize-space(.)='1']]", "xpath")
                    network_ready(alice, bob)
                    send_ui(bob, MESSAGE_RESTART)
                    wait_message(alice, MESSAGE_RESTART)
                    send_ui(alice, MESSAGE_REPLY)
                    wait_message(bob, MESSAGE_REPLY)
                    record("Bob process restart preserved history/reaction and automatically reconnected for new messages")
                    settings_ui(bob)
                    remote_backup_ui(bob, "Synchroniser maintenant", "Sauvegarde chiffr\u00e9e synchronis\u00e9e")
                    bob.assert_no_alert()
                    stored = list(server.storage.glob("users/*/backup.enc"))
                    if len(stored) != 1 or stored[0].stat().st_size == 0:
                        raise AssertionError("The real server did not persist the encrypted remote backup")
                    if MESSAGE_A.encode() in stored[0].read_bytes():
                        raise AssertionError("Remote backup exposed message plaintext")
                    record("UI remote backup sync persisted ciphertext on the real server")
                with NativeSession(application, driver, restored_env, artifacts, "bob-restored") as restored:
                    restored.wait_element('input[placeholder="Ex: Kayfeer"]')
                    require_bundled_origin(restored.command("GET", "/url"))
                    restored.invoke("import_profile", {"json": portable, "password": password, "allowLegacy": False})
                    restored.command("POST", "/refresh", {})
                    restored.wait_element(xpath_text("p", BOB), "xpath")
                    select_channel(restored)
                    if restored.invoke("get_settings").get("serverUrl") != server.base:
                        raise AssertionError("Portable profile lost the selected server URL")
                    if restored.invoke("search_messages", {"query": MESSAGE_A, "channelId": alice_id}):
                        raise AssertionError("Pre-message portable profile unexpectedly already contained remote history")
                    settings_ui(restored)
                    remote_backup_ui(restored, "Restaurer depuis le serveur", "messages restaur\u00e9s depuis le serveur")
                    restored.assert_no_alert()
                    leave_settings_ui(restored)
                    for message in (MESSAGE_A, MESSAGE_B, MESSAGE_RESTART, MESSAGE_REPLY):
                        wait_message(restored, message)
                    restored.wait_element(f"//button[span[normalize-space(.)='{EMOJI}'] and span[normalize-space(.)='1']]", "xpath")
                    restored.screenshot("04-bob-remote-restored.png")
                    record("Fresh profile import restored identity/server URL; UI remote restore recovered all messages and reaction")
                alice.assert_no_alert()
                alice.screenshot("05-alice-peer-history.png")
        report["status"] = "passed"
    except BaseException as error:
        report["status"] = "failed"
        report["error"] = str(error)
        raise
    finally:
        (artifacts / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        # Deliberately excludes invites, profile archives/passwords, identity/channel keys and server admin token.
        print("CODEX_NATIVE_INTEGRATION_RESULT " + json.dumps(report), flush=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--application", required=True)
    parser.add_argument("--server", required=True)
    parser.add_argument("--driver", default="tauri-driver")
    parser.add_argument("--artifacts", default=".ci-output/native-integration")
    args = parser.parse_args()
    run_integration(args.application, args.server, args.driver, args.artifacts)


if __name__ == "__main__":
    main()
