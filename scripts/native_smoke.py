"""Exercise the real Linux Tauri/WebKit client through external W3C WebDriver.

No frontend/IPC mocks, application hooks, or production permission changes.
The generated identity lives only in a temporary XDG profile, never in artifacts.
"""
from __future__ import annotations

import argparse
import base64
import hashlib
import http.client
import json
import os
import re
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request

ELEMENT_KEY = "element-6066-11e4-a52e-4f735466cecf"
TRANSIENT_ERRORS = {"no such element", "stale element reference", "element not interactable"}


class WebDriverError(RuntimeError):
    def __init__(self, code, message):
        self.code = code
        super().__init__(f"WebDriver {code}: {message}")


def decode_response(status, body):
    try:
        response = json.loads(body)
    except (ValueError, UnicodeError) as error:
        raise RuntimeError(f"WebDriver returned invalid JSON (HTTP {status})") from error
    if not isinstance(response, dict) or "value" not in response:
        raise RuntimeError("WebDriver response has no W3C value")
    value = response["value"]
    if isinstance(value, dict) and value.get("error"):
        raise WebDriverError(value["error"], value.get("message", ""))
    if status >= 400:
        raise RuntimeError(f"WebDriver request failed with HTTP {status}")
    return value


def require_bundled_origin(url):
    parsed = urllib.parse.urlsplit(url)
    if parsed.scheme != "tauri" or parsed.netloc != "localhost":
        raise RuntimeError(f"Expected the bundled Linux Tauri frontend, received {url!r}")


def until(action, description, timeout=30):
    deadline = time.monotonic() + timeout
    last_error = None
    while True:
        try:
            result = action()
            if result:
                return result
        except WebDriverError as error:
            if error.code not in TRANSIENT_ERRORS:
                raise
            last_error = error
        if time.monotonic() >= deadline:
            raise TimeoutError(f"Timed out waiting for {description}; last error: {last_error}")
        time.sleep(0.2)


def sanitize_diagnostic_tail(raw):
    """Keep bounded diagnostics, excluding credentials and opaque payloads."""
    raw = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", raw)
    lines = []
    for line in raw.splitlines()[-120:]:
        if re.search(r"secret|password|token|invitation|invite|channel.?key|key_hex|private|ciphertext|encrypted|archive|backup", line, re.IGNORECASE):
            lines.append("[sensitive diagnostic line omitted]")
            continue
        # Includes hexadecimal identities, bearer invites, base64 archives and URLs.
        line = re.sub(r"[A-Za-z0-9_+/=.-]{32,}", "[opaque value omitted]", line)
        lines.append(line[:2000])
    return "\n".join(lines)[-12000:]


def decode_ipc_result(name, result):
    if not isinstance(result, dict) or result.get("ok") is not True or "value" not in result:
        error = result.get("error", "Invalid IPC response") if isinstance(result, dict) else "Invalid IPC response"
        raise RuntimeError(f"Native IPC {name} failed: {error}")
    return result["value"]


def isolated_environment(directory):
    env = os.environ.copy()
    for variable, folder in (("XDG_DATA_HOME", "data"), ("XDG_CONFIG_HOME", "config"),
                             ("XDG_CACHE_HOME", "cache"), ("XDG_RUNTIME_DIR", "runtime")):
        path = Path(directory) / folder
        path.mkdir(mode=0o700, parents=True, exist_ok=True)
        env[variable] = str(path)
    return env


def free_ports():
    with socket.socket() as first, socket.socket() as second:
        first.bind(("127.0.0.1", 0))
        second.bind(("127.0.0.1", 0))
        return first.getsockname()[1], second.getsockname()[1]


