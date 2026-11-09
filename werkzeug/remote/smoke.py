#!/usr/bin/env python3
"""Live, account-backed remote API check; creates two harmless agent turns.

Use a fresh code from the computer's `pair` command. The temporary test device
is revoked on exit. No credentials are printed or written to disk.
"""
import argparse
import getpass
import json
import secrets
import sys
import time
import urllib.error
import urllib.parse
import urllib.request


class Client:
    def __init__(self, address):
        url = urllib.parse.urlsplit(address)
        if (url.scheme not in ("http", "https") or not url.hostname
                or url.username or url.password or url.path not in ("", "/")
                or url.query or url.fragment):
            raise ValueError("Address must be an HTTP(S) origin")
        self.address = address.rstrip("/")
        self.token = None
        self.http = urllib.request.build_opener(NoRedirect, urllib.request.ProxyHandler({}))

    def request(self, path, method="GET", body=None):
        headers = {"Content-Type": "application/json"}
        if self.token:
            headers["Authorization"] = "Bearer " + self.token
        request = urllib.request.Request(
            self.address + "/remote/v1" + path,
            data=None if body is None else json.dumps(body).encode(),
            headers=headers, method=method,
        )
        with self.http.open(request, timeout=20) as response:
            raw = response.read()
            return json.loads(raw) if raw else None


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def cleanup_failure(event, error):
    # Preserve the original failure without printing requests or credentials.
    print(json.dumps({"event": event, "error_type": type(error).__name__,
                      "status": getattr(error, "code", None)}), file=sys.stderr, flush=True)


def turn(client, agent, prompt, timeout, resume=None):
    body = {"agent": agent, "prompt": prompt, "idempotency_key": secrets.token_hex(16)}
    if resume:
        body["resume"] = resume
    run = client.request("/runs", "POST", body)
    path = "/runs/" + run["id"]
    print(json.dumps({"event": "submitted", "agent": agent, "run_id": run["id"]}), flush=True)
    deadline = time.monotonic() + timeout
    cursor, events = 0, []
    try:
        if client.request("/runs", "POST", body)["id"] != run["id"]:
            raise RuntimeError("Idempotent retry created a second task")
        while time.monotonic() < deadline:
            page = client.request(path + "/events?after=" + str(cursor))
            events.extend(entry["event"] for entry in page["events"])
            cursor = page["next_cursor"]
            if page["state"] != "running" and not page["events"]:
                run = client.request(path)
                if run["state"] != "completed":
                    raise RuntimeError(f"Task {run['id']} {run['state']}: {run.get('error_code', '')}")
                return run, events
            time.sleep(0.5)
        raise TimeoutError(f"Task {run['id']} did not finish in {timeout}s")
    except BaseException:
        try:
            client.request(path + "/cancel", "POST")
        except Exception as error:
            cleanup_failure("test_task_cancel_failed", error)
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--address", default="http://127.0.0.1:7337")
    parser.add_argument("--agent", default="codex")
    parser.add_argument("--timeout", type=float, default=180)
    args = parser.parse_args()
    client = Client(args.address)
    code = getpass.getpass("One-time pairing code: ")
    client.token = client.request("/pair", "POST", {"code": code, "device_name": "Remote live verification"})["token"]
    try:
        marker = "ORCHESTER_" + secrets.token_hex(6).upper()
        first, events = turn(client, args.agent,
            "---\nRemember this verification word: " + marker +
            ". Reply exactly with this word. Do not call tools or modify files.", args.timeout)
        texts = [e.get("text", "").strip() for e in events if e["type"] == "message"]
        if marker not in texts or not first.get("session_id"):
            raise RuntimeError("Missing exact model response or resumable session ID")
        second, events = turn(client, args.agent,
            "Reply only with the verification word from my previous message. Do not call tools or modify files.",
            args.timeout, first["session_id"])
        texts = [e.get("text", "").strip() for e in events if e["type"] == "message"]
        if marker not in texts or second.get("session_id") != first["session_id"]:
            raise RuntimeError("Resume did not preserve the original conversation")
        print(json.dumps({"event": "verified", "agent": args.agent, "session_id": first["session_id"],
            "run_ids": [first["id"], second["id"]], "checks": ["pairing", "idempotency", "output", "resume_memory"]}), flush=True)
    finally:
        already_failed = sys.exc_info()[0] is not None
        try:
            client.request("/device", "DELETE")
            try:
                client.request("/info")
            except urllib.error.HTTPError as error:
                if error.code != 401:
                    raise
            else:
                raise RuntimeError("Revoked credentials were still accepted")
            print('{"event":"test_device_revoked"}', flush=True)
        except Exception as error:
            if not already_failed:
                raise
            cleanup_failure("test_device_revoke_failed", error)


if __name__ == "__main__":
    main()
