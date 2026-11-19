#!/usr/bin/env python3
"""Delete only the authorized, identity-bound Linux incremental cache."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import sys

OUT = Path(__file__).parent
TARGET = Path('/home/han/project/Orchester/.worktrees/main-integration/target/debug/incremental')
EXPECTED = (2096, 2901617)
ELF = Path('/home/han/project/Orchester/target/debug/orchester')
WATCHED = {'cargo', 'rustc', 'rust-analyzer', 'sccache'}
started = datetime.datetime.now(datetime.timezone.utc).isoformat()

def canonical(obj):
    return json.dumps(obj, ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode()

def save(name, obj):
    data = canonical(obj) + b'\n'
    fd = os.open(OUT / name, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    with os.fdopen(fd, 'wb') as f:
        f.write(data)
        f.flush()
        os.fsync(f.fileno())
    return hashlib.sha256(data).hexdigest()

def identity(s):
    return (s.st_dev, s.st_ino)

def sha(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b''):
            h.update(chunk)
    return h.hexdigest()

def require_bound_path():
    for p in reversed([TARGET, *TARGET.parents]):
        s = p.lstat()
        if not stat.S_ISDIR(s.st_mode):
            raise RuntimeError('target-ancestor-not-ordinary-directory')
    s = TARGET.lstat()
    if identity(s) != EXPECTED or s.st_uid != os.getuid():
        raise RuntimeError('target-identity-or-owner-drift')
    return {'device': s.st_dev, 'inode': s.st_ino, 'ownerUid': s.st_uid}

def inventory():
    require_bound_path()
    rows = []
    stack = [TARGET]
    counts = {'files': 0, 'directories': 0, 'symlinks': 0, 'hardLinkedFiles': 0}
    logical = 0
    unique = {}
    while stack:
        p = stack.pop()
        s = p.lstat()
        rel = p.relative_to(TARGET).as_posix()
        if stat.S_ISDIR(s.st_mode):
            kind = 'directory'
            counts['directories'] += 1
            stack.extend(sorted(p.iterdir(), reverse=True))
        elif stat.S_ISREG(s.st_mode):
            kind = 'file'
            counts['files'] += 1
            logical += s.st_size
            unique[identity(s)] = s.st_size
            if s.st_nlink > 1:
                counts['hardLinkedFiles'] += 1
        elif stat.S_ISLNK(s.st_mode):
            kind = 'symlink'
            counts['symlinks'] += 1
        else:
            raise RuntimeError('unsupported-object-in-authorized-cache')
        if s.st_dev != EXPECTED[0]:
            raise RuntimeError('nested-filesystem-in-authorized-cache')
        rows.append({'path': rel, 'kind': kind, 'device': s.st_dev,
                     'inode': s.st_ino, 'size': s.st_size, 'links': s.st_nlink,
                     'mtimeNs': s.st_mtime_ns})
    rows.sort(key=lambda r: r['path'])
    return {'rows': rows, 'counts': counts, 'regularFileLogicalBytes': logical,
            'uniqueRegularFileIdentities': len(unique),
            'uniqueIdentityLogicalBytes': sum(unique.values()),
            'inventoryMetadataSHA256': hashlib.sha256(canonical(rows)).hexdigest(),
            'inventoryHashIncludesFileContents': False}

def scope_path(s):
    s = s.removesuffix(' (deleted)')
    return s == str(TARGET) or s.startswith(str(TARGET) + '/')

def usage():
    result = {'observedProcesses': 0, 'readableCompilerProcesses': [],
              'cachePathUses': [], 'exeUnreadableOrExited': 0,
              'fdDirectoryUnreadableOrExited': 0, 'fdReadlinkUnreadableOrExited': 0,
              'mapsUnreadableOrExited': 0, 'cwdUnreadableOrExited': 0,
              'fdEntriesInspected': 0, 'mapsFilesInspected': 0}
    for p in Path('/proc').iterdir():
        if not p.name.isdigit():
            continue
        pid = int(p.name)
        result['observedProcesses'] += 1
        try:
            exe = os.readlink(p / 'exe')
            comm = (p / 'comm').read_text().strip()
            if Path(exe).name in WATCHED or comm in WATCHED:
                result['readableCompilerProcesses'].append({'pid': pid, 'basename': Path(exe).name})
            if scope_path(exe):
                result['cachePathUses'].append({'pid': pid, 'kind': 'exe'})
        except (OSError, UnicodeError):
            result['exeUnreadableOrExited'] += 1
        try:
            if scope_path(os.readlink(p / 'cwd')):
                result['cachePathUses'].append({'pid': pid, 'kind': 'cwd'})
        except OSError:
            result['cwdUnreadableOrExited'] += 1
        try:
            fds = list((p / 'fd').iterdir())
        except OSError:
            result['fdDirectoryUnreadableOrExited'] += 1
            fds = []
        for fd in fds:
            try:
                value = os.readlink(fd)
                result['fdEntriesInspected'] += 1
                if scope_path(value):
                    result['cachePathUses'].append({'pid': pid, 'kind': 'fd', 'fd': int(fd.name)})
            except OSError:
                result['fdReadlinkUnreadableOrExited'] += 1
        try:
            content = (p / 'maps').read_text()
            result['mapsFilesInspected'] += 1
            for line in content.splitlines():
                parts = line.split(None, 5)
                if len(parts) == 6 and scope_path(parts[5]):
                    result['cachePathUses'].append({'pid': pid, 'kind': 'mmap'})
        except (OSError, UnicodeError):
            result['mapsUnreadableOrExited'] += 1
    return result

def elf_state():
    s = ELF.lstat()
    if not stat.S_ISREG(s.st_mode):
        raise RuntimeError('preserved-elf-not-ordinary-file')
    proc = Path('/proc/427')
    raw = (proc / 'stat').read_text()
    fields = raw[raw.rfind(')') + 2:].split()
    exe = os.readlink(proc / 'exe')
    if exe != str(ELF):
        raise RuntimeError('current-7337-executable-binding-changed')
    return {'path': str(ELF), 'device': s.st_dev, 'inode': s.st_ino,
            'size': s.st_size, 'mtimeNs': s.st_mtime_ns, 'sha256': sha(ELF),
            'pid': 427, 'startTicks': int(fields[19])}

def siblings():
    return {p.name: {'device': p.lstat().st_dev, 'inode': p.lstat().st_ino,
                    'mode': p.lstat().st_mode, 'size': p.lstat().st_size,
                    'mtimeNs': p.lstat().st_mtime_ns}
            for p in TARGET.parent.iterdir() if p.name != TARGET.name}

deletion_started = False
result = {'schemaVersion': 1, 'startedAtUtc': started, 'authorizedPath': str(TARGET),
          'scriptSHA256': sha(Path(__file__)), 'deletionStarted': False,
          'processSignals': 0, 'gitMutations': 0, 'windowsActions': 0,
          'physicalSpaceReleaseMeasured': False, 'cDriveReleaseClaimed': False}
try:
    result['targetIdentityBefore'] = require_bound_path()
    preserved_elf = elf_state()
    preserved_siblings = siblings()
    first = inventory()
    preflight = usage()
    result['processUsageBefore'] = preflight
    save('process-usage-before-safe.json', preflight)
    if preflight['readableCompilerProcesses'] or preflight['cachePathUses']:
        raise RuntimeError('current-compiler-or-cache-use-observed')
    second = inventory()
    if first != second:
        raise RuntimeError('cache-inventory-drift')
    result['beforeInventoryFileSHA256'] = save('before-inventory-private.json', first)
    result['beforeInventory'] = {k: v for k, v in first.items() if k != 'rows'}
    result['inventoryStableAcrossTwoReads'] = True
    result['preserved7337Before'] = preserved_elf
    if not shutil.rmtree.avoids_symlink_attacks:
        raise RuntimeError('fd-anchored-rmtree-unavailable')
    parentfd = os.open(TARGET.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        s = os.stat(TARGET.name, dir_fd=parentfd, follow_symlinks=False)
        if identity(s) != EXPECTED or not stat.S_ISDIR(s.st_mode):
            raise RuntimeError('delete-boundary-identity-drift')
        deletion_started = True
        result['deletionStarted'] = True
        shutil.rmtree(TARGET.name, dir_fd=parentfd)
        os.fsync(parentfd)
    finally:
        os.close(parentfd)
    result['authorizedDirectoryAbsent'] = not os.path.lexists(TARGET)
    result['preserved7337After'] = elf_state()
    result['preserved7337ByteAndProcessIdentityExact'] = preserved_elf == result['preserved7337After']
    result['otherDebugSiblingsMetadataExact'] = preserved_siblings == siblings()
    if not result['authorizedDirectoryAbsent'] or not result['preserved7337ByteAndProcessIdentityExact'] or not result['otherDebugSiblingsMetadataExact']:
        raise RuntimeError('cleanup-postcondition-failed')
    result['status'] = 'authorized-exact-linux-incremental-cache-removed-and-preserved-elf-verified'
    result['exitCode'] = 0
except Exception as e:
    result['status'] = 'failed-after-deletion-started' if deletion_started else 'retained-before-deletion'
    result['failureType'] = type(e).__name__
    result['failureReason'] = str(e)
    result['authorizedDirectoryAbsent'] = not os.path.lexists(TARGET)
    result['exitCode'] = 1
finally:
    result['finishedAtUtc'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    result['resultSHA256'] = save('cleanup-result-safe.json', result)
    print(json.dumps(result, ensure_ascii=False), flush=True)
sys.exit(result['exitCode'])
