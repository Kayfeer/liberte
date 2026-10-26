"""Contract tests only; real native/LiveKit encryption is exercised in CI."""
import base64
import json
import unittest

from native_sfu import adversary_connection, require_encrypted_failure, fixture_metadata, publisher_metrics
from native_media import require_sfu_capabilities


def connection(identity, room="ab" * 32, key="12" * 32):
    payload = base64.urlsafe_b64encode(json.dumps({
        "sub": identity, "video": {"room": room, "roomJoin": True},
    }).encode()).decode().rstrip("=")
    return {"url": "ws://127.0.0.1:7880", "token": "header." + payload + ".test-signature",
            "roomName": room, "encryptionKey": key}


class NativeSfuContractTests(unittest.TestCase):
    def test_adversarial_key_changes_without_changing_the_real_room_or_token(self):
        alice, bob = connection("a" * 64), connection("b" * 64)
        adversary = adversary_connection(alice, bob)
        self.assertEqual(adversary["roomName"], alice["roomName"])
        self.assertEqual(adversary["token"], bob["token"])
        self.assertEqual(adversary["identity"], "b" * 64)
        self.assertEqual(adversary["url"], bob["url"])
        self.assertNotEqual(adversary["encryptionKey"], bob["encryptionKey"])
        self.assertEqual(bytes.fromhex(adversary["encryptionKey"])[1:], bytes.fromhex(bob["encryptionKey"])[1:])
        self.assertEqual(bob["encryptionKey"], "12" * 32)

    def test_different_room_or_initial_key_cannot_pretend_to_prove_decryption_failure(self):
        alice = connection("a" * 64)
        for bob in (connection("b" * 64, room="cd" * 32),
                    connection("b" * 64, key="34" * 32), connection("a" * 64)):
            with self.assertRaises(AssertionError):
                adversary_connection(alice, bob)

    def test_silent_or_inactive_probe_is_insufficient_without_explicit_crypto_failure(self):
        value = {"context": "running", "sampleCount": 5, "maxRms": 0,
                 "decodedFrames": 0, "frameCallbacks": 0}
        errors = ["Erreur de chiffrement SFU."]
        require_encrypted_failure(value, errors)
        for field, invalid in (("context", "suspended"), ("sampleCount", 0), ("maxRms", 0.02),
                               ("maxRms", float("nan")), ("maxRms", None),
                               ("decodedFrames", 1), ("frameCallbacks", 1)):
            with self.assertRaises(AssertionError):
                require_encrypted_failure({**value, field: invalid}, errors)
        with self.assertRaises(AssertionError):
            require_encrypted_failure(value, ["Connexion interrompue"])

    def test_fixture_hash_contract_keeps_three_explicit_sources_and_no_extra_fields(self):
        value = {"runtime": "CEF", "browserVersion": "154.0.8040.0", "applicationSha256": "a" * 64,
                 "sourceSha256": {
                     "launcher": "c" * 64, "page": "d" * 64, "participant": "e" * 64,
                     "unrelated": "must not appear in report"}, "unrelated": "ignored"}
        clean = fixture_metadata(value)
        self.assertEqual(set(clean["sourceSha256"]), {"launcher", "page", "participant"})
        self.assertNotIn("unrelated", clean)
        with self.assertRaises(AssertionError):
            fixture_metadata({**value, "sourceSha256": "f" * 64})
        for name in ("audioBytes", "videoBytes", "encodedFrames"):
            metrics = {"e2ee": True, "audioBytes": 80, "videoBytes": 400, "encodedFrames": 3}
            self.assertGreater(publisher_metrics(metrics)[name], 0)
            with self.assertRaises(AssertionError):
                publisher_metrics({**metrics, name: 0})

    def test_sfu_requires_crypto_worker_and_encoded_transform_support(self):
        value = {"isSecureContext": True, "getUserMedia": "function", "RTCPeerConnection": "function",
                 "AudioContext": "function", "RTCRtpScriptTransform": "function",
                 "Worker": "function", "cryptoSubtle": True}
        require_sfu_capabilities(value)
        require_sfu_capabilities({**value, "RTCRtpScriptTransform": "undefined", "createEncodedStreams": "function"})
        for field, invalid in (("RTCRtpScriptTransform", "undefined"), ("Worker", "undefined"), ("cryptoSubtle", False)):
            with self.assertRaises(RuntimeError):
                require_sfu_capabilities({**value, field: invalid})


if __name__ == "__main__":
    unittest.main()