class NoRedirects(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise RuntimeError("WebDriver must not redirect away from its loopback endpoint")


class NativeSession:
    def __init__(self, application, driver, env, artifacts, label):
        self.application = application
        self.driver = driver
        self.env = env
        self.artifacts = artifacts
        self.label = label
        self.process = None
        self.log = None
        self.session = None
        self.port, self.native_port = free_ports()
        self.opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirects())

    def request(self, method, path, data=None, timeout=20):
        request = urllib.request.Request(
            f"http://127.0.0.1:{self.port}{path}", method=method,
            data=json.dumps(data).encode() if data is not None else None,
            headers={"Content-Type": "application/json"},
        )
        try:
            with self.opener.open(request, timeout=timeout) as response:
                return decode_response(response.status, response.read())
        except urllib.error.HTTPError as error:
            try:
                return decode_response(error.code, error.read())
            finally:
                error.close()

    def command(self, method, path, data=None, timeout=20):
        if not self.session:
            raise RuntimeError("No native WebDriver session was established")
        # A read can safely be repeated after a dropped control connection.
        # Clicks, typing, scripts/IPC and all other mutations are never retried.
        read_only = method == "GET" or (method == "POST" and path in {"/element", "/elements"})
        for attempt in range(3):
            try:
                return self.request(method, f"/session/{urllib.parse.quote(self.session, safe='')}{path}", data, timeout=timeout)
            except (http.client.RemoteDisconnected, ConnectionResetError, BrokenPipeError):
                if not read_only or attempt == 2:
                    raise
                print(f"Native WebDriver read transport interrupted; retry {attempt + 1}/2 ({method} {path.split('/')[1]})", flush=True)
                time.sleep(0.15 * (attempt + 1))

    def __enter__(self):
        self.log = (self.artifacts / f"{self.label}-driver.log").open("wb")
        try:
            native = Path(self.driver).name == "WebKitWebDriver"
            environment = self.env.copy()
            if native:
                # Exact Linux mapping used by tauri-driver 2.1.0, without its HTTP proxy.
                environment.update(TAURI_AUTOMATION="true", TAURI_WEBVIEW_AUTOMATION="true")
                command = [self.driver, f"--port={self.port}", "--host=127.0.0.1"]
                options = {"webkitgtk:browserOptions": {"binary": str(self.application), "args": []}}
            else:
                command = [self.driver, "--port", str(self.port), "--native-port", str(self.native_port),
                           "--native-host", "127.0.0.1"]
                options = {"tauri:options": {"application": str(self.application)}}
            self.process = subprocess.Popen(
                command, env=environment, stdout=self.log, stderr=subprocess.STDOUT, start_new_session=True,
            )
            deadline = time.monotonic() + 30
            while True:
                if self.process.poll() is not None:
                    raise RuntimeError(f"Native WebDriver exited with status {self.process.returncode}")
                try:
                    status = self.request("GET", "/status", timeout=2)
                    if isinstance(status, dict) and status.get("ready"):
                        break
                except (OSError, urllib.error.URLError):
                    pass
                if time.monotonic() >= deadline:
                    raise TimeoutError("Native WebDriver did not become ready")
                time.sleep(0.2)
            created = self.request("POST", "/session", {"capabilities": {"alwaysMatch": {
                "browserName": "wry", **options,
            }}}, timeout=45)
            if not isinstance(created, dict) or not created.get("sessionId"):
                raise RuntimeError("WebDriver did not return a native session identifier")
            self.session = created["sessionId"]
            self.command("POST", "/timeouts", {"implicit": 0, "pageLoad": 30000, "script": 10000})
            return self
        except BaseException:
            self.close(diagnostics=True)
            raise

    def find(self, selector, using="css selector"):
        value = self.command("POST", "/element", {"using": using, "value": selector})
        if not isinstance(value, dict) or ELEMENT_KEY not in value:
            raise RuntimeError("Native WebDriver did not return a W3C element")
        return urllib.parse.quote(value[ELEMENT_KEY], safe="")

    def wait_element(self, selector, using="css selector"):
        return until(lambda: self.find(selector, using), selector)

    def click(self, selector, using="css selector"):
        element = self.wait_element(selector, using)
        self.command("POST", f"/element/{element}/click", {})

    def fill(self, selector, text):
        element = self.wait_element(selector)
        self.command("POST", f"/element/{element}/value", {"text": text})

    def value(self, selector):
        element = self.wait_element(selector)
        return self.command("GET", f"/element/{element}/property/value")

    def invoke(self, name, arguments=None):
        """Call shipped IPC. Only audited read commands may be retried."""
        read_only = name in {
            "get_network_status", "list_channels", "list_channel_files", "get_settings",
            "search_messages", "get_call_state", "qa_media_capture_state",
        }
        payload = {
            "script": """
                const name = arguments[0], args = arguments[1];
                const done = arguments[arguments.length - 1];
                Promise.resolve().then(() => window.__TAURI_INTERNALS__.invoke(name, args))
                    .then(value => done({ok: true, value: value === undefined ? null : value}),
                          error => done({ok: false, error: String(error)}));
            """,
            "args": [name, arguments or {}],
        }
        for attempt in range(3):
            try:
                self.command("POST", "/timeouts", {"script": 60000})
                result = self.command("POST", "/execute/async", payload, timeout=65)
                return decode_ipc_result(name, result)
            except (http.client.RemoteDisconnected, ConnectionResetError, BrokenPipeError):
                if not read_only or attempt == 2:
                    raise
                print(f"Native IPC read transport interrupted; retry {attempt + 1}/2 ({name})", flush=True)
                time.sleep(0.15 * (attempt + 1))

    def assert_no_alert(self):
        elements = self.command("POST", "/elements", {"using": "css selector", "value": '[role="alert"]'})
        if not isinstance(elements, list):
            raise RuntimeError("Native WebDriver did not return an element list")
        for element in elements:
            text = self.command("GET", f"/element/{element[ELEMENT_KEY]}/text")
            if text and str(text).strip():
                raise AssertionError(f"The native application reported an error: {text}")

    def screenshot(self, name):
        encoded = self.command("GET", "/screenshot")
        data = base64.b64decode(encoded, validate=True)
        if not data.startswith(b"\x89PNG\r\n\x1a\n"):
            raise RuntimeError("Native screenshot is not a PNG image")
        (self.artifacts / name).write_bytes(data)
        # Only screenshots from this harness's disposable synthetic profile.
        print("CODEX_NATIVE_SCREENSHOT " + json.dumps({
            "name": name, "sha256": hashlib.sha256(data).hexdigest(),
            "base64": base64.b64encode(data).decode("ascii"),
        }), flush=True)

    def close(self, diagnostics=False):
        alive = self.process is not None and self.process.poll() is None
        if self.session:
            try:
                self.command("DELETE", "")
            except Exception as error:
                print(f"WebDriver session cleanup: {error}", file=sys.stderr)
            self.session = None
        if self.process:
            # Only this harness's own process group, started with a new session.
            try:
                os.killpg(self.process.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                pass
            try:
                os.killpg(self.process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            self.process.wait(timeout=5)
            self.process = None
        if self.log:
            path = Path(self.log.name)
            self.log.close()
            self.log = None
            with path.open("rb") as source:
                source.seek(max(0, path.stat().st_size - 65536))
                tail = sanitize_diagnostic_tail(source.read().decode("utf-8", errors="replace"))
            path.write_bytes((tail + "\n").encode("utf-8"))
            if diagnostics:
                print("CODEX_NATIVE_DIAGNOSTIC " + json.dumps({
                    "client": self.label, "driverAliveBeforeCleanup": alive, "tail": tail,
                }), flush=True)

    def __exit__(self, error_type, error, traceback):
        if error is not None and self.session:
            try:
                self.screenshot(f"{self.label}-failure.png")
            except Exception as capture_error:
                print(f"Could not capture native failure screenshot: {capture_error}", file=sys.stderr)
        self.close(diagnostics=error is not None)


def run_smoke(application, driver, artifacts):
    application = Path(application).resolve(strict=True)
    if not application.is_file() or not os.access(application, os.X_OK):
        raise ValueError("The native application must be an executable file")
    if sys.platform != "linux":
        raise RuntimeError("This smoke test requires the Linux WebKitGTK runtime")
    driver = shutil.which(driver)
    if not driver:
        raise RuntimeError("Pinned tauri-driver executable was not found")
    artifacts = Path(artifacts).resolve()
    artifacts.mkdir(parents=True, exist_ok=True)
    with application.open("rb") as source:
        binary_sha = hashlib.file_digest(source, "sha256").hexdigest()
    report = {"status": "running", "commit": os.environ.get("GITHUB_SHA"),
              "application_sha256": binary_sha, "runtime": "Tauri Linux/WebKitGTK", "driver": Path(driver).name,
              "checks": [], "mocks": False}
    try:
        with tempfile.TemporaryDirectory(prefix="liberte-native-", dir=os.environ.get("RUNNER_TEMP")) as temporary:
            env = os.environ.copy()
            for variable, folder in (("XDG_DATA_HOME", "data"), ("XDG_CONFIG_HOME", "config"),
                                     ("XDG_CACHE_HOME", "cache"), ("XDG_RUNTIME_DIR", "runtime")):
                path = Path(temporary) / folder
                path.mkdir(mode=0o700)
                env[variable] = str(path)
            display_name = "Native CI identity"
            channel_name = "native-ci-persistence"
            identity_xpath = f"//p[normalize-space(.)='{display_name}']"
            channel_xpath = f"//button[span[normalize-space(.)='{channel_name}']]"
            with NativeSession(application, driver, env, artifacts, "created") as client:
                client.wait_element('input[placeholder="Ex: Kayfeer"]')
                require_bundled_origin(client.command("GET", "/url"))
                client.fill('input[placeholder="Ex: Kayfeer"]', display_name)
                client.click("//button[span[normalize-space(.)='Créer mon identité']]", "xpath")
                client.wait_element(identity_xpath, "xpath")
                client.click('button[title="Créer un canal"]')
                client.fill('input[placeholder="ex: general"]', channel_name)
                client.click("//button[normalize-space(.)='Créer']", "xpath")
                client.wait_element(channel_xpath, "xpath")
                client.assert_no_alert()
                client.screenshot("01-created.png")
                report["checks"].append("Identity and channel created through real UI and IPC")
            with NativeSession(application, driver, env, artifacts, "restarted") as client:
                client.wait_element(identity_xpath, "xpath")
                client.wait_element(channel_xpath, "xpath")
                require_bundled_origin(client.command("GET", "/url"))
                client.assert_no_alert()
                client.screenshot("02-restarted.png")
                report["checks"].append("Identity and channel restored after a fresh process launch")
        report["status"] = "passed"
        print("Native Tauri smoke passed: bundled frontend, identity, channel and restart persistence; no IPC mocks")
    except BaseException as error:
        report["status"] = "failed"
        report["error"] = str(error)
        raise
    finally:
        (artifacts / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--application", required=True)
    parser.add_argument("--driver", default="tauri-driver")
    parser.add_argument("--artifacts", default=".ci-output/native-smoke")
    arguments = parser.parse_args()
    run_smoke(arguments.application, arguments.driver, arguments.artifacts)


if __name__ == "__main__":
    main()
