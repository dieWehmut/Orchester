"""Offline regression checks for cleanup after a live-check failure."""
import contextlib
import importlib.util
import io
from pathlib import Path
import unittest
from unittest.mock import Mock, patch


spec = importlib.util.spec_from_file_location("remote_smoke", Path(__file__).with_name("smoke.py"))
smoke = importlib.util.module_from_spec(spec)
spec.loader.exec_module(smoke)


class SmokeCleanupTest(unittest.TestCase):
    def test_lost_idempotent_retry_cancels_the_already_started_task(self):
        failure = OSError("retry connection lost")
        client = Mock()
        client.request.side_effect = [{"id": "started"}, failure, None]
        with contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaises(OSError) as raised:
                smoke.turn(client, "codex", "test", 10)
        self.assertIs(raised.exception, failure)
        self.assertEqual(client.request.call_args.args, ("/runs/started/cancel", "POST"))

    def test_cancel_failure_does_not_replace_the_original_read_error(self):
        failure = OSError("output connection lost")
        client = Mock()
        client.request.side_effect = [{"id": "started"}, {"id": "started"},
                                      failure, RuntimeError("cancel failed")]
        diagnostics = io.StringIO()
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(diagnostics):
            with self.assertRaises(OSError) as raised:
                smoke.turn(client, "codex", "test", 10)
        self.assertIs(raised.exception, failure)
        self.assertIn('"event": "test_task_cancel_failed"', diagnostics.getvalue())

    def test_revoke_failure_preserves_an_existing_verification_error(self):
        failure = RuntimeError("verification failed")
        client = Mock()
        client.request.side_effect = [{"token": "private-token"}, OSError("revoke failed")]
        diagnostics = io.StringIO()
        with patch.object(smoke, "Client", return_value=client), \
                patch.object(smoke.getpass, "getpass", return_value="private-code"), \
                patch.object(smoke.sys, "argv", ["smoke.py"]), \
                patch.object(smoke, "turn", side_effect=failure), \
                contextlib.redirect_stderr(diagnostics):
            with self.assertRaises(RuntimeError) as raised:
                smoke.main()
        self.assertIs(raised.exception, failure)
        self.assertIn('"event": "test_device_revoke_failed"', diagnostics.getvalue())
        self.assertNotIn("private-token", diagnostics.getvalue())
        self.assertNotIn("private-code", diagnostics.getvalue())

    def test_revoke_failure_is_still_fatal_after_successful_turns(self):
        failure = OSError("revoke failed")
        client = Mock()
        client.request.side_effect = [{"token": "private-token"}, failure]
        completed = ({"id": "run", "session_id": "session"},
                     [{"type": "message", "text": "ORCHESTER_ABC"}])
        with patch.object(smoke, "Client", return_value=client), \
                patch.object(smoke.getpass, "getpass", return_value="private-code"), \
                patch.object(smoke.sys, "argv", ["smoke.py"]), \
                patch.object(smoke.secrets, "token_hex", return_value="abc"), \
                patch.object(smoke, "turn", return_value=completed), \
                contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaises(OSError) as raised:
                smoke.main()
        self.assertIs(raised.exception, failure)


if __name__ == "__main__":
    unittest.main()
