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

    def test_empty_or_live_tracks_cannot_count_as_cleanup(self):
        self.assertFalse(native_media.tracks_ended([]))
        self.assertFalse(native_media.tracks_ended([{"readyState": "ended"}, {"readyState": "live"}]))
        self.assertFalse(native_media.tracks_ended([{}]))
        self.assertTrue(native_media.tracks_ended([{"readyState": "ended"}]))


if __name__ == "__main__":
    unittest.main()
