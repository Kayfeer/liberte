"""Fail-closed checks for the native test harness, without starting any app."""
import json
import http.client
import unittest
from unittest import mock

import native_smoke


class NativeHarnessTests(unittest.TestCase):
    def test_webdriver_protocol_errors_cannot_look_like_success(self):
        body = json.dumps({"value": {"error": "unknown error", "message": "WebKit crashed"}})
        with self.assertRaises(native_smoke.WebDriverError):
            native_smoke.decode_response(500, body)
        for status, payload in ((200, b"not json"), (200, b"{}"), (500, b'{"value": null}')):
            with self.assertRaises(RuntimeError):
                native_smoke.decode_response(status, payload)
        self.assertIsNone(native_smoke.decode_response(200, b'{"value": null}'))

    def test_unexpected_runtime_error_is_not_retried_until_success(self):
        action = mock.Mock(side_effect=native_smoke.WebDriverError("unknown error", "WebKit crashed"))
        with mock.patch.object(native_smoke.time, "sleep") as sleep:
            with self.assertRaises(native_smoke.WebDriverError):
                native_smoke.until(action, "native application")
        action.assert_called_once()
        sleep.assert_not_called()

    def test_missing_element_has_a_bounded_wait(self):
        action = mock.Mock(side_effect=native_smoke.WebDriverError("no such element", "missing"))
        with self.assertRaises(TimeoutError):
            native_smoke.until(action, "identity", timeout=0)

    def test_only_idempotent_webdriver_reads_retry_dropped_connections(self):
        for method, path in (("GET", "/url"), ("POST", "/element"), ("POST", "/elements")):
            client = native_smoke.NativeSession.__new__(native_smoke.NativeSession)
            client.session = "synthetic-session"
            client.request = mock.Mock(side_effect=[http.client.RemoteDisconnected(), "read-result"])
            with mock.patch.object(native_smoke.time, "sleep"):
                self.assertEqual(client.command(method, path), "read-result")
            self.assertEqual(client.request.call_count, 2)

    def test_mutations_and_ipc_are_never_retried_after_transport_failure(self):
        for method, path in (("POST", "/element/id/click"), ("POST", "/element/id/value"),
                             ("POST", "/execute/async"), ("POST", "/execute/sync"),
                             ("POST", "/refresh"), ("DELETE", "")):
            client = native_smoke.NativeSession.__new__(native_smoke.NativeSession)
            client.session = "synthetic-session"
            client.request = mock.Mock(side_effect=http.client.RemoteDisconnected())
            with self.assertRaises(http.client.RemoteDisconnected):
                client.command(method, path)
            client.request.assert_called_once()

    def test_read_retries_are_bounded_and_protocol_errors_remain_failures(self):
        client = native_smoke.NativeSession.__new__(native_smoke.NativeSession)
        client.session = "synthetic-session"
        client.request = mock.Mock(side_effect=http.client.RemoteDisconnected())
        with mock.patch.object(native_smoke.time, "sleep"), self.assertRaises(http.client.RemoteDisconnected):
            client.command("GET", "/url")
        self.assertEqual(client.request.call_count, 3)
        client.request = mock.Mock(side_effect=native_smoke.WebDriverError("unknown error", "crash"))
        with self.assertRaises(native_smoke.WebDriverError):
            client.command("GET", "/url")
        client.request.assert_called_once()

    def test_only_explicit_read_ipc_retries_a_transport_reset(self):
        client = native_smoke.NativeSession.__new__(native_smoke.NativeSession)
        client.command = mock.Mock(side_effect=[None, http.client.RemoteDisconnected(), None,
                                               {"ok": True, "value": [{"status": "received"}]}])
        with mock.patch.object(native_smoke.time, "sleep"):
            self.assertEqual(client.invoke("list_channel_files"), [{"status": "received"}])
        self.assertEqual(client.command.call_count, 4)
        for name in ("send_file", "save_received_file", "send_message", "import_profile", "new_unknown_command"):
            client.command = mock.Mock(side_effect=[None, http.client.RemoteDisconnected()])
            with self.assertRaises(http.client.RemoteDisconnected):
                client.invoke(name)
            self.assertEqual(client.command.call_count, 2)

    def test_read_ipc_failure_and_retry_limit_are_not_hidden(self):
        client = native_smoke.NativeSession.__new__(native_smoke.NativeSession)
        client.command = mock.Mock(side_effect=[None, {"ok": False, "error": "Database unavailable"}])
        with self.assertRaisesRegex(RuntimeError, "Database unavailable"):
            client.invoke("list_channel_files")
        self.assertEqual(client.command.call_count, 2)
        client.command = mock.Mock(side_effect=[None, http.client.RemoteDisconnected()] * 3)
        with mock.patch.object(native_smoke.time, "sleep"), self.assertRaises(http.client.RemoteDisconnected):
            client.invoke("list_channel_files")
        self.assertEqual(client.command.call_count, 6)

    def test_diagnostic_tail_redacts_secrets_and_opaque_payloads(self):
        secret = "a" * 64
        archive = "Q" * 300
        raw = f"WebKit warning: connection closed\nsecret_key_hex={secret}\n{archive}\ninvite=short-bearer\nGTK error: no display"
        tail = native_smoke.sanitize_diagnostic_tail(raw)
        self.assertIn("connection closed", tail)
        self.assertIn("GTK error: no display", tail)
        for sensitive in (secret, archive, "short-bearer"):
            self.assertNotIn(sensitive, tail)

    def test_dev_server_or_external_page_does_not_count_as_native_bundle(self):
        native_smoke.require_bundled_origin("tauri://localhost/index.html")
        native_smoke.require_bundled_origin("http://tauri.localhost/index.html")
        native_smoke.require_bundled_origin("https://tauri.localhost/index.html")
        for url in ("http://localhost:5173", "http://127.0.0.1:1420", "https://example.com", "tauri://localhost.example.com",
                    "http://tauri.localhost.example.com", "http://tauri.localhost:5173",
                    "http://user@tauri.localhost"):
            with self.assertRaises(RuntimeError):
                native_smoke.require_bundled_origin(url)


if __name__ == "__main__":
    unittest.main()
