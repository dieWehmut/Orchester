#!/usr/bin/env python3
"""Offline archive validation only: no installer, GUI, network or user profile reads."""
from __future__ import annotations
import argparse
import datetime as dt
import email.utils
import hashlib
import json
import re
import shutil
import subprocess
import tempfile
from pathlib import Path

FINGERPRINT = "31DDDE24DDFAB679F42D7BD2BAA929FF1A7ECACE"
VERSION = "5.11492.5"
DEB_SHA256 = "b96208b207e450b740f7dbe304f6077fed80e7f3d37ddd545991c4e8201564cf"
ELF_SHA256 = "fe6219735b2e67422a9a23de2fc35371b09980531601c019b7cc2b001e29c7a8"
ASAR_SHA256 = "33a8a05f5704a7159711c7aa9ab99086ea2b04f17ac9e494fb1df54c19a142e6"

def need(value: bool, label: str) -> None:
    if not value:
        raise ValueError(label)

def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        while chunk := stream.read(1024 * 1024):
            h.update(chunk)
    return h.hexdigest()

def load(root: Path, relative: str) -> dict:
    return json.loads((root / relative).read_text())

def safe_relative(relative: str) -> bool:
    p = Path(relative)
    return not p.is_absolute() and ".." not in p.parts and relative == p.as_posix()

