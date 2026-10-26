"""Failure checks for the harness; these do not substitute for the real CI run."""
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import native_integration
from native_smoke import NativeSession, decode_ipc_result, isolated_environment


class NativeIntegrationHarnessTests(unittest.TestCase):
    def test_rejected_or_malformed_ipc_cannot_count_as_success(self):
        for result in (None, {}, {"ok": True}, {"ok": 1, "value": None},
                       {"ok": False, "error": "permission denied"}):
            with self.assertRaises(RuntimeError):
                decode_ipc_result("send_file", result)
        self.assertIsNone(decode_ipc_result("save_received_file", {"ok": True, "value": None}))

    def test_real_ipc_rejection_is_not_retried(self):
        client = NativeSession.__new__(NativeSession)
        client.command = mock.Mock(side_effect=[None, {"ok": False, "error": "publish failed"}])
        with self.assertRaisesRegex(RuntimeError, "publish failed"):
            client.invoke("send_file", {"channelId": "test", "filePath": "/tmp/synthetic"})
        self.assertEqual(client.command.call_count, 2)  # timeout setup + exactly one invocation
        self.assertEqual(client.command.call_args.args[1], "/execute/async")

    def test_receiver_completion_does_not_count_as_sender_ack(self):
        self.assertFalse(native_integration.transfer_has_status(None, "completed"))
        self.assertFalse(native_integration.transfer_has_status({"status": "received"}, "completed"))
        self.assertTrue(native_integration.transfer_has_status({"status": "completed"}, "completed"))
        with self.assertRaises(AssertionError):
            native_integration.transfer_has_status({"status": "failed", "error": "wrong hash"}, "completed")

    def test_file_content_and_length_must_match(self):
        with tempfile.TemporaryDirectory() as directory:
            source, destination = Path(directory) / "source", Path(directory) / "destination"
            source.write_bytes(b"native-file-content")
            destination.write_bytes(source.read_bytes())
            self.assertEqual(len(native_integration.verify_file(source, destination)), 64)
            destination.write_bytes(b"native-file-contend")
            with self.assertRaises(AssertionError):
                native_integration.verify_file(source, destination)

    def test_two_profiles_use_distinct_xdg_paths_without_changing_home(self):
        with tempfile.TemporaryDirectory() as directory:
            alice = isolated_environment(Path(directory) / "alice")
            bob = isolated_environment(Path(directory) / "bob")
            for key in ("XDG_DATA_HOME", "XDG_CONFIG_HOME", "XDG_CACHE_HOME", "XDG_RUNTIME_DIR"):
                self.assertNotEqual(alice[key], bob[key])
                self.assertTrue(Path(alice[key]).is_dir())
            self.assertEqual(alice.get("HOME"), bob.get("HOME"))


if __name__ == "__main__":
    unittest.main()
