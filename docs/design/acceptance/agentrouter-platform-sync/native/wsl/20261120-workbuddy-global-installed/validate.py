#!/usr/bin/env python3
"""Validate retained evidence offline; optionally rehash installed public files.

Never import/run archived probes or launch an application.
"""
import argparse
import ast
import copy
import hashlib
import json
import struct
from pathlib import Path

BASE = Path(__file__).resolve().parent
BUCKET = "https://codebuddy-1328495429.cos.accelerate.myqcloud.com"
URL = BUCKET + "/workbuddy/saas/linux-x64/WorkBuddy-linux-x64-5.7.6.40488862-3e43260f.deb"
PAYLOAD_SHA = "3cf1244520d54eca391378a29579918122006f5bfdba942fe977b908365e93db"
PAYLOAD_BYTES = 421733316
ASAR_PACKAGE_SHA = "096b2dbd19a33671652255bd40da0d728eb6618a029443cfeed61adadc223e18"
PRODUCT_SHA = "26271e75c2af04beadb45ba665caa3facd0ebe328d01d1466eb0ff235b09b609"
ELF_SHA = "13bf1da1273c22f0536ee6a52fbadb3561dd31b63c11e80f7feff6beccac1578"
CONTROLLER_SHA = "199c74d5a547ccb8a3e48bd6e3b22356898caf6b685519b9878def59063ee491"
HASH = lambda raw: hashlib.sha256(raw).hexdigest()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def file_hash(path):
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(4 * 1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def load_evidence(base):
    mapping = {
        "summary": "summary-safe.json",
        "publisher": "official-bucket-publisher-binding-safe.json",
        "discovery": "candidate-discovery-safe.json",
        "download": "official-payload-download-safe.json",
        "extract": "native-prefix-extraction-safe.json",
        "install": "native-international-installation-safe.json",
        "identity": "actual-linux-identity-safe.json",
        "preservation": "preservation-and-profile-final-safe.json",
        "gui": "raw/gui-native-result-safe.json",
        "outer": "raw/gui-outer-execution-safe.json",
        "completion": "gui-native-completion-derived-safe.json",
        "version": "failures/version-without-display-safe.json",
        "headless": "failures/headless-gtk-startup-safe.json",
        "gui1": "failures/gui-attempt1-outer-safe.json",
        "gui2": "failures/gui-attempt2-outer-safe.json",
        "review": "raw/independent-install-review-before-gui-safe.json",
    }
    evidence = {key: json.loads((base / path).read_bytes()) for key, path in mapping.items()}
    evidence["raw_gui"] = (base / mapping["gui"]).read_bytes()
    evidence["raw_package"] = (base / "sources/actual-linux-package.json").read_bytes()
    evidence["publisher_html"] = (base / "sources/official-codebuddy-home.html").read_bytes()
    evidence["manifest_source"] = (base / "sources/discovery-community-manifest.yml").read_bytes()
    evidence["controller"] = (base / "sources/gui-isolated-controller.py").read_bytes()
    evidence["runner"] = (base / "sources/run-isolated-gui.py").read_bytes()
    evidence["failure_bytes"] = {
        item["path"]: (base / item["path"]).read_bytes()
        for item in evidence["summary"]["failedAttempts"]
    }
    return evidence


def validate_semantics(e):
    s, p, d, i, a, g, o, c, r = (
        e[key] for key in ["summary", "publisher", "download", "install", "identity", "gui", "outer", "completion", "preservation"]
    )
    require(s["status"] == "official-international-linux-native-installed-and-isolated-gui-normal-quit-verified", "wrong acceptance boundary")
    require(s["officialPayloadUrl"] == d["requestedUrl"] == d["finalUrl"] == URL, "wrong actual vendor URL")
    require(s["packageSha256"] == d["sha256"] == PAYLOAD_SHA and s["packageBytes"] == d["bytesRead"] == PAYLOAD_BYTES, "payload identity mismatch")
    require(d["httpStatus"] == 200 and d["downloadComplete"] is True and d["tlsVerificationDisabled"] is False, "download not complete/trusted HTTPS")
    require(p["requestedUrl"] == p["finalUrl"] == "https://www.codebuddy.ai/" and p["httpStatus"] == 200 and p["tlsVerificationDisabled"] is False, "publisher source mismatch")
    require(HASH(e["publisher_html"]) == p["sha256"] and len(e["publisher_html"]) == p["bytesRead"], "publisher body binding mismatch")
    require(BUCKET in p["exactBucketReferences"] and BUCKET.encode() in e["publisher_html"], "exact bucket lacks official binding")
    require((BUCKET + "/web/ide/logo.svg").encode() in e["publisher_html"], "official asset bucket binding absent")
    require(URL.encode() in e["manifest_source"] and PAYLOAD_SHA.encode() in e["manifest_source"], "exact discovery manifest source absent")
    requests = {item["name"]: item for item in e["discovery"]["requests"]}
    manifest = requests["flatpark-public-manifest"]
    require(HASH(e["manifest_source"]) == manifest["sha256"] and manifest["httpStatus"] == 200, "discovery source binding mismatch")
    candidate = requests["candidate-exact-upstream-range"]
    require(candidate["requestedUrl"] == URL and candidate["httpStatus"] == 206 and candidate["startsWithDebArMagic"] is True, "candidate range proof mismatch")
    require(e["discovery"]["thirdPartyBinaryUsed"] is False and e["discovery"]["arbitraryUrlConstruction"] is False, "community binary/guessed URL used")
    require(e["extract"]["extractExit"] == 0 and e["extract"]["installerScriptsExecuted"] is False, "payload-only extraction absent")
    require("Package: workbuddyai\n" in e["extract"]["metadata"] and "Homepage: https://www.workbuddy.ai\n" in e["extract"]["metadata"], "actual Debian identity mismatch")
    require(a["actualElf"]["firstFourBytesHex"] == "7f454c46" and a["actualElf"]["elfClass"] == 2 and a["actualElf"]["elfMachine"] == 62, "not native x86-64 ELF")
    require(a["actualElf"]["sha256"] == i["elf"]["sha256"] == ELF_SHA, "actual ELF binding mismatch")
    require(a["actualProductJson"]["sha256"] == i["productJson"]["sha256"] == PRODUCT_SHA, "actual product binding mismatch")
    require(HASH(e["raw_package"]) == a["actualAsarPackageEntry"]["sha256"] == ASAR_PACKAGE_SHA, "actual ASAR package entry mismatch")
    package = json.loads(e["raw_package"])
    require(package["version"] == "5.7.6" and package["productName"] == "WorkBuddy AI" and package["homepage"] == "https://www.workbuddy.ai", "actual ASAR international product mismatch")
    expected = {"productName": "WorkBuddy AI", "applicationName": "workbuddy-ai", "isOversea": True,
                "dataFolderName": ".workbuddy-ai", "urlProtocol": "workbuddy-ai", "updateUrl": "https://www.workbuddy.ai", "deploymentType": "SaaS",
                "commit": "3e43260f771393d75c6504db99e751eb03dea8db", "genieVersion": "5.7.6"}
    for key, value in expected.items():
        require(a["selectedProduct"][key] == value and i["identity"].get(key) == value, "international identity mismatch: " + key)
    require(a["selectedProductConfig"]["customUserDataDir"] == i["identity"]["customUserDataDir"] == ".workbuddy-ai", "primary profile identity mismatch")
    require(a["runtimeDependencies"]["lddExitCode"] == 0 and a["runtimeDependencies"]["missing"] == [], "runtime libraries missing")
    require(a["nativeWindowsSourceUsedAsLinuxProof"] is False, "Windows source promoted to Linux proof")
    require(i["launcher"]["path"] == "/home/han/.local/bin/workbuddy-ai" and i["desktop"]["path"].endswith("/orchester-workbuddy-ai.desktop"), "distinct launcher absent")
    require(HASH(i["launcher"]["content"].encode()) == i["launcher"]["sha256"] and HASH(i["desktop"]["content"].encode()) == i["desktop"]["sha256"], "launcher content binding mismatch")
    require('exec "/home/han/.local/opt/workbuddy-ai/5.7.6.40488862-3e43260f/opt/WorkBuddy AI/workbuddyai" "$@"' in i["launcher"]["content"], "wrong launcher executable")
    before = {item["path"]: item for item in i["domesticPreservationBefore"]}
    require(len(before) == 3 and len(r["domesticPreservationAfter"]) == 3, "domestic preservation coverage mismatch")
    for after in r["domesticPreservationAfter"]:
        require(after["matchesBefore"] is True and after["bytes"] == before[after["path"]]["bytes"] and after["sha256"] == before[after["path"]]["sha256"], "domestic artifact changed")
    require(r["allDomesticArtifactsByteUnchanged"] is True and r["freshTestHomePrimaryProfileExists"] is True, "preservation/profile proof missing")
    helper = r["freshTestHomeDomesticNamedHelperFiles"]
    require(len(helper) == 2 and all(item["path"].startswith(".workbuddy/logs/") for item in helper), "shared helper namespace disclosure absent")
    require(s["allHelperPathsEditionSpecificClaimed"] is False and s["helperProfileNamespaceAlsoCreated"] == ".workbuddy/logs", "helper namespace overclaimed")
    require(g["status"] == "pass" and g["appNativeExitCode"] == o["actualOuterExitCode"] == c["actualAppExitCode"] == c["actualOuterExitCode"] == 0, "nonzero native GUI exit")
    require(g["appSignals"] == c["appSignals"] == [] and g["normalProductQuitVerified"] is True and c["normalProductCtrlQQuitVerified"] is True, "normal product quit absent")
    require(g["nativeKeyboardActions"] == [{"keys": "ctrl+q", "exitCode": 0, "targetOwnedDisplay": True}], "wrong quit/input scope")
    require(g["mappedWindowObserved"] is True and g["windowTitle"] == c["windowTitle"] == "WorkBuddy AI", "native international window missing")
    require(g["windowGeometry"]["WIDTH"] == 1199 and g["windowGeometry"]["HEIGHT"] == 799 and g["windowGeometry"] == c["windowGeometry"], "window geometry mismatch")
    require(g["freshInternationalProfileCreated"] is True and g["domesticProfileCreated"] is True and c["freshHomeAlsoContainsDomesticNamedDirectory"] is True, "actual profile observation mismatch")
    require(g["networkNamespaceIsolated"] is True and g["originalHomeHidden"] is True, "test isolation absent")
    require("--unshare-all" in o["command"] and "--clearenv" in o["command"] and ["--tmpfs", "/home"] == o["command"][o["command"].index("/home") - 1:o["command"].index("/home") + 1], "namespace command boundary absent")
    require(HASH(e["controller"]) == o["controllerSha256"] == CONTROLLER_SHA, "actual GUI controller mismatch")
    require(HASH(e["raw_gui"]) == c["sourceReceiptSha256"] and c["originalGuiReceiptPreserved"] is True, "raw GUI receipt changed")
    identities = g["pidfdIdentities"]
    excluded = c["excludedPidNamespaceInit"]
    require(excluded["pid"] == 1 and excluded["exeBasename"] == "bwrap", "infrastructure exclusion changed")
    init = [item for item in identities if item["pid"] == 1]
    require(len(init) == 1 and init[0]["exeBasename"] == "bwrap" and init[0]["terminal"] is False and init[0]["birthTicks"] == excluded["birthTicks"], "PID1 caveat erased")
    children = [item for item in identities if item["pid"] not in [1, g["xvfbPid"]]]
    require(len(children) == c["observedNonInfrastructureIdentityCount"] == 13 and all(item["terminal"] is True for item in children), "observed native children not terminal")
    require(g["allObservedAppPidfdsTerminalBeforeXvfbStop"] is False and g["allObservedPidfdsTerminalAfterOwnCleanup"] is False, "raw infrastructure observations rewritten")
    require(c["observedAllNonInfrastructureAppIdentitiesTerminal"] is True and g["ownedXvfbExitCode"] == 0, "derived process completion unsupported")
    require(e["version"]["exit"] == e["headless"]["exit"] == 139 and e["gui1"]["actualOuterExitCode"] == e["gui2"]["actualOuterExitCode"] == 1, "failed attempts erased")
    for failure in s["failedAttempts"]:
        raw = e["failure_bytes"][failure["path"]]
        require(HASH(raw) == failure["sha256"] and len(raw) == failure["bytes"] and failure["countsAsSuccessfulVersionOrGuiProof"] is False, "failure raw binding/claim changed")
    for key in ["communityBinaryInstalled", "arbitraryPackageUrlGuessed", "versionOnlyCommandSupportedClaimed", "currentWebsiteLinuxButtonAvailableClaimed",
                "allOfficialChannelsWorkingClaimed", "latestVersionClaimed", "accountLoginTested", "regionalServiceAvailabilityTested", "windowsApplicationsTouched", "nativePackageMaintainerScriptsExecuted"]:
        require(s[key] is False, "unsupported claim: " + key)
    require(s["modelCalls"] == s["authenticationActions"] == s["existingPrivateProfileReads"] == 0, "account/model/private profile boundary changed")
    ast.parse(e["controller"])
    ast.parse(e["runner"])


def validate_manifest(base):
    manifest = json.loads((base / "artifact-manifest.json").read_bytes())
    rows = manifest["artifacts"]
    paths = [item["path"] for item in rows]
    require(len(paths) == len(set(paths)), "duplicate manifest entry")
    for item in rows:
        relative = Path(item["path"])
        require(not relative.is_absolute() and ".." not in relative.parts, "unsafe artifact path")
        raw = (base / relative).read_bytes()
        require(len(raw) == item["bytes"] and HASH(raw) == item["sha256"], "artifact digest mismatch: " + str(relative))
    actual = {str(path.relative_to(base)) for path in base.rglob("*") if path.is_file()}
    require(actual == set(paths) | {"artifact-manifest.json", "verification-safe.json"}, "unexpected/missing artifact")
    require(not any(path.lower().endswith((".deb", ".asar", ".exe", ".dll", ".pyc")) for path in actual), "binary/private cache archived")
    return len(rows)


def negative_checks(e):
    mutations = [
        ("publisher bucket", lambda x: x["publisher"].update(exactBucketReferences=[])),
        ("exact download URL", lambda x: x["download"].update(finalUrl=URL.replace("linux-x64/", "linux-x64-deb/"))),
        ("payload digest", lambda x: x["download"].update(sha256="0" * 64)),
        ("partial payload", lambda x: x["download"].update(bytesRead=1024)),
        ("ASAR package source", lambda x: x.update(raw_package=x["raw_package"] + b"\n")),
        ("domestic product substitution", lambda x: x["identity"]["selectedProduct"].update(isOversea=False)),
        ("primary namespace", lambda x: x["identity"]["selectedProductConfig"].update(customUserDataDir=".workbuddy")),
        ("ELF identity", lambda x: x["identity"]["actualElf"].update(firstFourBytesHex="4d5a0000")),
        ("preservation", lambda x: x["preservation"]["domesticPreservationAfter"][0].update(sha256="0" * 64)),
        ("native GUI exit", lambda x: x["gui"].update(appNativeExitCode=139)),
        ("quit action", lambda x: x["gui"]["nativeKeyboardActions"][0].update(keys="ctrl+w")),
        ("app force signal", lambda x: x["gui"]["appSignals"].append({"signal": "SIGKILL"})),
        ("live app child", lambda x: x["gui"]["pidfdIdentities"][3].update(terminal=False)),
        ("raw PID1 caveat", lambda x: x["gui"]["pidfdIdentities"][0].update(terminal=True)),
        ("controller source", lambda x: x.update(controller=x["controller"] + b"\n# mutation\n")),
        ("failed version promoted", lambda x: x["summary"].update(versionOnlyCommandSupportedClaimed=True)),
        ("helper paths overclaim", lambda x: x["summary"].update(allHelperPathsEditionSpecificClaimed=True)),
        ("failed raw receipt", lambda x: x["failure_bytes"].update({next(iter(x["failure_bytes"])): b"{}\n"})),
        ("account/model overclaim", lambda x: x["summary"].update(accountLoginTested=True)),
    ]
    for name, mutate in mutations:
        changed = copy.deepcopy(e)
        mutate(changed)
        try:
            validate_semantics(changed)
        except (ValueError, KeyError):
            continue
        raise ValueError("negative case accepted: " + name)
    return len(mutations)


def verify_live(e):
    i, a = e["install"], e["identity"]
    rows = [a["actualElf"], a["actualAsar"], a["actualProductJson"],
            i["launcher"], i["desktop"], *e["preservation"]["domesticPreservationAfter"]]
    for row in rows:
        path = Path(row["path"])
        require(file_hash(path) == row["sha256"], "live file digest mismatch: " + str(path))
        if "bytes" in row:
            require(path.stat().st_size == row["bytes"], "live file size mismatch: " + str(path))
    payload = Path(e["download"]["localPath"])
    require(payload.stat().st_size == PAYLOAD_BYTES and file_hash(payload) == PAYLOAD_SHA, "live vendor payload mismatch")
    with Path(a["actualElf"]["path"]).open("rb") as stream:
        header = stream.read(64)
    require(header[:4] == b"\x7fELF" and header[4] == 2 and struct.unpack_from("<H", header, 18)[0] == 62, "live native ELF mismatch")
    product = json.loads(Path(a["actualProductJson"]["path"]).read_bytes())
    require(all(product[key] == value for key, value in a["selectedProduct"].items()), "live selected product differs")
    require(product["config"]["customUserDataDir"] == ".workbuddy-ai", "live namespace mismatch")
    with Path(a["actualAsar"]["path"]).open("rb") as stream:
        header = stream.read(16)
        tree = json.loads(stream.read(struct.unpack_from("<I", header, 12)[0]))
        entry = tree["files"]["package.json"]
        stream.seek(8 + struct.unpack_from("<I", header, 4)[0] + int(entry["offset"]))
        package = stream.read(entry["size"])
    require(package == e["raw_package"], "live actual ASAR package source differs")
    return len(rows) + 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--live-install", action="store_true")
    args = parser.parse_args()
    evidence = load_evidence(BASE)
    validate_semantics(evidence)
    count = validate_manifest(BASE)
    negatives = negative_checks(evidence) if args.self_test else 0
    live = verify_live(evidence) if args.live_install else 0
    print(json.dumps({"status": "passed", "artifactDigestsVerified": count,
                      "semanticNegativeCasesRejected": negatives, "actualPublicFilesRehashed": live,
                      "applicationExecutionsByValidator": 0, "accountOrModelAcceptanceClaimed": False,
                      "successfulVersionOnlyCommandClaimed": False}, indent=2))


if __name__ == "__main__":
    main()
