"""CI helpers. No deployment, package installation or external API calls."""
from __future__ import annotations

import argparse
import base64
import hashlib
import gzip
import json
import os
from pathlib import Path
import secrets
import shutil
import socket
import subprocess
import tarfile
import tempfile
import time
import tomllib
import urllib.error
import urllib.parse
import urllib.request
import uuid
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def run(args, **kwargs):
    return subprocess.run(args, check=True, **kwargs)


def read_json(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def write_json(path, value):
    Path(path).write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def versions(root=ROOT, tag=None):
    workspace = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    version = workspace["workspace"]["package"]["version"]
    frontend = read_json(root / "frontend/package.json")["version"]
    tauri = read_json(root / "crates/liberte-client/tauri.conf.json")["version"]
    if len({version, frontend, tauri}) != 1:
        raise ValueError(f"Version mismatch: Rust={version}, frontend={frontend}, Tauri={tauri}")
    if tag is not None and tag != f"v{version}":
        raise ValueError(f"The release tag must be v{version}, received {tag!r}")
    return version


def lockfile():
    path = ROOT / "Cargo.lock"
    if not path.is_file():
        print("::warning::Cargo.lock is missing. Generating a candidate for review; this is not a reproducible release.")
        run(["cargo", "generate-lockfile"], cwd=ROOT)
        export_file(path, "Cargo.lock")
    run(["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"],
        cwd=ROOT, stdout=subprocess.DEVNULL)
    if not path.is_file() or path.stat().st_size == 0:
        raise ValueError("Cargo.lock was not created")


def export_file(path, label):
    """A recoverable log for non-secret generated dependency/format artifacts only."""
    data = Path(path).read_bytes()
    print("CODEX_BOOTSTRAP_FILE " + json.dumps({
        "path": label, "sha256": hashlib.sha256(data).hexdigest(),
        "gzip_base64": base64.b64encode(gzip.compress(data, mtime=0)).decode("ascii"),
    }), flush=True)


def npm_lock_matches(package, lock):
    locked = lock.get("packages", {}).get("")
    if not isinstance(locked, dict) or lock.get("lockfileVersion") != 3:
        return False
    return all(package.get(key, {}) == locked.get(key, {}) for key in (
        "dependencies", "devDependencies", "optionalDependencies"
    )) and package.get("version") == locked.get("version")


def frontend_lock():
    directory = ROOT / "frontend"
    package = read_json(directory / "package.json")
    path = directory / "package-lock.json"
    old = read_json(path) if path.is_file() else {}
    if not npm_lock_matches(package, old):
        print("::warning::Frontend manifest and lockfile differ. Generating a candidate for review.")
        run(["npm", "install", "--package-lock-only", "--ignore-scripts"], cwd=directory)
        if not npm_lock_matches(package, read_json(path)):
            raise ValueError("npm did not produce a matching lockfile")
        export_file(path, "frontend/package-lock.json")


def format_patch():
    """Capture rustfmt changes but test the original source, without weakening the gate."""
    check = subprocess.run(["cargo", "fmt", "--all", "--", "--check"], cwd=ROOT, check=False)
    if check.returncode == 0:
        return
    run(["git", "diff", "--quiet", "--", "*.rs"], cwd=ROOT)
    paths = subprocess.check_output(["git", "ls-files", "-z", "--", "*.rs"], cwd=ROOT).decode().split("\0")
    originals = {ROOT / name: (ROOT / name).read_bytes() for name in paths if name}
    output = ROOT / ".ci-output"
    output.mkdir(exist_ok=True)
    try:
        run(["cargo", "fmt", "--all"], cwd=ROOT)
        patch = subprocess.check_output(["git", "diff", "--binary", "--", "*.rs"], cwd=ROOT)
        (output / "rustfmt.patch").write_bytes(patch)
        if patch:
            export_file(output / "rustfmt.patch", "rustfmt.patch")
    finally:
        for path, data in originals.items():
            path.write_bytes(data)
    raise RuntimeError("Rust formatting failed. Apply and commit the rustfmt.patch artifact, then rerun CI.")


def updater_mode(private_key, public_key):
    if bool(private_key.strip()) != bool(public_key.strip()):
        raise ValueError("Set both TAURI_SIGNING_PRIVATE_KEY and TAURI_UPDATER_PUBLIC_KEY, or neither")
    if public_key:
        decoded = base64.b64decode(public_key.strip(), validate=True).decode("utf-8")
        lines = decoded.splitlines()
        if len(lines) < 2 or not lines[0].startswith("untrusted comment:"):
            raise ValueError("TAURI_UPDATER_PUBLIC_KEY must be the base64 Tauri public key")
        key = base64.b64decode(lines[1], validate=True)
        if len(key) != 42 or key[:2] != b"Ed":
            raise ValueError("Invalid Minisign Ed25519 public key")
    return bool(private_key.strip())


def prepare_updater():
    private = os.environ.get("TAURI_SIGNING_PRIVATE_KEY", "")
    public = os.environ.get("TAURI_UPDATER_PUBLIC_KEY", "").strip()
    signed = updater_mode(private, public)
    # The workflow already builds the frontend from its actual directory.
    # Only public configuration is written; the private key stays in the environment.
    override = {
        "build": {"beforeBuildCommand": ""},
        "bundle": {"createUpdaterArtifacts": signed},
        "plugins": {"updater": {"pubkey": public, "endpoints": (
            ["https://github.com/Kayfeer/liberte/releases/latest/download/latest.json"] if signed else []
        )}},
    }
    write_json(ROOT / "crates/liberte-client/tauri-ci.conf.json", override)
    print("Updater signing enabled" if signed else "Unsigned installers: automatic updates disabled")
    if os.environ.get("GITHUB_OUTPUT"):
        with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
            output.write(f"signed={str(signed).lower()}\n")


def collect_client(target, label):
    bundle = ROOT / "target" / target / "release/bundle"
    destination = ROOT / "release-assets"
    destination.mkdir(exist_ok=True)
    expected = ("appimage/*.AppImage", "deb/*.deb", "rpm/*.rpm") if label.startswith("linux") else ("nsis/*setup.exe", "msi/*.msi")
    selected = []
    for pattern in expected:
        matches = list(bundle.glob(pattern))
        if len(matches) != 1:
            raise ValueError(f"Expected one bundle for {pattern}, found {len(matches)}")
        selected.extend(matches)
    signed = bool(read_json(ROOT / "crates/liberte-client/tauri-ci.conf.json")["bundle"]["createUpdaterArtifacts"])
    for artifact in selected:
        if artifact.stat().st_size < 1024:
            raise ValueError(f"Empty or truncated installer: {artifact.name}")
        shutil.copy2(artifact, destination / artifact.name)
        signature = artifact.with_name(artifact.name + ".sig")
        if signed and artifact.suffix in (".AppImage", ".exe", ".msi"):
            if not signature.is_file() or not signature.read_text(encoding="utf-8").strip():
                raise ValueError(f"Missing updater signature for {artifact.name}")
            shutil.copy2(signature, destination / signature.name)
    write_json(destination / f"client-{label}.json", {"signed": signed, "label": label, "version": versions()})


def package_server(target, label):
    source = ROOT / "target" / target / "release" / ("liberte-server.exe" if label.startswith("windows") else "liberte-server")
    if not source.is_file() or source.stat().st_size < 1024:
        raise ValueError("Server binary is missing or truncated")
    destination = ROOT / "release-assets"
    destination.mkdir(exist_ok=True)
    if label.startswith("windows"):
        archive = destination / f"liberte-server-{label}.zip"
        with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as output:
            output.write(source, source.name)
    else:
        archive = destination / f"liberte-server-{label}.tar.gz"
        with tarfile.open(archive, "w:gz") as output:
            output.add(source, arcname=source.name)
    with archive.open("rb") as stream:
        checksum = hashlib.file_digest(stream, "sha256").hexdigest()
    archive.with_name(archive.name + ".sha256").write_text(f"{checksum}  {archive.name}\n", encoding="ascii")


def request(base, path, method="GET", data=None, headers=None):
    parsed = urllib.parse.urlsplit(base)
    if parsed.scheme != "http" or parsed.hostname != "127.0.0.1":
        raise ValueError("Smoke tests only target a local loopback HTTP server")
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    req = urllib.request.Request(base + path, data=data, method=method, headers=headers or {})
    try:
        with opener.open(req, timeout=3) as response:
            return response.status, response.read()
    except urllib.error.HTTPError as error:
        return error.code, error.read()


def probe_api(base, admin_token, expected_version):
    status, body = request(base, "/health")
    health = json.loads(body)
    if status != 200 or health.get("status") != "ok" or health.get("version") != expected_version:
        raise ValueError(f"Invalid health response ({status}): {health}")
    status, body = request(base, "/info")
    if status != 200 or json.loads(body).get("version") != expected_version:
        raise ValueError("Server info endpoint failed")
    status, _ = request(base, "/admin/status")
    if status not in (401, 403):
        raise ValueError(f"Admin endpoint accepted a missing credential: HTTP {status}")
    status, _ = request(base, "/admin/status", headers={"Authorization": f"Bearer {admin_token}"})
    if status != 200:
        raise ValueError(f"Valid admin credential rejected: HTTP {status}")
    checks = [
        ("/blob/upload", "POST", b"not-authenticated", {"Content-Type": "application/octet-stream"}),
        (f"/blob/{uuid.uuid4()}", "GET", None, {}),
        (f"/blob/{uuid.uuid4()}", "DELETE", None, {}),
        ("/backup/sync", "POST", b'{"user_pubkey_hex":"' + b"00" * 32 + b'","encrypted_data":"x"}', {"Content-Type": "application/json"}),
        ("/backup/" + "00" * 32, "GET", None, {}),
    ]
    for path, method, payload, headers in checks:
        status, _ = request(base, path, method, payload, headers)
        if status not in (401, 403):
            raise ValueError(f"{method} {path}: authentication was not enforced (HTTP {status})")


def smoke(binary=None, image=None):
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    base = f"http://127.0.0.1:{port}"
    token = secrets.token_hex(32)
    container = None
    process = None
    with tempfile.TemporaryDirectory(prefix="liberte-smoke-") as folder:
        log_path = Path(folder) / "server.log"
        with log_path.open("wb") as log:
            try:
                if image:
                    container = "liberte-smoke-" + uuid.uuid4().hex[:12]
                    run(["docker", "run", "--detach", "--name", container,
                         "--publish", f"127.0.0.1:{port}:8080",
                         "--env", "PREMIUM_REQUIRED=false",
                         "--env", f"ADMIN_TOKEN={token}", image], stdout=subprocess.DEVNULL)
                else:
                    env = os.environ.copy()
                    env.update(HTTP_ADDR=f"127.0.0.1:{port}", LISTEN_ADDR="/ip4/127.0.0.1/udp/0/quic-v1",
                               BLOB_STORAGE_PATH=str(Path(folder) / "blobs"), PREMIUM_REQUIRED="false",
                               ADMIN_TOKEN=token)
                    process = subprocess.Popen([str(Path(binary).resolve())], env=env, stdout=log, stderr=log)
                deadline = time.monotonic() + 45
                while True:
                    if process is not None and process.poll() is not None:
                        raise RuntimeError(f"Server exited before becoming ready: {process.returncode}")
                    try:
                        if request(base, "/health")[0] == 200:
                            break
                    except (OSError, urllib.error.URLError):
                        pass
                    if time.monotonic() >= deadline:
                        raise TimeoutError("Server health check did not become ready")
                    time.sleep(0.3)
                probe_api(base, token, versions())
                print("Server smoke passed: health, version, admin and unauthenticated API rejection")
            finally:
                if process is not None:
                    process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=5)
                if container:
                    subprocess.run(["docker", "logs", container], check=False)
                    subprocess.run(["docker", "rm", "--force", container], check=False, stdout=subprocess.DEVNULL)


