#!/usr/bin/env python3
"""Exercise semantic and signature failures in disposable archive copies only.

Each case refreshes ordinary file/source digests after its mutation. Rejection
therefore has to come from the evidence relationships or retained signature,
rather than an incidental stale inventory hash. No private host input is used.
"""
from pathlib import Path
import hashlib
import json
import runpy
import shutil
import tempfile


def read(path):
    return json.loads(path.read_bytes())


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')


def digest(path):
    data = path.read_bytes()
    return hashlib.sha256(data).hexdigest(), len(data)


def edit(root, relative, fields, value):
    path = root / relative
    document = read(path)
    target = document
    for field in fields[:-1]:
        target = target[field]
    target[fields[-1]] = value
    write(path, document)


def mutate_gui(root, field, value):
    for name in ('native-gui-result-original-safe.json', 'native-gui-result-safe.json'):
        edit(root, 'linux/evidence/20261119/' + name, field, value)


def refresh_noncryptographic_links(root):
    linux = root / 'linux'
    original_path = linux / 'evidence/20261119/native-gui-result-original-safe.json'
    gui_path = linux / 'evidence/20261119/native-gui-result-safe.json'
    original, gui = read(original_path), read(gui_path)
    gui['metadataCorrection']['originalReceiptSHA256'] = digest(original_path)[0]
    write(gui_path, gui)
    audit_path = linux / 'evidence/20261119/gui-metadata-change-audit-derived-safe.json'
    audit = read(audit_path)
    audit['originalReceiptSha256'] = digest(original_path)[0]
    audit['correctedSourceReceiptSha256'] = digest(gui_path)[0]
    audit['changedCommonFields'] = {k: {'originalValue': original[k], 'correctedValue': gui[k]}
                                    for k in original.keys() & gui.keys() if original[k] != gui[k]}
    audit['removedOriginalFields'] = {k: original[k] for k in original.keys() - gui.keys()}
    audit['addedMetadataFields'] = {k: gui[k] for k in gui.keys() - original.keys()}
    write(audit_path, audit)
    edit(root, 'linux/evidence/20261119/final-native-linux-verification-safe.json',
         ['nativeGuiReceiptSHA256'], digest(gui_path)[0])
    mapping_path = linux / 'source-map.json'
    mapping = read(mapping_path)
    for row in mapping['sources']:
        row['archiveSha256'], row['archiveBytes'] = digest(linux / row['archivePath'])
        if row['representation'] == 'original-bytes':
            row['sourceSha256'], row['sourceBytes'] = row['archiveSha256'], row['archiveBytes']
        elif row['id'] == 'GUI-receipt-complete-change-audit':
            row['sourceSha256'], row['sourceBytes'] = digest(gui_path)
    write(mapping_path, mapping)
    windows = root / 'windows'
    mapping_path = windows / 'source-map.json'
    mapping = read(mapping_path)
    sources = {row['id']: row for row in mapping['sources']}
    for row in mapping['publishedArtifacts']:
        row['sha256'], row['bytes'] = digest(windows / row['path'])
        if row['representation'] == 'original-bytes':
            source = sources[row['sourceIds'][0]]
            source['sha256'], source['bytes'] = row['sha256'], row['bytes']
    write(mapping_path, mapping)
    for directory, name in ((linux, 'artifact-index.json'), (root, 'artifact-manifest.json')):
        index = read(directory / name)
        for row in index['files']:
            row['sha256'], row['bytes'] = digest(directory / row['path'])
        if name == 'artifact-manifest.json':
            index['bytesExcludingManifest'] = sum(row['bytes'] for row in index['files'])
        write(directory / name, index)


