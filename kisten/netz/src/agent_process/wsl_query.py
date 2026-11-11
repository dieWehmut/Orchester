"""Bounded, read-only /proc identity query. stdout contains counts/quality only.
No raw arguments, executable paths, PIDs or distro names are returned or saved.
Branding rules are supplied by the Rust caller, shared with local sysinfo.
"""
import hashlib
import json
import os
import sys
import time

rules = json.loads(sys.argv[1])
MAX_BYTES = 16 * 1024
MAX_ARGS = 64

def path(value):
    value = value.replace("\\", "/").lower()
    rooted, parts = value.startswith("/"), []
    for part in value.split("/"):
        if part in ("", "."):
            continue
        if part == ".." and parts and parts[-1] != ".." and not parts[-1].endswith(":"):
            parts.pop()
        elif part == ".." and (rooted or (parts and parts[-1].endswith(":"))):
            continue
        else:
            parts.append(part)
    return ("/" if rooted else "") + "/".join(parts)

def basename(value):
    name = path(value).rsplit("/", 1)[-1]
    return name[:-4] if name.endswith(".exe") else name

def suffix(value, expected):
    return value == expected or value.endswith("/" + expected)

def candidate(name):
    name = basename(name)
    return (name in rules["primary"] or name in rules["desktop_services"]
            or name in rules["native_cli"] or name in rules["runtimes"]
            or name in rules["candidates"]
            or name.startswith(("python3.", "pypy3.")))

def cursor_cli_file(value):
    if "/cursor-agent/versions/" not in value:
        return None
    parts = value.rsplit("/cursor-agent/versions/", 1)[1].split("/")
    return parts[1] if len(parts) == 2 and parts[0] else None

def entry_index(args):
    index = 2 if args and basename(args[0]) == "bun" and args[1:2] == ["run"] else 1
    while index < len(args):
        arg = args[index]
        if arg in ("-e", "--eval", "-p", "--print", "-c"):
            return None
        if arg == "--":
            return index + 1 if index + 1 < len(args) else None
        key = arg.split("=", 1)[0]
        if key in rules["runtime_value_flags"]:
            index += 1 if "=" in arg else 2
        elif key in rules["runtime_switch_flags"]:
            index += 1
        elif arg.startswith("-"):
            return None
        else:
            return index
    return None

def classify(name, exe, args):
    name, exe = basename(name), path(exe)
    if name == "claude" and not exe:
        return None
    desktop = desktop_provider(exe)
    if desktop:
        return desktop if desktop_main(args) else None
    if name in rules["primary"]:
        return rules["primary"][name]
    executable = basename(exe)
    if executable in rules["native_cli"]:
        provider = rules["native_cli"][executable]
        return provider if args and not cli_maintenance(provider, args[1:]) else None
    if cursor_cli_file(exe) in ("cursor-agent", "cursor-agent.exe", "agent", "agent.exe"):
        return "cursor"
    if name.startswith("language_server") or executable.startswith("language_server"):
        branded = any(suffix(exe, "antigravity/resources/bin/" + filename)
                      for filename in ("language_server", "language_server.exe", "language_server_linux_x64", "language_server_linux_arm"))
        explicit = any(arg.startswith("--override_user_agent_name=") and arg.split("=", 1)[1].lower() == "antigravity"
                       or (arg == "--override_user_agent_name" and index + 1 < len(args)
                           and args[index + 1].lower() == "antigravity")
                       for index, arg in enumerate(args))
        return "antigravity" if branded or explicit else None
    runtime = (executable in rules["runtimes"]
               or executable.startswith(("python3.", "pypy3."))
               or (executable == "dsh desktop" and "/dsh desktop/" in exe))
    if not runtime:
        return None
    python = executable.startswith(("python", "pypy"))
    if python and name == "hermes" and args and basename(args[0]) == "hermes":
        return "hermes"
    if python:
        module_index = 1
        while module_index < len(args) and args[module_index] in ("-u", "-I", "-B", "-E", "-s", "-S"):
            module_index += 1
        if args[module_index:module_index + 1] == ["-m"]:
            provider = rules["python_modules"].get(args[module_index + 1]) if module_index + 1 < len(args) else None
            return provider if provider and not cli_maintenance(provider, args[module_index + 2:]) else None
    if args and basename(args[0]) == "openclaw-gateway":
        return "openclaw"
    index = entry_index(args)
    if index is None:
        return None
    entry = path(args[index])
    if suffix(entry, "dsh desktop/resources/harness-node-entry.mjs"):
        index += 1
        if index >= len(args):
            return None
        entry = path(args[index])
        if not suffix(entry, "@deepseek-ai/dsh/lib/bin.js"):
            return None
    if cursor_cli_file(entry) == "index.js":
        return "cursor"
    provider = next((provider for expected, provider in rules["entries"].items()
                     if suffix(entry, expected)), None)
    if provider == "deepseek" and dsh_maintenance(args[index + 1:]):
        return None
    if provider == "qwen" and qwen_maintenance(args[index + 1:]):
        return None
    if provider == "openclaw" and not openclaw_gateway(args[index + 1:]):
        return None
    if provider in rules["cli_maintenance_commands"] and cli_maintenance(provider, args[index + 1:]):
        return None
    return provider

