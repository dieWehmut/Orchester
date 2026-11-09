#!/usr/bin/env python3
"""Run OpenCode or the remote server with this computer's Codex API provider.

Only the launched process receives the provider override. Credentials stay in
the child environment and are never put in argv, a generated file or output.
This supports Codex API-key providers, not ChatGPT subscription credentials.
"""
import argparse
import json
import os
from pathlib import Path
import sys
import subprocess
import tomllib
from urllib.parse import urlsplit


def provider_environment(config_path, auth_path, inherited):
    with Path(config_path).open("rb") as source:
        config = tomllib.load(source)
    provider_id = config.get("model_provider", "openai")
    provider = config.get("model_providers", {}).get(provider_id, {})
    model = config.get("model")
    if not isinstance(model, str) or not model or "/" in model:
        raise ValueError("Codex configuration needs an explicit model name")
    wire_api = provider.get("wire_api", "responses")
    if wire_api not in ("responses", "chat"):
        raise ValueError("Codex provider must use the responses or chat API")
    endpoint = provider.get("base_url", "https://api.openai.com/v1")
    parsed = urlsplit(endpoint)
    if (parsed.scheme not in ("https", "http") or not parsed.hostname
            or parsed.username or parsed.password or parsed.query or parsed.fragment):
        raise ValueError("Codex provider needs an HTTP(S) base URL without embedded credentials")
    env_key = provider.get("env_key")
    if env_key:
        key = inherited.get(env_key)
    else:
        key = inherited.get("OPENAI_API_KEY")
        if not key:
            with Path(auth_path).open(encoding="utf-8") as source:
                key = json.load(source).get("OPENAI_API_KEY")
    if not isinstance(key, str) or not key.strip():
        raise ValueError("Codex API key is unavailable; configure an API-key provider on this computer")
    sdk = "@ai-sdk/openai" if wire_api == "responses" else "@ai-sdk/openai-compatible"
    open_code = {
        "provider": {
            "orchester-codex": {
                "npm": sdk,
                "name": "This computer's Codex provider",
                "options": {
                    "baseURL": "{env:ORCHESTER_REMOTE_MODEL_URL}",
                    "apiKey": "{env:ORCHESTER_REMOTE_MODEL_KEY}",
                },
                "models": {model: {"name": model}},
            },
        },
        "model": "orchester-codex/" + model,
        "small_model": "orchester-codex/" + model,
    }
    env = inherited.copy()
    env["ORCHESTER_REMOTE_MODEL_URL"] = endpoint
    env["ORCHESTER_REMOTE_MODEL_KEY"] = key
    env["OPENCODE_CONFIG_CONTENT"] = json.dumps(open_code)
    return env, {"model": open_code["model"], "wire_api": wire_api, "sdk": sdk}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path, default=Path.home() / ".codex/config.toml")
    parser.add_argument("--auth", type=Path, default=Path.home() / ".codex/auth.json")
    parser.add_argument("--check", action="store_true", help="Validate local configuration without calling a model")
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    try:
        env, description = provider_environment(args.config, args.auth, os.environ)
    except (ValueError, OSError, tomllib.TOMLDecodeError):
        parser.error("Cannot load a compatible Codex API-key provider. Check --config, --auth and the provider's env_key.")
    if args.check:
        print(json.dumps({"event": "provider_ready", **description}))
        return
    command = args.command
    if command and command[0] == "--":
        command = command[1:]
    if not command:
        parser.error("Provide a command after --, or use --check")
    # Windows CRT exec does not reliably preserve a captured console's handles.
    # Inherit them through CreateProcess and return the actual child's status.
    if os.name == "nt":
        raise SystemExit(subprocess.call(command, env=env))
    # Unix exec keeps cancellation attached to the actual command's PID.
    os.execvpe(command[0], command, env)


if __name__ == "__main__":
    main()