def build_manifest(directory, repository, tag, version, public_key=""):
    directory = Path(directory)
    markers = [read_json(directory / f"client-{label}.json") for label in ("windows-x86_64", "linux-x86_64")]
    if any(marker["version"] != version for marker in markers):
        raise ValueError("Mixed client versions")
    modes = {marker["signed"] for marker in markers}
    if len(modes) != 1:
        raise ValueError("Mixed signed and unsigned clients")
    signed = modes.pop()
    manifest_path = directory / "latest.json"
    if not signed:
        if public_key.strip() or list(directory.glob("*.sig")) or manifest_path.exists():
            raise ValueError("Unsigned release contains signing metadata")
        return None
    if not public_key:
        raise ValueError("Signed release requires the public verification key")
    encoded_key = base64.b64decode(public_key, validate=True)
    platforms = {}
    for platform, pattern in (("windows-x86_64", "*setup.exe"), ("linux-x86_64", "*.AppImage")):
        matches = list(directory.glob(pattern))
        if len(matches) != 1:
            raise ValueError(f"Expected exactly one updater bundle for {platform}")
        artifact = matches[0]
        encoded_signature = artifact.with_name(artifact.name + ".sig").read_text(encoding="utf-8").strip()
        decoded_signature = base64.b64decode(encoded_signature, validate=True)
        with tempfile.TemporaryDirectory(prefix="liberte-signature-") as temporary:
            key_file = Path(temporary) / "key.pub"
            sig_file = Path(temporary) / "artifact.minisig"
            key_file.write_bytes(encoded_key)
            sig_file.write_bytes(decoded_signature)
            run(["minisign", "-V", "-m", str(artifact), "-p", str(key_file), "-x", str(sig_file)])
        platforms[platform] = {
            "signature": encoded_signature,
            "url": f"https://github.com/{repository}/releases/download/{urllib.parse.quote(tag, safe='')}/{urllib.parse.quote(artifact.name, safe='')}",
        }
    manifest = {"version": version, "notes": "See CHANGELOG.md and migration notes before updating.", "platforms": platforms}
    write_json(manifest_path, manifest)
    return manifest


