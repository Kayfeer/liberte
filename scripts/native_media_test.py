"""Proof checks for the media observer; actual media runs only in Linux CI."""
import copy
import unittest
from unittest import mock

import native_media


class NativeMediaObservationTests(unittest.TestCase):
    def test_live_tracks_alone_do_not_prove_received_media(self):
        value = {"context": "running", "sampleCount": 10, "maxRms": 0.1,
                 "remote": [{"kind": kind, "readyState": "live"} for kind in ("audio", "video")],
                 "width": 640, "height": 480, "decodedFrames": 12, "frameCallbacks": 0}
        self.assertTrue(native_media.received_media(value, 10))
        for key, invalid in (("maxRms", 0), ("width", 0), ("sampleCount", 0),
                             ("context", "suspended"), ("decodedFrames", 10)):
            changed = copy.deepcopy(value)
            changed[key] = invalid
            self.assertFalse(native_media.received_media(changed, 10), key)
        changed = copy.deepcopy(value)
        changed["remote"][1]["readyState"] = "ended"
        self.assertFalse(native_media.received_media(changed, 10))

    def test_preflight_rejects_physical_capture_or_missing_automation(self):
        for field in ("mockCaptureDevices", "automationControlled"):
            state = {"mockCaptureDevices": True, "automationControlled": True,
                     "microphone": "none", "camera": "none"}
            state[field] = False
            client = mock.Mock()
            client.invoke.return_value = state
            with self.assertRaisesRegex(AssertionError, "synthetic devices"):
                native_media.capture_state(client)
            client.invoke.assert_called_once_with("qa_media_capture_state")

    def test_cef_preflight_requires_real_debug_configuration_and_matching_port(self):
        configuration = {"engine": "cef", "syntheticMedia": True, "debugBuild": True,
                         "remoteDebuggingPort": 9223}
        native_media.require_cef_qa_configuration(configuration, 9223)
        for field, invalid in (("engine", "webkit"), ("syntheticMedia", False),
                               ("debugBuild", False), ("remoteDebuggingPort", 9224)):
            with self.assertRaises(AssertionError):
                native_media.require_cef_qa_configuration({**configuration, field: invalid}, 9223)
        with self.assertRaises(AssertionError):
            native_media.require_cef_qa_configuration(None, 9223)

    def test_cef_capture_requires_the_dedicated_real_track_observer(self):
        client = mock.Mock()
        client.driver = "cef"
        client.capture_state.return_value = {"source": "cdp-media-stream-tracks", "microphone": "none"}
        self.assertEqual(native_media.capture_state(client, {"microphone": "none"})["microphone"], "none")
        client.invoke.assert_not_called()
        client.capture_state.return_value = {"mockCaptureDevices": True, "automationControlled": True}
        with self.assertRaisesRegex(AssertionError, "real MediaStreamTrack"):
            native_media.capture_state(client)

    def test_missing_runtime_apis_report_the_exact_cause_before_call(self):
        supported = {"isSecureContext": True, "getUserMedia": "function",
                     "RTCPeerConnection": "function", "AudioContext": "function",
                     "origin": "tauri://localhost", "protocol": "tauri:"}
        native_media.require_capabilities(supported)
        for name in ("getUserMedia", "RTCPeerConnection", "AudioContext"):
            missing = {**supported, name: "undefined"}
            with self.assertRaisesRegex(RuntimeError, name + " .*undefined"):
                native_media.require_capabilities(missing)
        with self.assertRaisesRegex(RuntimeError, "secure context"):
            native_media.require_capabilities({**supported, "isSecureContext": False})
        with self.assertRaisesRegex(RuntimeError, "invalid result"):
            native_media.require_capabilities(None)

    def test_start_call_reports_application_error_without_waiting_for_panel(self):
        client = mock.Mock(spec=native_media.NativeSession)
        client.assert_no_alert.side_effect = AssertionError("WebRTC missing in this browser")
        with self.assertRaisesRegex(AssertionError, "WebRTC missing"):
            native_media.start_call(client)
        client.find.assert_not_called()
        client.invoke.assert_not_called()
        client.click.assert_called_once()

    def test_empty_or_live_tracks_cannot_count_as_cleanup(self):
        self.assertFalse(native_media.tracks_ended([]))
        self.assertFalse(native_media.tracks_ended([{"readyState": "ended"}, {"readyState": "live"}]))
        self.assertFalse(native_media.tracks_ended([{}]))
        self.assertTrue(native_media.tracks_ended([{"readyState": "ended"}]))


if __name__ == "__main__":
    unittest.main()
