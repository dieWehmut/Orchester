#!/usr/bin/env python3
"""Read only bounded cache metadata and Git refs; emit no private payloads."""
from pathlib import Path
import datetime
import hashlib
import json
import os
import stat
import subprocess
import sys

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[5]
LINUX_PATHS = {
    'original-repo-rust-incremental': REPO / 'target/debug/incremental',
    'original-linux-npm-download-cache': Path('/home/han/.npm/_cacache'),
    'original-linux-go-build-cache': Path('/home/han/.cache/go-build'),
    'original-linux-cargo-download-cache': Path('/home/han/.cargo/registry/cache'),
    'original-linux-pip-download-cache': Path('/home/han/.cache/pip'),
    'separately-archived-main-integration-incremental': REPO / '.worktrees/main-integration/target/debug/incremental',
}
WINDOWS_SUFFIXES = [
    'AppData/Local/npm-cache/_cacache', 'AppData/Local/pnpm-cache', '.cargo/registry/cache',
    'AppData/Roaming/Code/CachedExtensionVSIXs', 'AppData/Local/npm-cache/_libvips',
    'AppData/Local/npm-cache/_prebuilds', 'AppData/Local/NuGet/v3-cache', 'AppData/Local/pip/Cache',
]
OLD_BRANCHES = {
    'feat/additional-agent-monitors-backend': '84437b8ac8ec31ca538f55df4cc0465b3c0c2a51',
    'feat/additional-agent-monitors-frontend': '4ca1eb9a5d246c456b675e58ff151182a2c5505a',
    'fix/agent-process-source-status': '4b1e21e965c3d456f2d0e2c25ad4762bff37273c',
    'fix/cursor-cli-official-identity': 'ce4b591e74bbf4623a20a9cce7cdca5f20d1f862',
}


def run(*args):
    return subprocess.run(['git', '--no-optional-locks', '-C', str(REPO), *args],
                          capture_output=True, text=True, timeout=30)


def metadata(path):
    try:
        s = path.lstat()
        return {'status': 'present', 'ordinaryDirectory': stat.S_ISDIR(s.st_mode),
                'symlink': stat.S_ISLNK(s.st_mode), 'device': s.st_dev, 'inode': s.st_ino}
    except FileNotFoundError:
        return {'status': 'absent'}
    except OSError as error:
        return {'status': 'unavailable', 'errorType': type(error).__name__, 'errno': error.errno}


result = {'schemaVersion': 1, 'observedAtUtc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
          'observerSourceSha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
          'cacheContentsRead': False, 'applicationProfilesRead': False,
          'gitMutations': 0, 'cacheDeletions': 0, 'processSignals': 0,
          'serviceLifecycleActions': 0, 'windowsNativeActions': 0,
          'physicalOrCDriveReleaseClaimed': False}
try:
    result['linuxCachePaths'] = {label: metadata(path) for label, path in LINUX_PATHS.items()}
    v = os.statvfs(REPO)
    result['linuxFilesystemAvailableBytesSnapshot'] = v.f_bavail * v.f_frsize
    try:
        v = os.statvfs('/mnt/c')
        os.stat('/mnt/c/Users/han')
        result['windowsFilesystem'] = {'status': 'accessible', 'availableBytesSnapshot': v.f_bavail * v.f_frsize}
        result['windowsPriorCachePaths'] = {suffix: metadata(Path('/mnt/c/Users/han') / suffix) for suffix in WINDOWS_SUFFIXES}
    except OSError as error:
        result['windowsFilesystem'] = {'status': 'unavailable', 'errorType': type(error).__name__,
                                      'errno': error.errno, 'cacheAbsenceVerified': False,
                                      'oldPdbInventoryOrDeletionPerformed': False}
    main = run('rev-parse', 'main')
    if main.returncode:
        raise RuntimeError('main-resolution-failed')
    main_sha = main.stdout.strip()
    result['mainRevisionSnapshot'] = main_sha
    result['historicallyDeletedLocalBranches'] = []
    for name, tip in OLD_BRANCHES.items():
        ref = run('show-ref', '--verify', '--quiet', 'refs/heads/' + name)
        cherry = run('cherry', main_sha, tip)
        ancestry = run('merge-base', '--is-ancestor', tip, main_sha)
        if cherry.returncode or ancestry.returncode not in (0, 1):
            raise RuntimeError('historical-branch-comparison-failed')
        rows = cherry.stdout.splitlines()
        result['historicallyDeletedLocalBranches'].append({
            'branch': name, 'historicalTip': tip, 'localRefAbsent': ref.returncode == 1,
            'ancestorOfMain': ancestry.returncode == 0,
            'patchEquivalentCommits': sum(r.startswith('-') for r in rows),
            'nonEquivalentCommits': sum(r.startswith('+') for r in rows),
            'gitCherryExitCode': cherry.returncode,
        })
    result['historicallyRemovedWorktreePaths'] = {name: metadata(REPO / '.worktrees' / name) for name in
        ('additional-agent-monitors-backend', 'additional-agent-monitors-frontend', 'cursor-cli-identity')}
    worktrees = run('worktree', 'list', '--porcelain')
    if worktrees.returncode:
        raise RuntimeError('worktree-list-failed')
    result['registeredWorktreeCountSnapshot'] = sum(r.startswith('worktree ') for r in worktrees.stdout.splitlines())
    monitor = subprocess.run(['git', '--no-optional-locks', '-C', str(REPO / '.worktrees/monitor-validation'),
                              'status', '--porcelain=v1', '--untracked-files=no'],
                             capture_output=True, text=True, timeout=30)
    result['retainedMonitorValidation'] = {'present': (REPO / '.worktrees/monitor-validation').is_dir(),
        'statusExitCode': monitor.returncode, 'trackedChangeCount': len(monitor.stdout.splitlines()),
        'privateFilenamesPublished': False, 'cleanupPerformed': False}
    remote = run('ls-remote', '--heads', 'origin')
    result['remoteHeadsObservationExitCode'] = remote.returncode
    if remote.returncode == 0:
        refs = {line.split()[1]: line.split()[0] for line in remote.stdout.splitlines() if len(line.split()) == 2}
        result['remoteHeadsSnapshot'] = refs
        result['historicallyDeletedRemoteRefsAbsent'] = {
            name: 'refs/heads/' + name not in refs for name in ('feat/agent-process-presence', 'feat/web-workspace-shell')}
        retained = refs.get('refs/heads/feat/rail-reference-alignment')
        if retained:
            cherry = run('cherry', main_sha, retained)
            ancestry = run('merge-base', '--is-ancestor', retained, main_sha)
            result['retainedRemoteBranch'] = {'branch': 'feat/rail-reference-alignment', 'tip': retained,
                'ancestorOfMain': ancestry.returncode == 0, 'gitCherryExitCode': cherry.returncode,
                'nonEquivalentCommits': sum(r.startswith('+') for r in cherry.stdout.splitlines()),
                'cleanupPerformed': False}
    result['mainRefStableDuringObservation'] = run('rev-parse', 'main').stdout.strip() == main_sha
    result['status'] = 'bounded-readonly-observation-complete'
    result['exitCode'] = 0
except Exception as error:
    result['status'] = 'bounded-readonly-observation-failed'
    result['failureType'] = type(error).__name__
    result['failureReason'] = str(error)
    result['exitCode'] = 1
print(json.dumps(result, ensure_ascii=False, sort_keys=True))
sys.exit(result['exitCode'])