def finalize_release(directory, repository, tag):
    version = versions(tag=tag)
    directory = Path(directory)
    expected = [
        "liberte-server-linux-x86_64.tar.gz",
        "liberte-server-linux-aarch64.tar.gz",
        "liberte-server-windows-x86_64.zip",
        "liberte-server-docker-linux-amd64.tar.gz",
        "liberte-server-docker-linux-arm64.tar.gz",
    ]
    for filename in expected:
        artifact = directory / filename
        if not artifact.is_file() or artifact.stat().st_size < 1024:
            raise ValueError(f"Missing or truncated release asset: {filename}")
    manifest = build_manifest(directory, repository, tag, version, os.environ.get("TAURI_UPDATER_PUBLIC_KEY", "").strip())
    lines = []
    for path in sorted(directory.iterdir()):
        if path.is_file() and path.name != "SHA256SUMS":
            with path.open("rb") as stream:
                digest = hashlib.file_digest(stream, "sha256").hexdigest()
            lines.append(f"{digest}  {path.name}")
    (directory / "SHA256SUMS").write_text("\n".join(lines) + "\n", encoding="ascii")
    mode = "Signed updater bundles and latest.json included." if manifest else "Unsigned installers. Automatic updates are unavailable; install manually."
    (ROOT / "release-notes.md").write_text(
        f"Liberté {version}\n\n{mode}\n\nProtocol migration required. See wiki/Migration-2.4.md and CHANGELOG.md.\n"
        "Docker images are provided separately for linux/amd64 and linux/arm64. Verify SHA256SUMS before loading.\n",
        encoding="utf-8")
    print(mode)