def desktop_provider(exe):
    if basename(exe) in rules["desktop_services"]:
        return rules["desktop_services"][basename(exe)]
    for brand, provider in sorted(rules["desktop_brands"].items()):
        if suffix(exe, brand):
            return provider
    if basename(exe) == "claude" and ("/anthropicclaude/" in exe
            or any((component.startswith("claude_") and component.endswith("__pzs8sxrjxfjjc"))
                   or (component.startswith("anthropicpbc.claude_") and component.endswith("__fnn82j28hfe8t"))
                   for component in exe.split("/"))):
        return "claude_app"
    return None

def desktop_main(args):
    index = entry_index(args)
    helper = index is not None and path(args[index]).endswith((".js", ".mjs", ".cjs", ".ts"))
    return bool(args and not helper and not any(arg.split("=", 1)[0] in ("--type", "--crashpad-handler", "--ms-enable-electron-run-as-node") for arg in args))

def cli_maintenance(provider, args):
    commands = rules["cli_maintenance_commands"].get(provider, [])
    index = 0
    while index < len(args):
        arg = args[index]
        key = arg.split("=", 1)[0]
        if key in ("--prompt", "-p", "--message", "--query", "-q", "--"):
            return False
        if key in rules["qwen_value_flags"]:
            index += 1 if "=" in arg else 2
            continue
        if arg in rules["maintenance"] or arg in ("-v", "version"):
            return True
        if not arg.startswith("-"):
            return arg in commands
        index += 1
    return False

def qwen_maintenance(args):
    index = 0
    while index < len(args):
        arg = args[index]
        key = arg.split("=", 1)[0]
        if key in ("--prompt", "-p", "--"):
            return False
        if key in rules["qwen_value_flags"]:
            index += 1 if "=" in arg else 2
            continue
        if arg in rules["maintenance"] or arg == "-v":
            return True
        if not arg.startswith("-"):
            return arg in rules["qwen_maintenance_commands"]
        index += 1
    return False

def openclaw_gateway(args):
    index, command, run = 0, False, False
    while index < len(args):
        arg = args[index]
        key = arg.split("=", 1)[0]
        value_flag = (key in rules["openclaw_root_value_flags"]
                      or (command and key in rules["openclaw_gateway_value_flags"]))
        if value_flag:
            if "=" not in arg:
                if index + 1 >= len(args) or not args[index + 1] or args[index + 1].startswith("-"):
                    return False
                index += 1
            elif arg.endswith("="):
                return False
        elif (key in rules["openclaw_root_switch_flags"]
              or (command and key in rules["openclaw_gateway_switch_flags"])):
            if "=" in arg:
                return False
        elif not command and arg == "gateway":
            command = True
        elif command and not run and arg == "run":
            run = True
        else:
            return False
        index += 1
    return command

