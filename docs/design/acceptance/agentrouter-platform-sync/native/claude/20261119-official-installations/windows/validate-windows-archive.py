#!/usr/bin/env python3
"""Validate archived Windows evidence only. No host install, GUI, network or profile access."""
from pathlib import Path
import datetime
import hashlib
import json
import re
def need(value, label):
    if not value:
        raise ValueError(label)
def digest(path):
    b=path.read_bytes()
    return hashlib.sha256(b).hexdigest(),len(b)
def run(root):
    root=Path(root)
    mapping=json.loads((root/'source-map.json').read_bytes())
    sources={s['id']:s for s in mapping['sources']}
    need(len(sources)==len(mapping['sources']), "Windows evidence invariant: len(sources)==len(mapping['sources'])")
    for s in sources.values():
        need(re.fullmatch('[0-9a-f]{64}',s['sha256']) and s['bytes']>0, "Windows evidence invariant: re.fullmatch('[0-9a-f]{64}',s['sha256']) and s['bytes']>0")
    for a in mapping['publishedArtifacts']:
        relative=Path(a['path'])
        need(not relative.is_absolute() and '..' not in relative.parts and relative.as_posix()==a['path'], "Windows evidence invariant: not relative.is_absolute() and '..' not in relative.parts and relative.as_posix()==a['path']")
        path=root/relative
        need(path.is_file() and not path.is_symlink(), 'Windows evidence invariant: path.is_file() and not path.is_symlink()')
        need(digest(path)==(a['sha256'],a['bytes']), a['path'])
        need(a['representation'] in ('original-bytes','derived-selected-fields'), "Windows evidence invariant: a['representation'] in ('original-bytes','derived-selected-fields')")
        need(a['sourceIds'] and all(s in sources for s in a['sourceIds']), "Windows evidence invariant: a['sourceIds'] and all(s in sources for s in a['sourceIds'])")
        if a['representation']=='original-bytes':
            need(len(a['sourceIds'])==1, "Windows evidence invariant: len(a['sourceIds'])==1")
            s=sources[a['sourceIds'][0]]
            need((a['sha256'],a['bytes'])==(s['sha256'],s['bytes']), "Windows evidence invariant: (a['sha256'],a['bytes'])==(s['sha256'],s['bytes'])")
        else:
            need(a['derivation'], "Windows evidence invariant: a['derivation']")
    def j(name):
        return json.loads((root/name).read_bytes())
    show=j('original/winget-store-show-start-safe.json')
    shown=j('original/winget-store-show-terminal-safe.json')
    start=j('original/winget-store-install-start-safe.json')
    terminal=j('original/winget-store-install-terminal-safe.json')
    reg=j('original/winget-store-installed-registration-safe.json')
    final=j('original/installed-windows-native-final-safe.json')
    fresh=j('original/current-registered-installed-proof-safe.json')
    labels=j('original/store-install-terminal-field-label-reconciliation-safe.json')
    package_label=j('derived/appinstaller-package-version-label-clarification-safe.json')
    catalog=j('derived/official-store-catalog-20261116-safe.json')
    publicshow=j('derived/official-store-show-public-metadata-safe.json')
    prior=j('derived/prior-failure-and-truncated-candidate-safe.json')
    gui=j('derived/windows-optional-gui-status-safe.json')
    rejected=j('original/native-windows-gui-failure-safe.json')
    need(show['arguments'].startswith('show ') and show['installOperations']==0, "Windows evidence invariant: show['arguments'].startswith('show ') and show['installOperations']==0")
    need(shown['actualNativeExitCode']==0 and shown['originalCreatorAndQueryHandlesHeldToExit'], "Windows evidence invariant: shown['actualNativeExitCode']==0 and shown['originalCreatorAndQueryHandlesHeldToExit']")
    need(start['arguments'].startswith('install ') and '--source msstore' in start['arguments'], "Windows evidence invariant: start['arguments'].startswith('install ') and '--source msstore' in start['arguments']")
    need('--silent' in start['arguments'] and start['installOperations']==1, "Windows evidence invariant: '--silent' in start['arguments'] and start['installOperations']==1")
    need(start['wingetAuthenticodeStatus']=='Valid', "Windows evidence invariant: start['wingetAuthenticodeStatus']=='Valid'")
    need(package_label['originalStartReceiptsPreserved'] is True, "Windows evidence invariant: package_label['originalStartReceiptsPreserved'] is True")
    need(package_label['appInstallerPackageVersion']==package_label['showStartReportedWingetVersion']==package_label['installStartReportedWingetVersion']==start['wingetVersion']==show['wingetVersion']=='1.29.379.0', "Windows evidence invariant: package_label['appInstallerPackageVersion']==package_label['showStartReportedWingetVersion']==package_label['installStartReportedWingetVersion']==start['wingetVersion']==show['wingetVersion']=='1.29.379.0'")
    need(package_label['wingetCliVersionIndependentlyObserved'] is False, "Windows evidence invariant: package_label['wingetCliVersionIndependentlyObserved'] is False")
    need(terminal['actualNativeExitCode']==0 and terminal['installOperations']==1, "Windows evidence invariant: terminal['actualNativeExitCode']==0 and terminal['installOperations']==1")
    need(terminal['originalCreatorAndQueryHandlesHeldToExit'], "Windows evidence invariant: terminal['originalCreatorAndQueryHandlesHeldToExit']")
    need(terminal['explicitLoginActions']==terminal['modelAuthCalls']==terminal['sharedWorkerOperations']==terminal['processSignals']==0, "Windows evidence invariant: terminal['explicitLoginActions']==terminal['modelAuthCalls']==terminal['sharedWorkerOperations']==terminal['processSignals']==0")
    ids=terminal['identities']
    need(len(ids)==2 and all(i['nativeExitCode']==0 and i['originalHandleLive'] is False for i in ids), "Windows evidence invariant: len(ids)==2 and all(i['nativeExitCode']==0 and i['originalHandleLive'] is False for i in ids)")
    main,child=ids
    need(main['pid']==start['nativePid'], "Windows evidence invariant: main['pid']==start['nativePid']")
    need(main['createdFileTimeExact']==start['nativeCreatedFileTimeExact'], "Windows evidence invariant: main['createdFileTimeExact']==start['nativeCreatedFileTimeExact']")
    need(main['imageSha256']==start['wingetImageSha256'], "Windows evidence invariant: main['imageSha256']==start['wingetImageSha256']")
    need(child['parentPid']==main['pid'], "Windows evidence invariant: child['parentPid']==main['pid']")
    need(child['parentCreatedFileTimeExact']==main['createdFileTimeExact'], "Windows evidence invariant: child['parentCreatedFileTimeExact']==main['createdFileTimeExact']")
    need(child['image'].lower().endswith('conhost.exe'), "Windows evidence invariant: child['image'].lower().endswith('conhost.exe')")
    need(all(int(i['endedFileTimeExact'])>=int(i['createdFileTimeExact']) for i in ids), "Windows evidence invariant: all(int(i['endedFileTimeExact'])>=int(i['createdFileTimeExact']) for i in ids)")
    need(final['nativeInstallerTerminalProofSha256']==digest(root/'original/winget-store-install-terminal-safe.json')[0], "Windows evidence invariant: final['nativeInstallerTerminalProofSha256']==digest(root/'original/winget-store-install-terminal-safe.json')[0]")
    need(labels['originalTerminalReceiptSha256']==final['nativeInstallerTerminalProofSha256'], "Windows evidence invariant: labels['originalTerminalReceiptSha256']==final['nativeInstallerTerminalProofSha256']")
    need(labels['originalReceiptPreserved'] is True and labels['actualCommandArguments']==start['arguments'], "Windows evidence invariant: labels['originalReceiptPreserved'] is True and labels['actualCommandArguments']==start['arguments']")
    need(final['nativeInstallationRegistrationProofSha256']==digest(root/'original/winget-store-installed-registration-safe.json')[0], "Windows evidence invariant: final['nativeInstallationRegistrationProofSha256']==digest(root/'original/winget-store-installed-registration-safe.json')[0]")
    need(reg['nativeInstallerExitCode']==0 and reg['currentUserPackageCount']==1, "Windows evidence invariant: reg['nativeInstallerExitCode']==0 and reg['currentUserPackageCount']==1")
    installed=reg['currentUserPackages'][0]
    need(installed['name']=='AnthropicPBC.Claude', "Windows evidence invariant: installed['name']=='AnthropicPBC.Claude'")
    need(installed['version']=='5.11492.5.0' and installed['status']=='Ok' and installed['signatureKind']=='Store', "Windows evidence invariant: installed['version']=='5.11492.5.0' and installed['status']=='Ok' and installed['signatureKind']=='Store'")
    for observed,recorded in [('name','registeredPackageName'),('version','registeredPackageVersion'),
                              ('packageFullName','registeredPackageFullName'),('familyName','registeredPackageFamilyName'),
                              ('status','registeredPackageStatus'),('publisher','registeredPackagePublisher'),
                              ('signatureKind','registeredPackageSignatureKind'),('installLocation','installationRoot')]:
        need(installed[observed]==final[recorded], 'registration/final identity mismatch: '+observed)
    need(final['actualNativeExitCode']==0 and final['nativeInstallerExitCode']==0, "Windows evidence invariant: final['actualNativeExitCode']==0 and final['nativeInstallerExitCode']==0")
    need(final['modelCalls']==final['explicitLoginActions']==final['sharedWorkerOperations']==final['processSignals']==final['forcedTerminations']==0, "Windows evidence invariant: final['modelCalls']==final['explicitLoginActions']==final['sharedWorkerOperations']==final['processSignals']==final['forcedTerminations']==0")
    need(final['registeredPackageVersion']=='5.11492.5.0' and final['registeredPackageSignatureKind']=='Store', "Windows evidence invariant: final['registeredPackageVersion']=='5.11492.5.0' and final['registeredPackageSignatureKind']=='Store'")
    need(final['actualInstalledExeAuthenticodeStatus']=='Valid', "Windows evidence invariant: final['actualInstalledExeAuthenticodeStatus']=='Valid'")
    need(final['actualInstalledExeSignerSimpleName']=='Anthropic, PBC', "Windows evidence invariant: final['actualInstalledExeSignerSimpleName']=='Anthropic, PBC'")
    need(final['actualInstalledExeFileVersion']==final['actualInstalledExeProductVersion']==final['actualInstalledAsarPackageVersion']=='5.11492.5', "Windows evidence invariant: final['actualInstalledExeFileVersion']==final['actualInstalledExeProductVersion']==final['actualInstalledAsarPackageVersion']=='5.11492.5'")
    need(final['actualInstalledAsarPackageName']=='@ant/desktop', "Windows evidence invariant: final['actualInstalledAsarPackageName']=='@ant/desktop'")
    need(final['actualInstalledExeBytes']==249955664 and final['actualInstalledAsarBytes']==88787202, "Windows evidence invariant: final['actualInstalledExeBytes']==249955664 and final['actualInstalledAsarBytes']==88787202")
    need(final['actualInstalledExeSha256']=='093fb6529006fe5fbe28b61231484734dab78a497dc1b05311f2315590118414', "Windows evidence invariant: final['actualInstalledExeSha256']=='093fb6529006fe5fbe28b61231484734dab78a497dc1b05311f2315590118414'")
    need(final['actualInstalledAsarSha256']=='47433b3c91cdbc813de9d277dfb5b0fbc21d17a7b3a49528e07ef1edc0935ed2', "Windows evidence invariant: final['actualInstalledAsarSha256']=='47433b3c91cdbc813de9d277dfb5b0fbc21d17a7b3a49528e07ef1edc0935ed2'")
    need(fresh['priorInstallationProofSha256']==digest(root/'original/installed-windows-native-final-safe.json')[0], "Windows evidence invariant: fresh['priorInstallationProofSha256']==digest(root/'original/installed-windows-native-final-safe.json')[0]")
    need(fresh['registeredPackageVersion']=='5.11492.5.0' and fresh['registeredPackageStatus']=='Ok', "Windows evidence invariant: fresh['registeredPackageVersion']=='5.11492.5.0' and fresh['registeredPackageStatus']=='Ok'")
    need(fresh['actualNativeExitCode']==0, "Windows evidence invariant: fresh['actualNativeExitCode']==0")
    for field in ['registeredPackageName','registeredPackageVersion','registeredPackageFullName',
                  'registeredPackageFamilyName','registeredPackageStatus','registeredPackageSignatureKind']:
        need(fresh[field]==final[field], 'current registration identity mismatch: '+field)
    for observed,recorded in [('installedExeBytes','actualInstalledExeBytes'),
                              ('installedAsarBytes','actualInstalledAsarBytes'),
                              ('installedExeFileVersion','actualInstalledExeFileVersion'),
                              ('installedExeProductVersion','actualInstalledExeProductVersion'),
                              ('installedExeSignerThumbprint','actualInstalledExeSignerThumbprint')]:
        need(fresh[observed]==final[recorded], 'current installed-file metadata mismatch: '+observed)
    need(fresh['installedExeSha256']==final['actualInstalledExeSha256'], "Windows evidence invariant: fresh['installedExeSha256']==final['actualInstalledExeSha256']")
    need(fresh['installedAsarSha256']==final['actualInstalledAsarSha256'], "Windows evidence invariant: fresh['installedAsarSha256']==final['actualInstalledAsarSha256']")
    need(fresh['installedExeAuthenticodeStatus']=='Valid' and fresh['installedExeSignerSimpleName']=='Anthropic, PBC', "Windows evidence invariant: fresh['installedExeAuthenticodeStatus']=='Valid' and fresh['installedExeSignerSimpleName']=='Anthropic, PBC'")
    need(fresh['installationOperations']==fresh['GUIActions']==fresh['processSignals']==fresh['modelOrAuthenticationCalls']==0, "Windows evidence invariant: fresh['installationOperations']==fresh['GUIActions']==fresh['processSignals']==fresh['modelOrAuthenticationCalls']==0")
    need(catalog['observedAtUtc'].startswith('2026-11-16'), "Windows evidence invariant: catalog['observedAtUtc'].startswith('2026-11-16')")
    need(catalog['packageFullName']==final['registeredPackageFullName'], "Windows evidence invariant: catalog['packageFullName']==final['registeredPackageFullName']")
    need(catalog['sourceBodySha256']==sources['S015']['sha256'] and catalog['sourceBodyBytes']==sources['S015']['bytes'], "Windows evidence invariant: catalog['sourceBodySha256']==sources['S015']['sha256'] and catalog['sourceBodyBytes']==sources['S015']['bytes']")
    need(catalog['storeProductId']==final['officialStoreProductId'], "Windows evidence invariant: catalog['storeProductId']==final['officialStoreProductId']")
    need(catalog['packageIdentityName']==final['registeredPackageName'], "Windows evidence invariant: catalog['packageIdentityName']==final['registeredPackageName']")
    need(catalog['packageFamilyName']==final['registeredPackageFamilyName'], "Windows evidence invariant: catalog['packageFamilyName']==final['registeredPackageFamilyName']")
    need(catalog['publisherCertificateName']==final['registeredPackagePublisher'], "Windows evidence invariant: catalog['publisherCertificateName']==final['registeredPackagePublisher']")
    need(catalog['catalogHashIsMetadataOnly'] and not catalog['containerPayloadDigestIndependentlyRecomputed'], "Windows evidence invariant: catalog['catalogHashIsMetadataOnly'] and not catalog['containerPayloadDigestIndependentlyRecomputed']")
    need(publicshow['reportedVersion']=='Unknown' and publicshow['installerType']=='msstore', "Windows evidence invariant: publicshow['reportedVersion']=='Unknown' and publicshow['installerType']=='msstore'")
    need(publicshow['publisher']=='Anthropic, PBC' and publicshow['installationProvedByThisShowOutput'] is False, "Windows evidence invariant: publicshow['publisher']=='Anthropic, PBC' and publicshow['installationProvedByThisShowOutput'] is False")
    need(publicshow['nativeShowExitCode']==shown['actualNativeExitCode'], "Windows evidence invariant: publicshow['nativeShowExitCode']==shown['actualNativeExitCode']")
    need(publicshow['productId']==final['officialStoreProductId'], "Windows evidence invariant: publicshow['productId']==final['officialStoreProductId']")
    need(publicshow['rawStdoutSha256']==sources['S017']['sha256'] and publicshow['rawStdoutBytes']==sources['S017']['bytes'], "Windows evidence invariant: publicshow['rawStdoutSha256']==sources['S017']['sha256'] and publicshow['rawStdoutBytes']==sources['S017']['bytes']")
    need(prior['priorInstallObservedAtUtc'].startswith('2026-11-16'), "Windows evidence invariant: prior['priorInstallObservedAtUtc'].startswith('2026-11-16')")
    need(prior['failedNativeInstallerExitCode']==-1 and prior['priorAllObservedHandlesExited'], "Windows evidence invariant: prior['failedNativeInstallerExitCode']==-1 and prior['priorAllObservedHandlesExited']")
    need(prior['priorObservedNativeIdentityCount']==2 and prior['priorObservedExitCodes']==[-1,-1], "Windows evidence invariant: prior['priorObservedNativeIdentityCount']==2 and prior['priorObservedExitCodes']==[-1,-1]")
    need(prior['truncatedCertificateTableWithinFile'] is False and prior['truncatedExactlyMatchesPinnedExePrefix'], "Windows evidence invariant: prior['truncatedCertificateTableWithinFile'] is False and prior['truncatedExactlyMatchesPinnedExePrefix']")
    need(prior['truncatedCandidateExecuted'] is False and prior['partialFilesPreserved'], "Windows evidence invariant: prior['truncatedCandidateExecuted'] is False and prior['partialFilesPreserved']")
    need(prior['truncatedCandidateAcceptedAsCurrentSetup'] is False and prior['pinnedVersionClaimedLatest'] is False, "Windows evidence invariant: prior['truncatedCandidateAcceptedAsCurrentSetup'] is False and prior['pinnedVersionClaimedLatest'] is False")
    need(prior['priorOriginalCreatorAndQueryHandlesHeldToExit'] and prior['binariesOrRawInstallerLogsIncluded'] is False, "Windows evidence invariant: prior['priorOriginalCreatorAndQueryHandlesHeldToExit'] and prior['binariesOrRawInstallerLogsIncluded'] is False")
    need(prior['processSignals']==prior['modelOrAuthenticationCalls']==0, "Windows evidence invariant: prior['processSignals']==prior['modelOrAuthenticationCalls']==0")
    need(rejected['started'] is False and rejected['staticCode']=='existing-Claude-process-do-not-duplicate', "Windows evidence invariant: rejected['started'] is False and rejected['staticCode']=='existing-Claude-process-do-not-duplicate'")
    need(gui['existingInstalledImageProcessCount']==6 and gui['existingNativeSessionIds']==[1], "Windows evidence invariant: gui['existingInstalledImageProcessCount']==6 and gui['existingNativeSessionIds']==[1]")
    need(not gui['independentGuiSmokeAttemptStartedAnyProcess'] and not gui['existingExternalProcessesStoppedOrSignaled'], "Windows evidence invariant: not gui['independentGuiSmokeAttemptStartedAnyProcess'] and not gui['existingExternalProcessesStoppedOrSignaled']")
    need(not gui['WindowsVisibleWindowOrNormalGuiExitClaimed'] and gui['WindowsInstallationVerifiedSeparately'], "Windows evidence invariant: not gui['WindowsVisibleWindowOrNormalGuiExitClaimed'] and gui['WindowsInstallationVerifiedSeparately']")
    need(not gui['authenticationOrModelAvailabilityClaimed'], "Windows evidence invariant: not gui['authenticationOrModelAvailabilityClaimed']")
    need(gui['observerVisibleWindowCount']==gui['processSignals']==gui['GUIActions']==0, "Windows evidence invariant: gui['observerVisibleWindowCount']==gui['processSignals']==gui['GUIActions']==0")
    need(gui['duplicateLaunchRejectedBeforeCreateProcess'] and gui['existingProcessesStartedByThisTask'] is False, "Windows evidence invariant: gui['duplicateLaunchRejectedBeforeCreateProcess'] and gui['existingProcessesStartedByThisTask'] is False")
    need(gui['existingUserWindowScreenshotIncluded'] is False and gui['privateProfileContentsRead'] is False, "Windows evidence invariant: gui['existingUserWindowScreenshotIncluded'] is False and gui['privateProfileContentsRead'] is False")
    need(datetime.datetime.fromisoformat(gui['existingRootCreatedUtc'].replace('Z','+00:00'))>datetime.datetime.fromisoformat(terminal['observedAtUtc'].replace('Z','+00:00')), "Windows evidence invariant: datetime.datetime.fromisoformat(gui['existingRootCreatedUtc'].replace('Z','+00:00'))>datetime.datetime.fromisoformat(terminal['observedAtUtc'].replace('Z','+00:00'))")
    for f in (final,fresh):
        need(f.get('privateProfileContentsRead',f.get('privateProfileConfigurationAuthOrUserDataRead')) is False, "Windows evidence invariant: f.get('privateProfileContentsRead',f.get('privateProfileConfigurationAuthOrUserDataRead')) is False")
    forbidden_exts={'.exe','.msix','.appx','.deb','.nupkg','.dll','.log','.png','.jpg','.zip','.tar','.p12'}
    for p in root.rglob('*'):
        if not p.is_file():
            continue
        need(not p.is_symlink() and p.suffix.lower() not in forbidden_exts, p.name)
        b=p.read_bytes()
        need(not b.startswith((b'MZ',b'\x7fELF',b'PK\x03\x04',b'\x89PNG')), p.name)
        text=b.decode('utf-8')
        need(not re.search(r"/home/[a-z0-9_.-]+/|[a-z]:[\\/]+Users[\\/]+[a-z0-9_.-]+",text,re.I), p.name)
        need(not re.search(r"-{5}BEGIN (?:OPENSSH |RSA |EC )?PRIVATE KEY-{5}",text), p.name)
    return {'status':'windows-archive-validated','mappedArtifactCount':len(mapping['publishedArtifacts']),'sourceCount':len(sources),'installedVersion':'5.11492.5','currentRegisteredVersion':'5.11492.5.0','independentWindowsGuiClaimed':False,'authenticationOrModelAvailabilityClaimed':False}
if __name__=='__main__':
    print(json.dumps(run(Path(__file__).resolve().parent),sort_keys=True))