def main():
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("lockfile")
    commands.add_parser("frontend-lock")
    commands.add_parser("format-patch")
    version = commands.add_parser("versions")
    version.add_argument("--tag")
    commands.add_parser("prepare-updater")
    client = commands.add_parser("collect-client")
    client.add_argument("--target", required=True)
    client.add_argument("--label", required=True)
    server = commands.add_parser("package-server")
    server.add_argument("--target", required=True)
    server.add_argument("--label", required=True)
    smoke_parser = commands.add_parser("smoke")
    source = smoke_parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--binary")
    source.add_argument("--image")
    finalize = commands.add_parser("finalize-release")
    finalize.add_argument("--directory", default="release-assets")
    finalize.add_argument("--repository", required=True)
    finalize.add_argument("--tag", required=True)
    args = parser.parse_args()
    if args.command == "lockfile":
        lockfile()
    elif args.command == "frontend-lock":
        frontend_lock()
    elif args.command == "format-patch":
        format_patch()
    elif args.command == "versions":
        print(versions(tag=args.tag))
    elif args.command == "prepare-updater":
        prepare_updater()
    elif args.command == "collect-client":
        collect_client(args.target, args.label)
    elif args.command == "package-server":
        package_server(args.target, args.label)
    elif args.command == "smoke":
        smoke(args.binary, args.image)
    elif args.command == "finalize-release":
        finalize_release(args.directory, args.repository, args.tag)


if __name__ == "__main__":
    main()