def cases():
    fresh = 'windows/original/current-registered-installed-proof-safe.json'
    derived = 'windows/derived/'
    yield 'summary-install-exit-contradiction', lambda r: edit(r, 'summary-safe.json', ['windows', 'nativeInstallerExitCode'], 1), 'Windows summary contradicts'
    yield 'summary-gui-exit-contradiction', lambda r: edit(r, 'summary-safe.json', ['linux', 'nativeGuiExitCode'], 1), 'Linux summary contradicts'
    yield 'current-Windows-native-nonzero-exit', lambda r: edit(r, fresh, ['actualNativeExitCode'], 1), "fresh['actualNativeExitCode']==0"
    yield 'current-Windows-Store-signature-kind', lambda r: edit(r, fresh, ['registeredPackageSignatureKind'], 'Developer'), 'current registration identity mismatch'
    yield 'current-Windows-complete-package-identity', lambda r: edit(r, fresh, ['registeredPackageFullName'], 'UnrelatedPackage_1.0_x64__other'), 'current registration identity mismatch'
    yield 'current-Windows-EXE-version', lambda r: edit(r, fresh, ['installedExeFileVersion'], '0.0.0'), 'current installed-file metadata mismatch'
    yield 'catalog-source-digest-binding', lambda r: edit(r, derived + 'official-store-catalog-20261116-safe.json', ['sourceBodySha256'], '0' * 64), "catalog['sourceBodySha256']"
    yield 'show-source-digest-binding', lambda r: edit(r, derived + 'official-store-show-public-metadata-safe.json', ['rawStdoutSha256'], '0' * 64), "publicshow['rawStdoutSha256']"
    yield 'Windows-visible-window-overclaim', lambda r: edit(r, derived + 'windows-optional-gui-status-safe.json', ['observerVisibleWindowCount'], 1), "gui['observerVisibleWindowCount']"
    yield 'truncated-candidate-promoted-to-setup', lambda r: edit(r, derived + 'prior-failure-and-truncated-candidate-safe.json', ['truncatedCandidateAcceptedAsCurrentSetup'], True), "prior['truncatedCandidateAcceptedAsCurrentSetup']"
    yield 'Linux-failed-download-promoted-to-install', lambda r: edit(r, 'linux/evidence/20261116/linux-deb-download-safe.json', ['installed'], True), 'historical incomplete download'
    yield 'Linux-correction-ASAR-version', lambda r: edit(r, 'linux/evidence/20261119/post-gui-native-package-reconciliation-safe.json', ['asarPackageVersion'], '45.2.1'), 'post-GUI package/runtime correction evidence'
    yield 'Linux-quit-target-not-owned-window', lambda r: mutate_gui(r, ['normalProductQuitActions', 0, 'window'], 999999), 'normal quit must target'
    yield 'Linux-image-window-geometry-mismatch', lambda r: mutate_gui(r, ['image', 'width'], 1), 'owned-window/image geometry mismatch'
    duplicate = {'pid': 70542, 'ppid': 70533, 'startTicks': '24473460', 'nativeElf': True,
                 'originalPidfdHeld': True, 'terminalByOriginalPidfd': True}
    yield 'Linux-duplicate-held-process-identity', lambda r: mutate_gui(r, ['heldObjects', 1], duplicate), 'GUI distinct process/window identities'

    def tamper_signature(root):
        path = root / 'linux/repository/InRelease'
        path.write_bytes(path.read_bytes().replace(b'Suite: stable', b'Suite: altered', 1))
    yield 'retained-InRelease-signature-tamper', tamper_signature, 'retained InRelease signature invalid'

    def tamper_packages(root):
        path = root / 'linux/repository/Packages-amd64'
        path.write_bytes(path.read_bytes() + b'\n')
    yield 'Packages-not-bound-to-signed-release', tamper_packages, 'Packages not bound to signed InRelease'


def run(root):
    validator = runpy.run_path(str(root / 'validate.py'))['validate']
    validator(root)
    results = []
    for label, mutate, expected in cases():
        with tempfile.TemporaryDirectory(prefix='claude-archive-negative-') as directory:
            copy = Path(directory) / 'archive'
            shutil.copytree(root, copy)
            mutate(copy)
            refresh_noncryptographic_links(copy)
            try:
                validator(copy)
            except ValueError as error:
                if expected not in str(error):
                    raise ValueError(label + ' failed for an unexpected reason: ' + str(error)) from error
                results.append({'case': label, 'status': 'rejected', 'reason': str(error)})
            else:
                raise ValueError('invalid archive was accepted: ' + label)
    return {'status': 'pass', 'baseline': 'pass', 'negativeCases': results,
            'ordinaryDigestMetadataRefreshedBeforeEveryCase': True,
            'privateInputs': 0, 'networkAccess': False, 'nativeInstallationOrGuiActions': 0}


if __name__ == '__main__':
    print(json.dumps(run(Path(__file__).resolve().parent), indent=2))
