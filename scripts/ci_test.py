"""Regression checks for gates that must fail closed."""
import base64
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
from pathlib import Path
import subprocess
import tempfile
import threading
import unittest
from unittest import mock

import ci


def fake_public_key():
    inner = base64.b64encode(b"Ed" + bytes(40)).decode("ascii")
    return base64.b64encode(f"untrusted comment: test only\n{inner}\n".encode()).decode()


class ReleaseTests(unittest.TestCase):
    def test_changed_cargo_manifest_produces_a_reviewable_lock_candidate(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            path = root / "Cargo.lock"
            path.write_text("old lock")
            calls = []
            def run_command(command, **kwargs):
                calls.append(command)
                if command == ["cargo", "generate-lockfile"]:
                    path.write_text("new lock")
            result = subprocess.CompletedProcess([], 101, stderr=(
                f"error: cannot update the lock file {path} because --locked was passed to prevent this\n"
                "help: to generate the lock file without accessing the network, "
                "remove the --locked flag and use --offline instead.\n"))
            with mock.patch.object(ci, "ROOT", root), mock.patch.object(ci.subprocess, "run", return_value=result), mock.patch.object(ci, "run", side_effect=run_command), mock.patch.object(ci, "export_file") as export:
                ci.lockfile()
            self.assertEqual(path.read_text(), "new lock")
            self.assertEqual(calls[0], ["cargo", "generate-lockfile"])
            self.assertIn("--locked", calls[-1])
            export.assert_called_once_with(path, "Cargo.lock")

    def test_cargo_network_failure_preserves_lock_and_propagates_error(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            path = root / "Cargo.lock"
            original = b"unchanged reviewed lock\n"
            path.write_bytes(original)
            diagnostic = "error: failed to download from https://index.crates.io/config.json\nCaused by:\n  Could not resolve host: index.crates.io\n"
            result = subprocess.CompletedProcess([], 101, stderr=diagnostic)
            with mock.patch.object(ci, "ROOT", root), mock.patch.object(ci.subprocess, "run", return_value=result), mock.patch.object(ci, "run") as run_command, mock.patch.object(ci, "export_file") as export:
                with self.assertRaises(subprocess.CalledProcessError) as failure:
                    ci.lockfile()
            self.assertEqual(failure.exception.returncode, 101)
            self.assertEqual(failure.exception.stderr, diagnostic)
            self.assertEqual(path.read_bytes(), original)
            run_command.assert_not_called()
            export.assert_not_called()

    def test_valid_cargo_lock_is_never_regenerated(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "Cargo.lock").write_text("valid lock")
            result = subprocess.CompletedProcess([], 0, stderr="")
            with mock.patch.object(ci, "ROOT", root), mock.patch.object(ci.subprocess, "run", return_value=result), mock.patch.object(ci, "run") as run_command, mock.patch.object(ci, "export_file") as export:
                ci.lockfile()
            self.assertEqual(run_command.call_count, 1)
            self.assertIn("--locked", run_command.call_args.args[0])
            export.assert_not_called()

    def test_rustsec_gate_keeps_informational_warnings_separate(self):
        report = {"database": {"last-commit": "database-commit"}, "lockfile": {"dependency-count": 1},
                  "settings": {"ignore": [], "severity": None, "target_arch": [], "target_os": []},
                  "vulnerabilities": {"found": False, "count": 0, "list": []},
                  "warnings": {"unmaintained": [{"package": {"name": "example"}}]}}
        with tempfile.TemporaryDirectory() as folder:
            report_path, status_path = Path(folder) / "report.json", Path(folder) / "status.txt"
            ci.write_json(report_path, report)
            status_path.write_text("0")
            ci.rust_audit_gate(report_path, status_path)
            status_path.write_text("2")
            with self.assertRaises(ValueError):
                ci.rust_audit_gate(report_path, status_path)
            status_path.write_text("0")
            report["settings"]["ignore"] = ["RUSTSEC-2099-0001"]
            ci.write_json(report_path, report)
            with self.assertRaises(ValueError):
                ci.rust_audit_gate(report_path, status_path)
            report["settings"]["ignore"] = []
            report["vulnerabilities"] = {"found": True, "count": 1, "list": [
                {"advisory": {"id": "RUSTSEC-2099-0001"},
                 "package": {"name": "example", "version": "1.0.0"}, "versions": {"patched": []}}]}
            ci.write_json(report_path, report)
            with self.assertRaisesRegex(ValueError, "known vulnerabilities"):
                ci.rust_audit_gate(report_path, status_path)
            ci.write_json(report_path, {})
            with self.assertRaises(ValueError):
                ci.rust_audit_gate(report_path, status_path)

    def test_dependency_audit_fails_closed(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "audit.json"
            for report in ({}, {"error": {"code": "ENETUNREACH"}},
                           {"metadata": {"vulnerabilities": {"high": 1, "critical": 0}}},
                           {"metadata": {"vulnerabilities": {"high": 0, "critical": 1}}}):
                ci.write_json(path, report)
                with self.assertRaises(ValueError):
                    ci.audit_gate(path)
            ci.write_json(path, {"metadata": {"vulnerabilities": {"high": 0, "critical": 0, "moderate": 1}}})
            ci.audit_gate(path)

    def test_partial_signing_configuration_is_rejected(self):
        for private, public in (("private", ""), ("", fake_public_key())):
            with self.assertRaises(ValueError):
                ci.updater_mode(private, public)

    def test_unsigned_mode_is_explicit(self):
        self.assertFalse(ci.updater_mode("", ""))
        self.assertTrue(ci.updater_mode("test-key", fake_public_key()))

    def test_malformed_public_key_is_rejected(self):
        with self.assertRaises((ValueError, UnicodeError)):
            ci.updater_mode("private", "not-a-valid-public-key")

    def test_version_mismatch_and_reused_tag_are_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "frontend").mkdir()
            (root / "crates/liberte-client").mkdir(parents=True)
            (root / "Cargo.toml").write_text('[workspace.package]\nversion = "2.4.0"\n')
            ci.write_json(root / "frontend/package.json", {"version": "2.4.0"})
            ci.write_json(root / "crates/liberte-client/tauri.conf.json", {"version": "2.4.0"})
            self.assertEqual(ci.versions(root, "v2.4.0"), "2.4.0")
            with self.assertRaises(ValueError):
                ci.versions(root, "v2.3")
            ci.write_json(root / "frontend/package.json", {"version": "0.2.1"})
            with self.assertRaises(ValueError):
                ci.versions(root)

    def markers(self, directory, modes):
        for label, signed in zip(("windows-x86_64", "linux-x86_64"), modes):
            ci.write_json(directory / f"client-{label}.json", {"label": label, "signed": signed, "version": "2.4.0"})

    def test_unsigned_release_never_creates_update_manifest(self):
        with tempfile.TemporaryDirectory() as folder:
            directory = Path(folder)
            self.markers(directory, (False, False))
            self.assertIsNone(ci.build_manifest(directory, "Kayfeer/liberte", "v2.4.0", "2.4.0"))
            self.assertFalse((directory / "latest.json").exists())
            (directory / "latest.json").write_text("{}")
            with self.assertRaises(ValueError):
                ci.build_manifest(directory, "Kayfeer/liberte", "v2.4.0", "2.4.0")

    def test_mixed_signing_modes_are_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            directory = Path(folder)
            self.markers(directory, (False, True))
            with self.assertRaises(ValueError):
                ci.build_manifest(directory, "Kayfeer/liberte", "v2.4.0", "2.4.0")

    def test_missing_signature_is_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            directory = Path(folder)
            self.markers(directory, (True, True))
            (directory / "app-setup.exe").write_bytes(b"installer")
            with self.assertRaises(FileNotFoundError):
                ci.build_manifest(directory, "Kayfeer/liberte", "v2.4.0", "2.4.0", fake_public_key())

    def test_verifier_failure_prevents_manifest_publication(self):
        with tempfile.TemporaryDirectory() as folder:
            directory = Path(folder)
            self.markers(directory, (True, True))
            for name in ("app-setup.exe", "app.AppImage"):
                (directory / name).write_bytes(b"altered installer")
                (directory / (name + ".sig")).write_text(base64.b64encode(b"invalid signature").decode())
            with mock.patch.object(ci, "run", side_effect=subprocess.CalledProcessError(1, ["minisign"])):
                with self.assertRaises(subprocess.CalledProcessError):
                    ci.build_manifest(directory, "Kayfeer/liberte", "v2.4.0", "2.4.0", fake_public_key())
            self.assertFalse((directory / "latest.json").exists())


class SmokeTests(unittest.TestCase):
    def test_refuses_external_server(self):
        with self.assertRaises(ValueError):
            ci.request("https://example.com", "/health")

    def exercise_server(self, insecure=False):
        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_):
                pass

            def do_GET(self):
                if self.path in ("/health", "/info"):
                    status, payload = 200, {"status": "ok", "version": "2.4.0"}
                elif self.path == "/admin/status" and self.headers.get("Authorization") == "Bearer test-token":
                    status, payload = 200, {}
                else:
                    status, payload = 403, {}
                self.send_response(status)
                self.end_headers()
                self.wfile.write(json.dumps(payload).encode())

            def do_POST(self):
                self.send_response(201 if insecure else 403)
                self.end_headers()

            def do_DELETE(self):
                self.send_response(403)
                self.end_headers()

        server = HTTPServer(("127.0.0.1", 0), Handler)
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        try:
            ci.probe_api(f"http://127.0.0.1:{server.server_port}", "test-token", "2.4.0")
        finally:
            server.shutdown()
            server.server_close()
            worker.join(timeout=2)

    def test_probes_health_admin_and_all_protected_routes(self):
        self.exercise_server()

    def test_detects_unauthenticated_upload_regression(self):
        with self.assertRaisesRegex(ValueError, "authentication was not enforced"):
            self.exercise_server(insecure=True)


if __name__ == "__main__":
    unittest.main()