def dsh_maintenance(args):
    if args and args[0] == "plugin":
        return True
    index = 0
    while index < len(args):
        arg = args[index]
        if arg in ("--prompt", "-p", "--"):
            break
        key = arg.split("=", 1)[0]
        if key in rules["dsh_value_flags"]:
            index += 1 if "=" in arg else 2
            continue
        if key.startswith("--dump-config") or key == "--dump-default-config" or (arg.startswith("-") and arg in rules["maintenance"]):
            return True
        index += 1
    return False

def relative_entry(args):
    if args and (basename(args[0]) in rules["native_cli"] or basename(args[0]) in rules["desktop_services"] or basename(args[0]) in rules["primary"]):
        return False
    index = entry_index(args)
    if index is None:
        return False
    entry = path(args[index])
    return not entry.startswith("/") and entry[1:2] != ":"

def resolve_relative_entry(args, cwd):
    if cwd and relative_entry(args):
        index = entry_index(args)
        args[index] = cwd.rstrip("/\\") + "/" + args[index]

def bounded_args(args):
    result, size = [], 0
    for value in args[:MAX_ARGS + 1]:
        size += len(value.encode("utf-8")) + 1
        if len(result) >= MAX_ARGS or size > MAX_BYTES:
            return result, False
        result.append(value)
    return result, True

def instance_key(name, exe, args):
    if desktop_provider(path(exe)):
        return "desktop:" + path(exe)
    if basename(name) in rules["primary"]:
        return "native:" + basename(name)
    if basename(exe) in rules["native_cli"]:
        return cli_instance_key(rules["native_cli"][basename(exe)], args[1:], True)
    if args and basename(args[0]) == "openclaw-gateway":
        return "openclaw:gateway-title"
    index = entry_index(args)
    if index is not None:
        if suffix(path(args[index]), "dsh desktop/resources/harness-node-entry.mjs"):
            index += 1
        if index < len(args):
            provider = next((provider for expected, provider in sorted(rules["entries"].items()) if suffix(path(args[index]), expected)), None)
            if provider in ("cline", "kilo", "crush", "hermes"):
                return cli_instance_key(provider, args[index + 1:], False)
            digest = hashlib.sha256()
            for arg in args[index + 1:]:
                digest.update(arg.encode("utf-8"))
                digest.update(b"\0")
            return canonical_entry(path(args[index])) + ":" + digest.hexdigest()
    return path(exe)

def cli_instance_key(provider, args, native):
    digest = hashlib.sha256()
    for arg in args:
        digest.update(arg.encode("utf-8"))
        digest.update(b"\0")
    return ("native:" if native else "launcher:") + provider + ":" + digest.hexdigest()

def canonical_entry(entry):
    for expected, provider in sorted(rules["entries"].items()):
        if provider in ("qwen", "openclaw") and suffix(entry, expected):
            package = "@qwen-code/qwen-code" if expected.startswith("@qwen-code/") else "qwen-code" if provider == "qwen" else "openclaw"
            return entry[:-len(expected)] + package
    return entry

def native_launcher_pair(provider, parent_key, key):
    if provider in rules["native_cli"].values():
        prefix = "launcher:" + provider + ":"
        if parent_key.startswith(prefix):
            return key == "native:" + provider + ":" + parent_key[len(prefix):]
        if provider == "hermes" and parent_key.startswith("native:hermes:"):
            return key == "launcher:hermes:" + parent_key[len("native:hermes:"):]
    launcher = {"codex": "/@openai/codex/bin/codex.js:",
                "claude": "/@anthropic-ai/claude-code/cli.js:",
                "opencode": "/opencode-ai/bin/opencode:"}.get(provider)
    return bool(launcher and key == "native:" + provider and launcher in parent_key)

