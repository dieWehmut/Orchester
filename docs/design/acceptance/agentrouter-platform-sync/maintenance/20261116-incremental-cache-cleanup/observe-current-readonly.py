#!/usr/bin/env python3
"""Read-only current absence and original Linux7337 ELF/process comparison."""
from pathlib import Path
import datetime
import hashlib
import json
import os
import stat
import sys

HERE = Path(__file__).parent
TARGET = Path('/home/han/project/Orchester/.worktrees/main-integration/target/debug/incremental')
ELF = Path('/home/han/project/Orchester/target/debug/orchester')
result = {
    'schemaVersion': 1,
    'observedAtUtc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'observerSourceSHA256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    'authorizedDirectoryAbsentCurrent': not os.path.lexists(TARGET),
    'processSignals': 0,
    'serviceLifecycleActions': 0,
    'windowsActions': 0,
    'gitMutations': 0,
    'privateInventoryContentsRead': False,
    'physicalOrCDriveReleaseClaimed': False,
}
try:
    raw = (HERE / 'cleanup-result-safe.json').read_bytes()
    result['originalCleanupResultSHA256'] = hashlib.sha256(raw).hexdigest()
    if result['originalCleanupResultSHA256'] != 'e49dd3246978bfbc4bb538998957970e0dc15724ac1cf132288fe558e9985ccf':
        raise RuntimeError('original-cleanup-receipt-drift')
    original = json.loads(raw)
    if original['exitCode'] != 0 or original['authorizedDirectoryAbsent'] is not True:
        raise RuntimeError('original-cleanup-not-passed')
    fd = os.open(ELF, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        before = os.fstat(fd)
        if not stat.S_ISREG(before.st_mode):
            raise RuntimeError('current-elf-not-ordinary-file')
        h = hashlib.sha256()
        while True:
            data = os.read(fd, 1024 * 1024)
            if not data:
                break
            h.update(data)
        after = os.fstat(fd)
    finally:
        os.close(fd)
    proc = Path('/proc/427')
    raw_stat = (proc / 'stat').read_text()
    fields = raw_stat[raw_stat.rfind(')') + 2:].split()
    current = {'path': str(ELF), 'device': after.st_dev, 'inode': after.st_ino,
               'size': after.st_size, 'mtimeNs': after.st_mtime_ns,
               'sha256': h.hexdigest(), 'pid': 427, 'startTicks': int(fields[19])}
    result['current7337ElfAndProcess'] = current
    result['currentProcessExePathExact'] = os.readlink(proc / 'exe') == str(ELF)
    result['elfReadIdentityStable'] = (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns) == (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
    result['original7337ElfAndProcessExact'] = current == original['preserved7337Before'] == original['preserved7337After']
    result['privateBeforeInventorySHA256Binding'] = original['beforeInventoryFileSHA256']
    result['originalDeletedInventoryCounts'] = original['beforeInventory']['counts']
    result['originalDeletedRegularLogicalBytes'] = original['beforeInventory']['regularFileLogicalBytes']
    result['originalDeletedUniqueIdentityLogicalBytes'] = original['beforeInventory']['uniqueIdentityLogicalBytes']
    passed = result['authorizedDirectoryAbsentCurrent'] and result['currentProcessExePathExact'] and result['elfReadIdentityStable'] and result['original7337ElfAndProcessExact']
    result['exitCode'] = 0 if passed else 1
    result['status'] = 'current-absence-and-original-7337-preservation-verified' if passed else 'current-comparison-failed'
except Exception as e:
    result['exitCode'] = 1
    result['status'] = 'current-readonly-observer-failed'
    result['failureType'] = type(e).__name__
    result['failureReason'] = str(e)
print(json.dumps(result, ensure_ascii=False, sort_keys=True))
sys.exit(result['exitCode'])
