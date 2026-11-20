#!/usr/bin/env python3
"""Validate retained evidence offline. Never import or run observed probes."""
import argparse
import ast
import copy
import hashlib
import json
import struct
from pathlib import Path

BASE = Path(__file__).resolve().parent
CMD_SHA = "15cd209c995b7d3009c91ca40892d5b8f1fb162f3e5f10716642ea278e8e0d7b"
PROBE_SHA = "f369fa857fb05b79157429b84b07bc740f83b5adee8944b92f53736a99921b62"
SENTINEL = b"ORCHESTER_NATIVE_ROUTE_OK\r\n"
HASH = lambda b: hashlib.sha256(b).hexdigest()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def load_evidence(base):
    files = {
        "summary": "summary-safe.json",
        "route": "linux-route-observation-safe.json",
        "protocol": "9p-public-file-read-safe.json",
        "identity": "copied-public-command-identity-safe.json",
        "host_identity": "native-host-command-identity-selected-safe.json",
        "native": "native-tool-receipt-safe.json",
        "tcp": "native-tcp-query-selected-safe.json",
        "sources": "source-provenance-safe.json",
        "failed": "direct-init-failures-safe.json",
    }
    result = {k: json.loads((base / v).read_bytes()) for k, v in files.items()}
    result["sentinel"] = (base / "native-sentinel.stdout.txt").read_bytes()
    result["source"] = (base / "sources/native_windows_readonly_probe.py").read_bytes()
    return result


