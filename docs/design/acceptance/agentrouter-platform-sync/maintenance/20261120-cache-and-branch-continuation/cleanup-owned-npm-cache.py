#!/usr/bin/env python3
"""Bounded cleanup of one verified regenerated npm download cache.

The default mode is read-only. --execute refuses identity or metadata drift.
No installed package, npx environment, application profile or Windows path is read.
"""
from pathlib import Path
import datetime
import hashlib
import json
import os
import re
import shutil
import stat
import sys

TARGET = Path('/home/han/.npm/_cacache')
EXPECTED_IDENTITY = (2096, 34873, 1000)
EXPECTED_METADATA_SHA256 = 'f1a9146d3d2aa0da00271512cc9b492cd0c72dbf2aff88cc8c9063f1340b73fe'
MANAGERS = re.compile(r'^(npm|npx|pnpm|yarn|corepack|bun|cargo|rustc|sccache|rust-analyzer|pip|pip3|go)(?:\.js|\.cjs)?$')


def inventory():
    root = TARGET.lstat()
    if (root.st_dev, root.st_ino, root.st_uid) != EXPECTED_IDENTITY:
        raise RuntimeError('cache-root-identity-drift')
    for p in (TARGET, *TARGET.parents):
        s = p.lstat()
        if not stat.S_ISDIR(s.st_mode) or stat.S_ISLNK(s.st_mode):
            raise RuntimeError('nonordinary-cache-ancestor')
    rows = []
    for directory, directories, files in os.walk(TARGET, followlinks=False):
        for name in sorted(directories + files):
            p = Path(directory) / name
            s = p.lstat()
            if s.st_uid != os.getuid() or not (stat.S_ISREG(s.st_mode) or stat.S_ISDIR(s.st_mode)):
                raise RuntimeError('unexpected-cache-entry')
            if stat.S_ISREG(s.st_mode) and s.st_nlink != 1:
                raise RuntimeError('externally-linked-cache-entry')
            rows.append([str(p.relative_to(TARGET)), s.st_dev, s.st_ino, s.st_uid,
                         s.st_mode, s.st_size, s.st_mtime_ns, s.st_nlink, s.st_blocks])
    digest = hashlib.sha256(json.dumps(sorted(rows), separators=(',', ':')).encode()).hexdigest()
    if digest != EXPECTED_METADATA_SHA256:
        raise RuntimeError('cache-metadata-drift')
    regular = [r for r in rows if stat.S_ISREG(r[4])]
    return {'rootIdentity': {'device': root.st_dev, 'inode': root.st_ino, 'ownerUid': root.st_uid},
            'regularFiles': len(regular), 'directories': len(rows) - len(regular),
            'regularLogicalBytes': sum(r[5] for r in regular),
            'regularAllocatedBytes': sum(r[8] * 512 for r in regular),
            'canonicalMetadataSha256': digest, 'symlinks': 0, 'multipleHardlinkedFiles': 0}


def consumers():
    result = {'ownedProcessesScanned': 0, 'packageManagerProcesses': 0,
              'cachePathReferences': 0, 'unavailableReads': {}}

    def unavailable(kind):
        result['unavailableReads'][kind] = result['unavailableReads'].get(kind, 0) + 1

    for proc in Path('/proc').iterdir():
        if not proc.name.isdigit():
            continue
        try:
            if proc.stat().st_uid != os.getuid():
                continue
        except OSError:
            continue
        result['ownedProcessesScanned'] += 1
        try:
            comm = (proc / 'comm').read_text().strip()
            # Only classifications and counts leave this function; no command line is archived.
            argv = (proc / 'cmdline').read_bytes().split(b'\0')
            names = [os.path.basename(a.decode(errors='replace')) for a in argv[:3] if a]
            if MANAGERS.fullmatch(comm) or any(MANAGERS.fullmatch(n) for n in names):
                result['packageManagerProcesses'] += 1
        except OSError:
            unavailable('processClassification')
        for kind in ('cwd', 'exe'):
            try:
                value = os.readlink(proc / kind)
                if value == str(TARGET) or value.startswith(str(TARGET) + '/'):
                    result['cachePathReferences'] += 1
            except OSError:
                unavailable(kind)
        try:
            descriptors = list((proc / 'fd').iterdir())
        except OSError:
            unavailable('fdDirectory')
            descriptors = []
        for fd in descriptors:
            try:
                value = os.readlink(fd)
                if value == str(TARGET) or value.startswith(str(TARGET) + '/'):
                    result['cachePathReferences'] += 1
            except OSError:
                unavailable('fdReadlink')
        try:
            if str(TARGET) in (proc / 'maps').read_text():
                result['cachePathReferences'] += 1
        except OSError:
            unavailable('maps')
    return result


def available():
    value = os.statvfs(TARGET.parent)
    return value.f_bavail * value.f_frsize


result = {'schemaVersion': 1, 'observedAtUtc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
          'scope': 'one-regenerated-linux-npm-download-cache', 'executeRequested': sys.argv[1:] == ['--execute'],
          'sourceSha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
          'cacheContentsRead': False, 'cacheDeletionPerformed': False,
          'gitMutations': 0, 'windowsActions': 0, 'processSignals': 0,
          'installedArtifactsChanged': False, 'physicalOrCDriveReleaseClaimed': False}
try:
    if sys.argv[1:] not in ([], ['--execute']):
        raise RuntimeError('unsupported-argument')
    result['beforeInventory'] = inventory()
    result['consumerObservation'] = consumers()
    if result['consumerObservation']['packageManagerProcesses'] or result['consumerObservation']['cachePathReferences']:
        raise RuntimeError('observed-cache-consumer')
    result['secondInventory'] = inventory()
    if result['beforeInventory'] != result['secondInventory']:
        raise RuntimeError('inventory-changed')
    result['filesystemAvailableBeforeBytes'] = available()
    if result['executeRequested']:
        if not shutil.rmtree.avoids_symlink_attacks:
            raise RuntimeError('fd-based-rmtree-unavailable')
        shutil.rmtree(TARGET)
        result['cacheDeletionPerformed'] = True
        result['authorizedCacheAbsentAfter'] = not os.path.lexists(TARGET)
        if not result['authorizedCacheAbsentAfter']:
            raise RuntimeError('cache-remains')
        result['filesystemAvailableAfterBytes'] = available()
        result['observedFilesystemAvailableChangeBytes'] = result['filesystemAvailableAfterBytes'] - result['filesystemAvailableBeforeBytes']
        result['filesystemChangeBoundary'] = 'Linux filesystem available-space observation during this interval; concurrent activity can affect it; no C-drive or VHDX physical-reclaim measurement.'
    result['status'] = 'bounded-cache-deletion-complete' if result['cacheDeletionPerformed'] else 'read-only-preflight-passed'
    result['exitCode'] = 0
except Exception as error:
    result['status'] = 'refused-or-failed'
    result['failureType'] = type(error).__name__
    result['failureReason'] = str(error)
    result['exitCode'] = 1
print(json.dumps(result, ensure_ascii=False, sort_keys=True))
sys.exit(result['exitCode'])
