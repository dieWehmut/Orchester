#!/usr/bin/env python3
"""Validate saved maintenance evidence without reading private state or mutating Git."""
from pathlib import Path
import hashlib
import json
import sys

HERE = Path(__file__).resolve().parent
checks = []


def check(condition, label):
    checks.append({'check': label, 'passed': bool(condition)})


def read(name):
    return json.loads((HERE / name).read_text())


try:
    manifest = read('artifact-manifest.json')
    expected = {r['path'] for r in manifest['files']}
    actual = {p.name for p in HERE.iterdir() if p.is_file() and p.name not in
              ('artifact-manifest.json', 'review-safe.json')}
    check(expected == actual, 'manifest-covers-all-archive-files-except-itself-and-review')
    for row in manifest['files']:
        raw = (HERE / row['path']).read_bytes()
        check(len(raw) == row['bytes'] and hashlib.sha256(raw).hexdigest() == row['sha256'],
              'artifact-sha256:' + row['path'])
    cleanup = read('npm-cleanup-result-safe.json')
    check(cleanup['sourceSha256'] == hashlib.sha256((HERE / 'cleanup-owned-npm-cache.py').read_bytes()).hexdigest(),
          'cleanup-source-bound-to-emitted-result')
    check(cleanup['exitCode'] == 0 and cleanup['cacheDeletionPerformed'] is True and
          cleanup['authorizedCacheAbsentAfter'] is True, 'actual-additional-deletion-complete')
    check(cleanup['beforeInventory'] == cleanup['secondInventory'], 'two-identical-cache-metadata-inventories')
    inv = cleanup['beforeInventory']
    check(inv['regularFiles'] == 6 and inv['regularLogicalBytes'] == 191672408 and
          inv['regularAllocatedBytes'] == 191688704 and inv['symlinks'] == 0 and
          inv['multipleHardlinkedFiles'] == 0, 'additional-cache-measurements-and-link-scope')
    check(cleanup['consumerObservation']['packageManagerProcesses'] == 0 and
          cleanup['consumerObservation']['cachePathReferences'] == 0, 'no-observed-cache-consumers')
    check(cleanup['observedFilesystemAvailableChangeBytes'] ==
          cleanup['filesystemAvailableAfterBytes'] - cleanup['filesystemAvailableBeforeBytes'],
          'new-linux-available-space-delta-arithmetic')
    check(not cleanup['physicalOrCDriveReleaseClaimed'] and not cleanup['cacheContentsRead'] and
          cleanup['gitMutations'] == cleanup['windowsActions'] == cleanup['processSignals'] == 0,
          'additional-cleanup-boundaries')
    historical = read('historical-cleanup-derived-safe.json')
    for label in ('linuxHistoricalCleanup', 'windowsHistoricalCleanup'):
        operation = historical[label]
        check(all(r['removed'] is True for r in operation['directories']), label + ':recorded-removed')
        check(operation['observedFilesystemAvailableIncreaseBytes'] ==
              operation['filesystemAvailableAfterBytes'] - operation['filesystemAvailableBeforeBytes'],
              label + ':interval-delta-arithmetic')
    check(len(historical['linuxHistoricalCleanup']['directories']) == 5 and
          len(historical['windowsHistoricalCleanup']['directories']) == 8,
          'historical-directory-counts')
    win = historical['windowsHistoricalCleanup']
    pre = historical['windowsHistoricalNativePreflight']
    check(win['preflightResultSha256BoundByCleanup'] == pre['sourceResultSha256'] and
          win['preflightTerminalSha256BoundByCleanup'] == pre['terminalSha256'] and
          pre['nativeExitCode'] == 0 and pre['allExclusiveReadOpensSucceeded'] is True,
          'historical-windows-cleanup-bound-to-native-preflight')
    excluded = historical['alreadyArchivedCleanupExcludedFromThisAccounting']
    check(excluded['regularLogicalBytes'] == 11441931494 and
          excluded['uniqueIdentityLogicalBytes'] == 10981446886 and
          excluded['additionalDeletedBytesInThisHistoricalSummary'] == 0,
          'existing-incremental-archive-not-double-counted')
    sources = read('original-source-bindings-safe.json')
    check(len(sources['sourceFiles']) == 7 and all(not r['originalBytesPublished'] and
          len(r['sha256']) == 64 for r in sources['sourceFiles']), 'seven-external-original-sources-hash-bound')
    current = read('current-readonly-safe.json')
    check(current['observerSourceSha256'] == hashlib.sha256((HERE / 'observe-current-readonly.py').read_bytes()).hexdigest(),
          'current-observer-source-bound')
    check(current['exitCode'] == 0 and current['status'] == 'bounded-readonly-observation-complete',
          'current-readonly-observer-completed')
    check(all(r['status'] == 'absent' for r in current['linuxCachePaths'].values()),
          'six-bounded-linux-paths-absent-at-observation')
    check(len(current['historicallyDeletedLocalBranches']) == 4 and all(
          r['localRefAbsent'] and r['nonEquivalentCommits'] == 0 and r['gitCherryExitCode'] == 0
          for r in current['historicallyDeletedLocalBranches']), 'historical-local-refs-absent-and-patch-equivalent')
    check(all(r['status'] == 'absent' for r in current['historicallyRemovedWorktreePaths'].values()),
          'historical-three-worktree-paths-absent')
    check(current['remoteHeadsObservationExitCode'] == 0 and
          all(current['historicallyDeletedRemoteRefsAbsent'].values()), 'historical-two-remote-refs-absent')
    check(current['retainedRemoteBranch']['nonEquivalentCommits'] == 18 and
          not current['retainedRemoteBranch']['cleanupPerformed'], 'unmerged-remote-branch-retained')
    check(current['retainedMonitorValidation']['present'] and
          current['retainedMonitorValidation']['trackedChangeCount'] == 20 and
          not current['retainedMonitorValidation']['cleanupPerformed'], 'dirty-monitor-validation-retained')
    check(current['windowsFilesystem']['status'] == 'unavailable' and
          current['windowsFilesystem']['cacheAbsenceVerified'] is False and
          'windowsPriorCachePaths' not in current, 'windows-io-failure-does-not-become-absence')
    initial = read('initial-current-readonly-failed-safe.json')
    check(initial['exitCode'] == 1 and initial['failureType'] == 'TimeoutExpired' and
          initial['observerSourceSha256'] == hashlib.sha256((HERE / 'initial-observer-source.py').read_bytes()).hexdigest(),
          'initial-remote-timeout-and-original-source-preserved')
except Exception as error:
    checks.append({'check': 'validation-exception', 'passed': False,
                   'failureType': type(error).__name__, 'failureReason': str(error)})

passed = all(r['passed'] for r in checks)
print(json.dumps({'schemaVersion': 1, 'status': 'passed' if passed else 'failed',
                  'checks': checks, 'checkCount': len(checks), 'privateSourceReads': 0,
                  'gitMutations': 0, 'cacheDeletions': 0, 'windowsActions': 0,
                  'exitCode': 0 if passed else 1}, sort_keys=True))
sys.exit(0 if passed else 1)
