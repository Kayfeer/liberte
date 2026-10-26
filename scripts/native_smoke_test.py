"""Fail-closed checks for the native test harness, without starting any app."""
import json
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

    def test_dev_server_or_external_page_does_not_count_as_native_bundle(self):
        native_smoke.require_bundled_origin("tauri://localhost/index.html")
        for url in ("http://localhost:5173", "http://127.0.0.1:1420", "https://example.com", "tauri://localhost.example.com"):
            with self.assertRaises(RuntimeError):
                native_smoke.require_bundled_origin(url)


if __name__ == "__main__":
    unittest.main()