def validate(root: Path) -> dict:
    index = load(root, "artifact-index.json")
    expected_paths = {r["path"] for r in index["files"]}
    need(len(expected_paths) == len(index["files"]), "duplicate artifact index path")
    actual_paths = {p.relative_to(root).as_posix() for p in root.rglob("*") if p.is_file()}
    need(actual_paths == expected_paths | {"artifact-index.json"}, "unexpected or missing archive file")
    for row in index["files"]:
        rel = row["path"]
        need(safe_relative(rel), "unsafe artifact index path")
        p = root / rel
        need(not p.is_symlink(), "symlink archive file")
        need(p.stat().st_size == row["bytes"], "artifact size mismatch: " + rel)
        need(sha256(p) == row["sha256"], "artifact SHA256 mismatch: " + rel)
        p.read_bytes().decode("utf-8")
        need(p.suffix.lower() not in {".deb", ".png", ".jpg", ".log", ".gpg", ".exe", ".asar"},
             "excluded artifact extension present: " + rel)
    source_map = load(root, "source-map.json")
    need(len({row["id"] for row in source_map["sources"]}) == len(source_map["sources"]),
         "duplicate source mapping identity")
    need(len({row["archivePath"] for row in source_map["sources"]}) == len(source_map["sources"]),
         "duplicate source mapping archive path")
    need(source_map["privateProfileContentsReadForArchive"] is False, "profile-read claim")
    need(source_map["installationOrGuiReruns"] == 0, "archive performed live action")
    for row in source_map["sources"]:
        need(safe_relative(row["archivePath"]), "unsafe source mapping path")
        p = root / row["archivePath"]
        need(p.stat().st_size == row["archiveBytes"], "source mapping size mismatch")
        need(sha256(p) == row["archiveSha256"], "source mapping SHA mismatch")
        need(bool(re.fullmatch(r"[0-9a-f]{64}", row["sourceSha256"])), "invalid original source hash")
        need(row["sourceBytes"] > 0, "invalid original source size")
        if row["representation"] == "original-bytes":
            need(row["sourceBytes"] == row["archiveBytes"], "original size changed")
            need(row["sourceSha256"] == row["archiveSha256"], "original bytes changed")
        else:
            need(row["representation"] in {"derived-source-export", "derived-text", "derived-selection", "derived-selected-fields"},
                 "unknown source representation")
            need("transformations" in row, "derived artifact has no transformation record")
    need(not re.search(r"'(?:USER|LOGNAME)':'(?!<LOCAL_ACCOUNT>)[^']+'",
                       (root / "controllers/native-gui-smoke.py.txt").read_text()),
         "local account literal remained in derived GUI source")
    need((root / ".gitattributes").read_text() == "* -text\n", "byte-preserving attributes missing")
    need(shutil.which("gpg") is not None and shutil.which("gpgv") is not None,
         "gpg and gpgv required for offline signature verification")
    with tempfile.TemporaryDirectory(prefix="claude-archive-gpg-") as directory:
        temp = Path(directory)
        ring = temp / "keyring.gpg"
        key = root / "repository/key.asc"
        dearmor = subprocess.run(
            ["gpg", "--no-options", "--homedir", str(temp), "--batch", "--no-tty",
             "--dearmor", "--output", str(ring), str(key)],
            capture_output=True, text=True)
        need(dearmor.returncode == 0, "public key dearmor failed")
        verify = subprocess.run(
            ["gpgv", "--homedir", str(temp), "--keyring", str(ring), "--status-fd", "1",
             str(root / "repository/InRelease")],
            capture_output=True, text=True)
        need(verify.returncode == 0, "retained InRelease signature invalid")
        need("[GNUPG:] VALIDSIG " + FINGERPRINT + " " in verify.stdout,
             "signature fingerprint mismatch")
    release = (root / "repository/InRelease").read_text()
    packages = root / "repository/Packages-amd64"
    section = release.split("\nSHA256:\n", 1)[1].split("\nSHA512:\n", 1)[0]
    release_rows = [line.split() for line in section.splitlines() if line.strip()]
    index_rows = [r for r in release_rows if r[2] == "main/binary-amd64/Packages"]
    need(len(index_rows) == 1, "signed Packages row not unique")
    need(index_rows[0][0] == sha256(packages), "Packages not bound to signed InRelease")
    need(int(index_rows[0][1]) == packages.stat().st_size, "signed Packages size mismatch")
    stanzas = []
    for block in packages.read_text().strip().split("\n\n"):
        fields = {}
        for line in block.splitlines():
            if line and not line[0].isspace() and ":" in line:
                key, value = line.split(":", 1)
                fields[key] = value.strip()
        if fields.get("Package") == "claude-desktop" and fields.get("Version") == VERSION:
            stanzas.append(fields)
    need(len(stanzas) == 1, "selected official package not unique")
    selected = stanzas[0]
    need(selected["Architecture"] == "amd64", "selected package architecture")
    need(selected["SHA256"] == DEB_SHA256 and selected["Size"] == "183335272",
         "selected deb SHA/size")
    signed = load(root, "evidence/20261116/signed-repository-verification-safe.json")
    need(signed["gpgvExit"] == 0 and signed["keyFingerprint"] == FINGERPRINT,
         "historical signature result")
    need(signed["packagesSha256"] == sha256(packages), "historical Packages digest")
    need(signed["selectedPackage"] == selected, "selected Packages stanza changed")
    historical_time = dt.datetime.fromisoformat(signed["observedAtUtc"])
    valid_until_text = re.search(r"^Valid-Until: (.+)$", release, re.M).group(1)
    valid_until = email.utils.parsedate_to_datetime(valid_until_text)
    release_date = email.utils.parsedate_to_datetime(re.search(r"^Date: (.+)$", release, re.M).group(1))
    need(release_date <= historical_time <= valid_until, "historical repository freshness")
    need(signed["repositoryNotExpired"] is True, "historical freshness claim")
    incomplete = load(root, "evidence/20261116/linux-deb-download-safe.json")
    need(incomplete["exit"] == 28 and 0 < incomplete["bytes"] < int(selected["Size"]) and
         incomplete["sizeMatchesSignedPackages"] is False and
         incomplete["sha256MatchesSignedPackages"] is False and incomplete["installed"] is False,
         "historical incomplete download must remain a failed preparation attempt")
    download = load(root, "evidence/20261116/linux-deb-resumed-download-safe.json")
    need(download["curlExit"] == 0 and download["httpStatus"] == 206, "complete resumed download")
    need(download["bytes"] == int(selected["Size"]) and download["sha256"] == DEB_SHA256,
         "historical deb digest binding")
    need(download["sizeMatchesSignedPackages"] and download["sha256MatchesSignedPackages"],
         "complete download did not match signed package")
    install = load(root, "evidence/20261116/install-result-safe.json")
    need(install["mode"] == "install" and install["actualInstallationInvocations"] == 1,
         "actual APT invocation absent")
    need(install["aptInstallation"]["exitCode"] == 0 and install["exitCode"] == 0,
         "actual native APT exit")
    need(install["postInstallPackage"]["exitCode"] == 0 and
         install["postInstallPackage"]["stdout"].splitlines() == ["install ok installed", VERSION, "amd64"],
         "actual dpkg installation identity")
    need(install["preinstall"]["debSHA256"] == DEB_SHA256 and
         install["preinstall"]["debBytes"] == int(selected["Size"]),
         "installed package is not selected signed deb")
    need(install["preinstall"]["repositorySignatureExitCode"] == 0 and
         install["preinstall"]["repositoryValidFingerprint"] == FINGERPRINT and
         install["preinstall"]["packagesSHA256"] == sha256(packages), "install signature/index binding")
    need(install["nativeElf"]["sha256"] == ELF_SHA256 and
         install["nativeElf"]["bytes"] == 234466632, "installed ELF identity")
    static = load(root, "evidence/20261116/linux-deb-static-inspection-safe.json")
    static_elf = [r for r in static["nativeExecutablesAndLinks"]
                  if r["relativePath"] == "usr/lib/claude-desktop/claude-desktop"]
    need(len(static_elf) == 1 and static_elf[0]["sha256"] == ELF_SHA256 and
         static_elf[0]["bytes"] == install["nativeElf"]["bytes"], "installed/extracted ELF mismatch")
    version = load(root, "evidence/20261116/native-version-result-safe.json")
    need(version["actualExitCode"] == 0 and version["versionOutput"] == VERSION and version["success"],
         "actual native --version result")
    need(version["installedElfSHA256"] == ELF_SHA256, "native version ELF identity binding")
    need(version["installedASARSHA256"] == ASAR_SHA256 and
         version["installedASARMatchesSignedPackage"], "native ASAR package binding")
    need(len(version["heldObjects"]) == 4 and
         all(r["pidfdHeld"] and r["terminalByOriginalPidfd"] for r in version["heldObjects"]),
         "native version held-process terminal proof")
    need(len({(r["pid"], r["startTicks"]) for r in version["heldObjects"]}) == 4 and
         any(r["pid"] == version["root"]["pid"] and r["startTicks"] == version["root"]["startTicks"]
             for r in version["heldObjects"]) and version["root"]["pidfdHeld"] and
         version["root"]["expectedInstalledElfSHA256"] == ELF_SHA256,
         "native version distinct process/root identities")
    need(version["signalCleanupActions"] == [] and version["loginOrModelActions"] == 0,
         "version cleanup/login/model actions")
    gui_original = load(root, "evidence/20261119/native-gui-result-original-safe.json")
    gui = load(root, "evidence/20261119/native-gui-result-safe.json")
    original_gui = root / "evidence/20261119/native-gui-result-original-safe.json"
    for receipt in (gui_original, gui):
        need(receipt["installedElfSHA256"] == ELF_SHA256 and receipt["installedASARSHA256"] == ASAR_SHA256,
             "GUI ELF/ASAR identity binding")
    common = set(gui_original) & set(gui)
    changed = {k for k in common if gui_original[k] != gui[k]}
    removed = set(gui_original) - set(gui)
    added = set(gui) - set(gui_original)
    need(changed == {"installedVersion"} and removed == {"userProfileReads"}, "undeclared GUI original-field change")
    need(added == {"installedPackageVersion", "testControllerUserProfileReads", "renderedStateEvidence",
                   "loginOrModelAvailabilityProven", "successScope", "packagedElectronRuntimeVersion",
                   "dpkgIdentity", "metadataCorrection", "renderedState"}, "undeclared GUI metadata addition")
    need(gui_original["userProfileReads"] == gui["testControllerUserProfileReads"] == 0, "controller privacy counter rename")
    need(gui["installedPackageVersion"] == VERSION and gui["packagedElectronRuntimeVersion"] == "45.2.1",
         "GUI package/runtime label split")
    need(gui["dpkgIdentity"].split(chr(9)) == ["claude-desktop", VERSION, "install ok installed"],
         "GUI dpkg identity metadata")
    audit = load(root, "evidence/20261119/gui-metadata-change-audit-derived-safe.json")
    need(audit["originalReceiptSha256"] == sha256(original_gui) and
         audit["correctedSourceReceiptSha256"] == sha256(root / "evidence/20261119/native-gui-result-safe.json"),
         "full GUI audit source hashes")
    need(audit["changedCommonFields"] == {k: {"originalValue": gui_original[k], "correctedValue": gui[k]} for k in changed}
         and audit["removedOriginalFields"] == {k: gui_original[k] for k in removed}
         and audit["addedMetadataFields"] == {k: gui[k] for k in added}, "full GUI metadata change audit")
    need(audit["allOtherOriginalFieldsUnchanged"] and audit["guiRerunPerformed"] is False, "GUI audit scope")

    correction = gui["metadataCorrection"]
    need(correction["originalReceiptSHA256"] == sha256(original_gui), "GUI correction original hash")
    need(gui_original["installedVersion"] == "45.2.1" and correction["oldValue"] == "45.2.1",
         "original metadata error not retained")
    need(gui["installedVersion"] == VERSION and correction["newValue"] == VERSION and
         correction["guiRerunPerformed"] is False, "GUI correction invalid")
    for field in ("startedAtUtc", "finishedAtUtc", "ownedWindow", "ownedWindowObservations",
                  "image", "normalProductQuitActions", "actualExitCode", "heldObjects"):
        need(gui_original[field] == gui[field], "GUI factual receipt field modified: " + field)
    need(gui["actualExitCode"] == 0 and gui["actualNativeGuiLaunches"] == 1 and gui["uid"] == 1000,
         "actual GUI launch/exit")
    need(gui["profileInitiallyEmpty"] and gui["realHomePassed"] is False and
         gui["providerSecretEnvironmentKeysPassed"] is False, "GUI profile/environment isolation")
    window = gui["ownedWindow"]
    need(window["mapped"] and window["width"] == 1200 and window["height"] == 800,
         "actual mapped native WSLg window")
    need(gui["success"] is True and gui["preexistingClaudeNativeProcessCount"] == 0 and
         gui["x11ErrorCount"] == 0 and window["titleContainsClaude"] and
         window in gui["ownedWindowObservations"], "successful owned-window observation")
    need(gui["image"]["actualXGetImage"] and gui["image"]["sampledDistinctColors"] > 1,
         "actual owned-window image observation")
    need(gui["image"]["width"] == window["width"] and gui["image"]["height"] == window["height"],
         "owned-window/image geometry mismatch")
    need(len(gui["heldObjects"]) == 16 and
         all(r["originalPidfdHeld"] and r["terminalByOriginalPidfd"] for r in gui["heldObjects"]),
         "GUI all original pidfds terminal")
    need(len({(r["pid"], r["startTicks"]) for r in gui["heldObjects"]}) == 16 and
         any(r["pid"] == window["pid"] and r["nativeElf"] for r in gui["heldObjects"]),
         "GUI distinct process/window identities")
    need(len(gui["normalProductQuitActions"]) == 1 and
         gui["normalProductQuitActions"][0]["sourceVerifiedAccelerator"] == "Ctrl+Q" and
         gui["normalProductQuitActions"][0]["eventsAccepted"] == [1, 1], "normal product quit action")
    quit_action = gui["normalProductQuitActions"][0]
    need(quit_action["window"] == window["window"] and quit_action["pid"] == window["pid"] and
         quit_action["globalInputActions"] == 0 and
         dt.datetime.fromisoformat(gui["startedAtUtc"]) <= dt.datetime.fromisoformat(quit_action["atUtc"]) <=
         dt.datetime.fromisoformat(gui["finishedAtUtc"]), "normal quit must target the observed owned window")
    need(gui["signalCleanupActions"] == [] and gui["loginOrModelActions"] == 0,
         "GUI signal/login/model actions")
    need(gui["renderedState"] == "Official App unavailable in region page" and
         gui["loginOrModelAvailabilityProven"] is False, "online availability overclaimed")
    final = load(root, "evidence/20261119/final-native-linux-verification-safe.json")
    reconciliation = load(root, "evidence/20261119/post-gui-native-package-reconciliation-safe.json")
    need(reconciliation["packageName"] == "@ant/desktop" and reconciliation["productName"] == "Claude" and
         reconciliation["asarPackageVersion"] == VERSION and reconciliation["dpkgExitCode"] == 0 and
         reconciliation["dpkgOutput"] == gui["dpkgIdentity"] and
         reconciliation["packagedElectronRuntimeVersion"] == gui["packagedElectronRuntimeVersion"],
         "post-GUI package/runtime correction evidence")
    need(reconciliation["nativeProcessCount"] == 0 and reconciliation["allHeldProcessesTerminal"] and
         reconciliation["receiptSuccess"] and reconciliation["controllerSHA256"] == final["originalControllerSHA256"],
         "post-GUI lifecycle/controller reconciliation")
    need(final["installedPackageVersion"] == VERSION and final["nativeGuiSmokeSuccess"] and
         final["renderedState"] == gui["renderedState"], "final package/GUI identity")
    need(final["dpkgVerifyExitCode"] == 0 and final["dpkgVerifyMismatchCount"] == 0,
         "native package integrity result")
    need(final["nativeGuiReceiptSHA256"] == sha256(root / "evidence/20261119/native-gui-result-safe.json"),
         "final corrected GUI receipt hash")
    need(final["observedRootExitCode"] == 0 and final["heldObjectCount"] == 16 and
         final["allHeldObjectsTerminal"] and final["nativeProcessCountAfterExit"] == 0,
         "final normal native exit state")
    need(final["loginOrModelAvailabilityProven"] is False and final["signalCleanupActionCount"] == 0,
         "final availability or cleanup overclaim")
    source_controller = next(r for r in source_map["sources"]
                             if r["archivePath"] == "controllers/native-gui-smoke.py.txt")
    need(source_controller["sourceSha256"] == final["originalControllerSHA256"],
         "original GUI controller source hash")
    omitted = load(root, "omitted-artifacts-safe.json")
    need(omitted["binaryOrRawLogOrImageBytesArchived"] is False and
         omitted["freshProfiles"]["contentsReadForArchive"] is False and
         omitted["userProfiles"]["contentsReadForArchive"] is False, "excluded/private archive policy")
    images = [r for r in omitted["artifacts"] if r["sourceId"] == "own-WSLg-window-image"]
    need(len(images) == 1 and images[0]["bytes"] == gui["image"]["bytes"] and
         images[0]["sha256"] == gui["image"]["sha256"], "omitted image binding")
    return {"status": "pass", "archiveFiles": len(actual_paths),
            "mappedSourceArtifacts": len(source_map["sources"]),
            "retainedInReleaseSignatureVerifiedOffline": True,
            "signerFingerprint": FINGERPRINT, "packageVersion": VERSION,
            "historicalAptNativeExitCode": 0, "historicalVersionNativeExitCode": 0,
            "historicalGuiNativeExitCode": 0, "historicalGuiPidfdsTerminal": 16,
            "guiRegionRestrictionObserved": True, "loginOrModelAvailabilityProven": False,
            "omittedPayloadsRehashedByValidator": False, "liveActions": 0}

def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--archive-root", type=Path, default=Path(__file__).resolve().parent)
    args = parser.parse_args()
    print(json.dumps(validate(args.archive_root.resolve()), indent=2))

if __name__ == "__main__":
    main()
