#!/usr/bin/env python3
"""Read-only checks for the final cross-epoch requirement reconciliation."""
import hashlib
import json
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
BASE = HERE.parents[2]
REPO = BASE.parents[3]


def read(path):
    return json.loads(path.read_text())


def require(value, message):
    if not value:
        raise ValueError(message)


audit = read(BASE / 'sources/requirement-delivery-audit.json')
expected = {
    'catalog_scope', 'official_icons', 'separate_workbuddy',
    'tidy_code_and_folders', 'sync_frontend_surfaces', 'desktop_reinstall',
    'windows_agents', 'wsl_agents', 'android_studio', 'linux_service',
    'parallelism', 'cleanup', 'git_steps', 'additional_agents_20261112',
}
rows = audit['requirements']
require(len(rows) == 14 and {r['id'] for r in rows} == expected, 'requirement inventory')
links = 0
for row in rows:
    require(row['remaining'] == [], 'open requirement: ' + row['id'])
    require(row['evidence'], 'missing evidence: ' + row['id'])
    for name in row['evidence']:
        path = Path(name)
        require(not path.is_absolute() and '..' not in path.parts, 'unsafe evidence path')
        require((BASE / path).is_file(), 'missing evidence: ' + name)
        links += 1

boundaries = read(HERE / 'original-requirement-boundaries-safe.json')
require(len(boundaries['foundationalRequirements']) == 6, 'foundational inventory')
for row in boundaries['foundationalRequirements']:
    require(row['remaining'] == [], 'open foundational requirement')
    for entry in row['evidence']:
        actual = hashlib.sha256((REPO / entry['path']).read_bytes()).hexdigest()
        require(actual == entry['sha256'], 'foundational document changed')
require(not boundaries['rawConversationOrPrivatePathsPublished'], 'private conversation claim')
require(all(not r['originalCompletionGate'] for r in boundaries['boundaryDecisions']), 'boundary drift')

summary = read(BASE / 'summary.json')
require(summary['fullObjectiveComplete'] and summary['pendingCurrentEpochScopes'] == [], 'completion summary')
require(summary['currentInternationalWorkBuddyLinuxInstalled'], 'international installation')
require(summary['currentWindowsManagedRemote7338CurrentRevalidationVerified'], 'current Windows runtime')
runtime = read(BASE / 'native/windows/20261120-recovered-managed-runtime/scope-safe.json')
require(runtime['observedSeconds'] >= 60 and runtime['samples'] == 88, 'bounded runtime observation')
require(runtime['resourceCount'] == 17 and runtime['originalRows'] == 138, 'resource or retention scope')
require(runtime['postObserverProbeVerified'] and runtime['anonymousInfoStatus'] == 401, 'post-observer scope')

pages = read(HERE / 'pages-mobile-open-safe.json')
opened, closed = pages['opened'], pages['escaped']
require(opened['width'] == 390 and opened['expanded'] == 'true', 'actual mobile opening')
require(opened['menuPresent'] and opened['menuVisible'] and len(opened['links']) == 3, 'visible mobile links')
require(opened['fontLoaded'] and not opened['horizontalOverflow'], 'mobile font/overflow')
require(closed['expanded'] == 'false' and closed['menuAbsent'] and closed['focusOnMenuTrigger'], 'Escape and focus')
require(not closed['horizontalOverflow'] and pages['isolatedPageClosed'], 'browser cleanup')

reviews = read(HERE / 'subagent-reviews-safe.json')
require(len(reviews['reviews']) == len({r['agent'] for r in reviews['reviews']}) == 16, 'distinct delegated reviews')
require(reviews['runtimeCapacityIncludingRoot'] == 17 and not reviews['reviewersModifiedRepository'], 'delegation scope')
delivery = read(HERE / 'git-delivery-before-completion-safe.json')
require(delivery['latestAtomicPushActualExitCode'] == delivery['latestRemoteReadbackActualExitCode'] == 0, 'recorded delivery exits')
require(delivery['remoteMatchesLocal'] and len(delivery['steps']) == 3, 'recorded remote scope')
for step in delivery['steps']:
    parents = subprocess.check_output(['git', 'show', '-s', '--format=%P', step['mainMergeCommit']], cwd=REPO, text=True).split()
    require(len(parents) == 2 and parents[1] == step['commit'], 'merge parent mismatch')
    require(subprocess.run(['git', 'merge-base', '--is-ancestor', step['commit'], 'HEAD'], cwd=REPO).returncode == 0, 'content ancestry')
completion = read(HERE / 'completion-safe.json')
require(completion['technicalRemainingScopes'] == [], 'remaining technical scope')
print(json.dumps({'status': 'final-original-requirement-reconciliation-verified', 'requirements': 14,
                  'foundationalGroups': 6, 'evidenceLinks': links, 'delegatedReviews': 16,
                  'recordedFollowOnMergePairs': 3, 'nativeOrModelActions': 0}, indent=2))
