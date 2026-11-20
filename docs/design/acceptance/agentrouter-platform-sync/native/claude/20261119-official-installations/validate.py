#!/usr/bin/env python3
"""Offline validation of this Claude evidence archive only."""
from pathlib import Path
import hashlib
import json
import re
import runpy
def need(v,label):
    if not v:
        raise ValueError(label)
def digest(p):
    b=p.read_bytes()
    return hashlib.sha256(b).hexdigest(),len(b)
def validate(root):
    root=Path(root)
    manifest=json.loads((root/'artifact-manifest.json').read_bytes())
    rows=manifest['files']
    names={r['path'] for r in rows}
    need(len(names)==len(rows),'duplicate manifest path')
    actual={p.relative_to(root).as_posix() for p in root.rglob('*') if p.is_file()}
    need(actual==names|{'artifact-manifest.json'},'archive inventory mismatch')
    forbidden={'.exe','.msix','.appx','.deb','.nupkg','.dll','.asar','.log','.png','.jpg','.zip','.tar','.gpg','.p12'}
    for row in rows:
        rel=Path(row['path'])
        need(not rel.is_absolute() and '..' not in rel.parts and row['path']==rel.as_posix(),'unsafe manifest path')
        p=root/rel
        need(not p.is_symlink(),'symlink artifact')
        need(digest(p)==(row['sha256'],row['bytes']),'hash/size mismatch: '+row['path'])
        need(p.suffix.lower() not in forbidden,'excluded artifact extension')
        b=p.read_bytes()
        need(not b.startswith((b'MZ',b'\x7fELF',b'PK\x03\x04',b'\x89PNG')),'binary artifact')
        text=b.decode('utf-8')
        need(not re.search(r"-{5}BEGIN (?:OPENSSH |RSA |EC )?PRIVATE KEY-{5}",text),'private key material')
        need(not re.search(r"/home/[a-z0-9_.-]+/|[a-z]:[\\/]+Users[\\/]+[a-z0-9_.-]+",text,re.I),'machine-specific user home path')
    need((root/'.gitattributes').read_text()=='* -text\n','byte-preserving attributes')
    windows=runpy.run_path(str(root/'windows/validate-windows-archive.py'))['run'](root/'windows')
    linux=runpy.run_path(str(root/'linux/validate-linux-archive.py'))['validate'](root/'linux')
    summary=json.loads((root/'summary-safe.json').read_bytes())
    def receipt(relative):
        return json.loads((root/relative).read_bytes())
    wfinal=receipt('windows/original/installed-windows-native-final-safe.json')
    wfresh=receipt('windows/original/current-registered-installed-proof-safe.json')
    wterminal=receipt('windows/original/winget-store-install-terminal-safe.json')
    wgui=receipt('windows/derived/windows-optional-gui-status-safe.json')
    linstall=receipt('linux/evidence/20261116/install-result-safe.json')
    lversion=receipt('linux/evidence/20261116/native-version-result-safe.json')
    lgui=receipt('linux/evidence/20261119/native-gui-result-safe.json')
    lfinal=receipt('linux/evidence/20261119/final-native-linux-verification-safe.json')
    expected_windows={
        'nativeInstallationCompleted':wfinal['actualNativeExitCode']==wfinal['nativeInstallerExitCode']==0,
        'successfulChannel':'official Microsoft Store native WinGet install',
        'nativeInstallerExitCode':wterminal['actualNativeExitCode'],
        'originalInstallerAndConhostHandlesTerminalCount':len(wterminal['identities']),
        'registeredPackageVersion':wfresh['registeredPackageVersion'],
        'registeredPackageStatus':wfresh['registeredPackageStatus'],
        'registeredPackageSignatureKind':wfresh['registeredPackageSignatureKind'],
        'installedExeVersion':wfresh['installedExeFileVersion'],
        'installedAsarPackageVersion':wfinal['actualInstalledAsarPackageVersion'],
        'installedExeAuthenticodeStatus':wfresh['installedExeAuthenticodeStatus'],
        'installedExeSignerSimpleName':wfresh['installedExeSignerSimpleName'],
        'currentRegistrationAndDigestRecheckObservedAtUtc':wfresh['observedAtUtc'],
        'currentReadOnlyNativeExitCode':wfresh['actualNativeExitCode'],
        'installationEvidence':'windows/original/installed-windows-native-final-safe.json',
        'currentEvidence':'windows/original/current-registered-installed-proof-safe.json',
        'independentGuiSmokeClaimed':wgui['WindowsVisibleWindowOrNormalGuiExitClaimed'],
        'externalExistingApplicationRetained':not wgui['existingExternalProcessesStoppedOrSignaled'],
        'optionalGuiStatusEvidence':'windows/derived/windows-optional-gui-status-safe.json',
        'authOrModelAvailabilityClaimed':wgui['authenticationOrModelAvailabilityClaimed'],
    }
    expected_linux={
        'nativeInstallationCompleted':linstall['exitCode']==linstall['aptInstallation']['exitCode']==0,
        'signedRepositoryChainRetainedForOfflineValidation':linux['retainedInReleaseSignatureVerifiedOffline'],
        'installedPackageVersion':lfinal['installedPackageVersion'],
        'nativeAptInstallationExitCode':linstall['aptInstallation']['exitCode'],
        'nativeVersionExitCode':lversion['actualExitCode'],
        'nativeGuiExitCode':lgui['actualExitCode'],
        'allOriginalGuiPidfdsTerminal':all(r['terminalByOriginalPidfd'] for r in lgui['heldObjects']),
        'originalGuiPidfdCount':len(lgui['heldObjects']),
        'nativeProcessesAfterGuiExit':lfinal['nativeProcessCountAfterExit'],
        'dpkgVerifyMismatchCount':lfinal['dpkgVerifyMismatchCount'],
        'nativeWSLgOwnedWindowAndNormalCtrlQQuitVerified':lgui['success'] and lgui['ownedWindow']['mapped'] and lgui['actualExitCode']==0,
        'renderedState':lgui['renderedState'],
        'authOrModelAvailabilityClaimed':lgui['loginOrModelAvailabilityProven'],
        'installationEvidence':'linux/evidence/20261116/install-result-safe.json',
        'finalEvidence':'linux/evidence/20261119/final-native-linux-verification-safe.json',
    }
    need(summary['windows']==expected_windows,'Windows summary contradicts validated receipts')
    need(summary['linux']==expected_linux,'Linux summary contradicts validated receipts')
    need(summary['windows']['nativeInstallationCompleted'] and summary['linux']['nativeInstallationCompleted'],'installation summary')
    need(summary['windows']['registeredPackageVersion']=='5.11492.5.0','Windows registered version')
    need(summary['windows']['installedExeVersion']==summary['windows']['installedAsarPackageVersion']==summary['linux']['installedPackageVersion']=='5.11492.5','cross-platform version')
    need(not summary['windows']['independentGuiSmokeClaimed'] and summary['windows']['externalExistingApplicationRetained'],'Windows GUI limit')
    need(summary['linux']['nativeWSLgOwnedWindowAndNormalCtrlQQuitVerified'] and summary['linux']['originalGuiPidfdCount']==16,'Linux GUI evidence')
    need(not summary['windows']['authOrModelAvailabilityClaimed'] and not summary['linux']['authOrModelAvailabilityClaimed'],'availability overclaim')
    need(summary['binariesProfilesRawLogsAndScreenshotsIncluded'] is False and summary['archiveAssemblyNativeInstallationOrGuiActions']==0,'assembly scope')
    return {'status':'pass','archiveFiles':len(actual),'archiveBytesIncludingManifest':sum((root/p).stat().st_size for p in actual),'windows':windows,'linux':linux,'liveActions':0,'networkAccess':False,'originalPrivateHostArtifactsRequired':False}
if __name__=='__main__':
    print(json.dumps(validate(Path(__file__).resolve().parent),sort_keys=True))
