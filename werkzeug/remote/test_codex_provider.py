import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


spec = importlib.util.spec_from_file_location("codex_provider", Path(__file__).with_name("codex-provider.py"))
provider = importlib.util.module_from_spec(spec)
spec.loader.exec_module(provider)


class ProviderConfigurationTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.config = Path(self.directory.name) / "config.toml"
        self.auth = Path(self.directory.name) / "auth.json"
        self.config.write_text('model = "test-model"\nmodel_provider = "test"\n[model_providers.test]\nwire_api = "responses"\nbase_url = "http://127.0.0.1:9876/v1"\n')
        self.auth.write_text(json.dumps({"OPENAI_API_KEY": "fixture-secret"}))

    def test_provider_is_child_only_and_configuration_has_no_literal_secret(self):
        inherited = {"PATH": "/fixture/bin", "KEEP": "unchanged"}
        env, metadata = provider.provider_environment(self.config, self.auth, inherited)
        self.assertEqual(inherited, {"PATH": "/fixture/bin", "KEEP": "unchanged"})
        self.assertEqual(env["ORCHESTER_REMOTE_MODEL_KEY"], "fixture-secret")
        self.assertNotIn("fixture-secret", env["OPENCODE_CONFIG_CONTENT"])
        self.assertNotIn("fixture-secret", json.dumps(metadata))
        self.assertEqual(env["PATH"], inherited["PATH"])
        config = json.loads(env["OPENCODE_CONFIG_CONTENT"])
        self.assertEqual(config["small_model"], config["model"])
        self.assertEqual(config["provider"]["orchester-codex"]["npm"], "@ai-sdk/openai")

    def test_explicit_env_key_does_not_fall_back_to_another_account(self):
        self.config.write_text(self.config.read_text() + 'env_key = "CUSTOM_KEY"\n')
        with self.assertRaises(ValueError):
            provider.provider_environment(self.config, self.auth, {"OPENAI_API_KEY": "wrong-account"})
        env, _ = provider.provider_environment(self.config, self.auth, {"CUSTOM_KEY": "right-account"})
        self.assertEqual(env["ORCHESTER_REMOTE_MODEL_KEY"], "right-account")

    def test_subscription_auth_cannot_silently_be_used_as_an_api_key(self):
        self.auth.write_text(json.dumps({"tokens": {"access_token": "fixture-subscription"}}))
        with self.assertRaises(ValueError):
            provider.provider_environment(self.config, self.auth, {})

    def test_embedded_url_credentials_are_rejected(self):
        self.config.write_text(self.config.read_text().replace("127.0.0.1:9876", "username:fixture-secret@127.0.0.1:9876"))
        with self.assertRaises(ValueError):
            provider.provider_environment(self.config, self.auth, {})


if __name__ == "__main__":
    unittest.main()