def counts(rows):
    identities = {row["pid"]: row for row in rows}
    result = {}
    for row in rows:
        parent = identities.get(row.get("parent"))
        if (parent and parent["provider"] == row["provider"]
                and ((parent["key"] == row["key"] and not row["key"].startswith("native:"))
                     or native_launcher_pair(row["provider"], parent["key"], row["key"])
                     or (row["provider"] == "openclaw" and row["key"] == "openclaw:gateway-title"))):
            continue
        result[row["provider"]] = result.get(row["provider"], 0) + 1
    return result

def observe():
    deadline = time.monotonic() + 2.8
    rows, complete, candidates = [], True, 0
    for item in os.scandir("/proc"):
        if not item.name.isdigit():
            continue
        if time.monotonic() >= deadline:
            complete = False
            break
        try:
            with open(item.path + "/comm", "rb") as handle:
                name = handle.read(128).decode("utf-8", "replace").strip()
            if not candidate(name):
                continue
            with open(item.path + "/stat", "rb") as handle:
                stat = handle.read(4096).decode("utf-8", "replace")
            stat_fields = stat.rsplit(") ", 1)[1].split()
            if stat_fields[0] in ("Z", "X", "x"):
                continue
            candidates += 1
            if candidates > 512:
                complete = False
                break
            parent = int(stat_fields[1])
            # Read only selected candidates, including cwd only to resolve a
            # relative entry. No environment, history or credentials are read.
            provider = classify(name, "", [])
            exe = "" if provider else os.readlink(item.path + "/exe")
            provider = provider or classify(name, exe, [])
            args = []
            if provider is None:
                with open(item.path + "/cmdline", "rb") as handle:
                    raw = handle.read(MAX_BYTES + 1)
                candidate_complete = True
                if len(raw) > MAX_BYTES:
                    candidate_complete = False
                    raw = raw[:MAX_BYTES]
                fields = raw.split(b"\0")
                if raw and raw[-1:] != b"\0":
                    fields.pop()  # Never classify a truncated argument.
                    candidate_complete = False
                if fields and not fields[-1]:
                    fields.pop()
                args, intact = bounded_args([value.decode("utf-8", "replace") for value in fields])
                candidate_complete = candidate_complete and intact and bool(args)
                if not candidate_complete:
                    complete = False
                    continue
                if relative_entry(args):
                    resolve_relative_entry(args, os.readlink(item.path + "/cwd"))
                provider = classify(name, exe, args)
                if not args:
                    complete = False
            if provider:
                rows.append({"pid": int(item.name), "parent": parent, "provider": provider,
                             "key": instance_key(name, exe, args)})
        except (OSError, ValueError, IndexError):
            # A vanished process is a normal enumeration race. A live but
            # inaccessible candidate is unknown, never a confirmed zero.
            if os.path.exists(item.path):
                complete = False
    return {"counts": counts(rows), "complete": complete}

if len(sys.argv) > 2 and sys.argv[2] == "--fixture":
    fixtures = json.load(sys.stdin)
    rows, complete = [], True
    for row in fixtures:
        if row.get("state", "R") in ("Z", "X", "x"):
            continue
        if row.get("unavailable", False):
            complete = False
            continue
        args, intact = bounded_args(row["args"])
        if not intact:
            complete = False
            continue
        resolve_relative_entry(args, row.get("cwd", ""))
        provider = classify(row["name"], row["exe"], args)
        if provider:
            rows.append({"pid": row["pid"], "parent": row.get("parent"), "provider": provider,
                         "key": instance_key(row["name"], row["exe"], args)})
    result = {"counts": counts(rows), "complete": complete}
else:
    result = observe()
print(json.dumps(result, separators=(",", ":")))