def validate_semantics(e):
    s, r, p, i, n, t = (e[k] for k in ["summary", "route", "protocol", "identity", "native", "tcp"])
    require(s["status"] == "new-read-route-and-native-powershell-sentinel-verified", "wrong acceptance boundary")
    require(s["nativeExecutionAccepted"] is True and s["nativeSentinelAccepted"] is True, "native result absent")
    for key in ["existingMountRepaired", "copiedCmdVerSemanticAccepted", "nativeTcpQueryErrorFreeCertified",
                "portReservedOrDefaultRuntimeAcceptanceProven", "rootCauseEstablished", "sourceMatchesInstalledWslEstablished"]:
        require(s[key] is False, "unsupported claim: " + key)
    require(s["controllerActions"]["windowsNaturalProcessCacheSideEffectsMeasured"] is False, "unmeasured Windows effects promoted")
    require(r["measuredLinuxRealEffectiveSavedFilesystemUids"] == ["1000"] * 4, "Linux identity mismatch")
    require(r["inheritedInteropPath"] == "/run/WSL/2482_interop" and r["inheritedInteropSocketPresentInCapturedListing"] is False, "stale route mismatch")
    require(r["existingMountRepaired"] is False and r["existingCDriveShare"][0]["errno"] == 5, "mount failure erased")
    require(r["otherHostShareEnumerationSucceeded"] is True, "host share scope lost")
    require({x["path"] for x in r["selectedSocketPeers"]} == {"/run/WSL/357_interop", "/run/WSL/628707_interop"}, "peer identity mismatch")
    require(all(x["connected"] and x["peer_uid"] == 0 for x in r["selectedSocketPeers"]), "peer evidence missing")
    require(p["hostCid"] == 2 and p["port"] == 50002 and p["version"] == "9P2000.L", "endpoint/protocol mismatch")
    require(p["serverMsize"] == 65536 and p["walkedPublicCommandComponents"] == 3, "negotiation/walk mismatch")
    require(p["publicCmdBytes"] == 339968 and p["publicCmdInitialBytesRead"] == 4096 and p["publicCmdMZMagic"], "public read mismatch")
    frames = p["protocolFrames"]
    require([x["requestType"] for x in frames] == [100, 104, 110, 24, 12, 116, 120, 120], "unexpected/write operation")
    require(all(x["responseType"] == x["requestType"] + 1 and 7 <= x["responseBytes"] <= 65536 for x in frames), "invalid reply metadata")
    require([x["tag"] for x in frames] == [65535, 1, 2, 3, 4, 5, 6, 7], "reply tag mismatch")
    require("readOnlyRootAttachSucceeded" not in p and p["attachSucceeded"] is True, "attach read-only claim retained")
    measured = i["independentLinuxArtifactCheck"]
    require(i["declaredReadBytes"] == measured["bytes"] == 339968, "byte count mismatch")
    require(i["declaredReadSha256"] == measured["sha256"] == CMD_SHA, "acquired binary digest mismatch")
    require(measured["mzMagic"] and measured["peSignatureHex"] == "50450000" and measured["coffMachine"] == "0x8664" and measured["optionalHeaderMagic"] == "0x20b", "PE identity mismatch")
    require(i["binaryArchived"] is False and i["copiedCmdVerProbe"]["semanticAccepted"] is False, "binary/ver boundary mismatch")
    host = e["host_identity"]
    pin = host["nativeSystemCommandImagePin"]
    require(pin["path"].lower() == r"c:\windows\system32\cmd.exe" and pin["sha256"] == CMD_SHA, "native host image identity mismatch")
    require(pin["authenticodeStatus"] == "Valid" and pin["signerSubject"] == "CN=Microsoft Windows, O=Microsoft Corporation, L=Redmond, S=Washington, C=US", "native host signature mismatch")
    require(host["externalExecution"]["actualExternalNativeExitCode"] == 0, "native identity preflight failed")
    require(host["externalExecution"]["copiedSystemCmdSha256"] == CMD_SHA, "native bridge copy hash mismatch")
    require(host["externalExecution"]["stdoutSha256"] == host["selectedFrom"]["nativeStdoutSha256"] == "b162fa637af907c1831788dddad20162be29dd81b40757dc38a37686c01529f2", "native identity stdout binding mismatch")
    require(host["externalExecution"]["stdoutBytes"] == host["selectedFrom"]["nativeStdoutBytes"], "native identity stdout byte count mismatch")
    require(host["sourceReceiptStdoutDigestIndependentlyMatched"] is True and host["matchesIndependentLinuxAcquiredByteSha256"] is True, "native identity source comparison absent")
    require(host["observedAtUtc"] == "2026-11-20T03:48:42.1359741Z" and i["windowsHashOrAuthenticodeMeasurementAvailableHere"] is True, "native identity timestamp/copy link missing")
    require(n["externalStatus"] == "completed" and n["externalExitCode"] == 0 and n["externalProcessId"] == "97208", "external completion mismatch")
    require(n["externalCommandId"] == "exec-3bc34559-0ce1-4ffa-bae8-5155d452222c" and n["externalEventOrdinal"] == 1047, "external event identity mismatch")
    require(n["observedAtUtc"] == "2026-11-20T03:28:09.711549+00:00" and n["sourceMatchedExternalHeredocExactly"] is True, "source/time binding mismatch")
    require(HASH(e["source"]) == n["observedSourceSha256"] == PROBE_SHA, "observed source mismatch")
    identity = n["argvIdentity"]
    require(identity["linuxEntryPoint"] == "/init" and identity["interopSocket"] == "/run/WSL/357_interop", "native route identity mismatch")
    require(identity["windowsPowerShellPath"] == r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe", "native executable identity mismatch")
    require(e["sentinel"] == SENTINEL, "actual ASCII stdout mismatch")
    probes = n["nativeProbes"]
    require(len(probes) == 2 and all(x["nativeProcessExitCode"] == 0 for x in probes), "nonzero native exit")
    require(probes[0]["stdoutBytes"] == len(SENTINEL) and probes[0]["stdoutSha256"] == HASH(SENTINEL), "stdout binding mismatch")
    require(probes[0]["actualPowerShellScript"] == "Write-Output 'ORCHESTER_NATIVE_ROUTE_OK'", "wrong executed sentinel")
    require(t["nativeProcessExitCode"] == 0 and t["selectedNativeOutput"] == {"tcpCount": 0, "tcp": [], "ownerCount": 0, "owners": []}, "native port projection mismatch")
    require(t["unselectedWslStatusUsedAsEvidence"] is False, "garbled status promoted")
    require(e["sources"]["installedVersionBindingEstablished"] is False and len(e["sources"]["sourceSnapshots"]) == 7, "upstream source scope mismatch")
    require([x["exitCode"] for x in e["failed"] if x.get("method") == "direct-init-standard-entry"] == [1, 1], "initial failure erased")
    # Parse rather than execute original source. Exact source digest binds its actual
    # subprocess.returncode capture and native cmdlet query to the external event.
    ast.parse(e["source"])


def validate_manifest(base):
    manifest = json.loads((base / "artifact-manifest.json").read_bytes())
    rows = manifest["artifacts"]
    paths = [x["path"] for x in rows]
    require(len(paths) == len(set(paths)), "duplicate manifest path")
    for row in rows:
        rel = Path(row["path"])
        require(not rel.is_absolute() and ".." not in rel.parts, "invalid archive path")
        raw = (base / rel).read_bytes()
        require(len(raw) == row["bytes"] and HASH(raw) == row["sha256"], "artifact digest mismatch: " + str(rel))
    actual = {str(x.relative_to(base)) for x in base.rglob("*") if x.is_file()}
    require(actual == set(paths) | {"artifact-manifest.json", "verification-safe.json"}, "unexpected/missing archived file")
    require(not any(x.lower().endswith((".exe", ".dll", ".pyc")) for x in actual), "binary/cache copied into archive")
    return len(rows)


def negative_checks(e):
    mutations = [
        ("native exit", lambda x: x["native"]["nativeProbes"][0].update(nativeProcessExitCode=1)),
        ("wrong output with consistent hash", lambda x: (x.update(sentinel=b"FAKE\r\n"), x["native"]["nativeProbes"][0].update(stdoutBytes=6, stdoutSha256=HASH(b"FAKE\r\n")))),
        ("binary digest", lambda x: x["identity"]["independentLinuxArtifactCheck"].update(sha256="0" * 64)),
        ("admin endpoint", lambda x: x["protocol"].update(port=50003)),
        ("wrong native path", lambda x: x["native"]["argvIdentity"].update(windowsPowerShellPath="powershell.exe")),
        ("mount repaired", lambda x: x["summary"].update(existingMountRepaired=True)),
        ("TCP query overclaim", lambda x: x["summary"].update(nativeTcpQueryErrorFreeCertified=True)),
        ("wrong source identity", lambda x: x.update(source=x["source"] + b"\n# changed\n")),
        ("native host hash", lambda x: x["host_identity"]["nativeSystemCommandImagePin"].update(sha256="0" * 64)),
        ("native host signature", lambda x: x["host_identity"]["nativeSystemCommandImagePin"].update(authenticodeStatus="UnknownError")),
        ("native host exit", lambda x: x["host_identity"]["externalExecution"].update(actualExternalNativeExitCode=1)),
    ]
    for name, mutate in mutations:
        changed = copy.deepcopy(e)
        mutate(changed)
        try:
            validate_semantics(changed)
        except ValueError:
            continue
        raise ValueError("negative case accepted: " + name)
    return len(mutations)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--private-copy", type=Path)
    args = parser.parse_args()
    evidence = load_evidence(BASE)
    validate_semantics(evidence)
    count = validate_manifest(BASE)
    controls = negative_checks(evidence) if args.self_test else 0
    private_checked = False
    if args.private_copy:
        raw = args.private_copy.read_bytes()
        require(len(raw) == 339968 and HASH(raw) == CMD_SHA, "actual private copy digest mismatch")
        offset = struct.unpack_from("<I", raw, 0x3c)[0]
        require(raw[:2] == b"MZ" and raw[offset:offset+4] == b"PE\x00\x00", "actual private PE mismatch")
        private_checked = True
    print(json.dumps({"status": "passed", "artifactDigestsVerified": count,
                      "semanticNegativeCasesRejected": controls,
                      "actualPrivateCopyIndependentlyRehashed": private_checked,
                      "nativeProbesInvoked": False, "existingMountRepaired": False,
                      "defaultRuntimeAcceptedByThisArchive": False}, indent=2))


if __name__ == "__main__":
    main()
