#!/usr/bin/env python3
"""Offline receipt consistency checks only; no native application actions."""
from pathlib import Path
import argparse
import hashlib
import json
import re
import shutil
import tempfile


def need(value, label):
    if not value:
        raise ValueError(label)


def read(path):
    return json.loads(path.read_bytes())


def digest(path):
    data = path.read_bytes()
    return hashlib.sha256(data).hexdigest(), len(data)


def validate(root):
    root = Path(root)
    manifest = read(root / 'artifact-manifest.json')
    rows = manifest['files']
    names = {row['path'] for row in rows}
    need(len(names) == len(rows), 'duplicate manifest path')
    actual = {p.relative_to(root).as_posix() for p in root.rglob('*') if p.is_file()}
    need(actual == names | {'artifact-manifest.json'}, 'archive inventory mismatch')
    for row in rows:
        relative = Path(row['path'])
        need(not relative.is_absolute() and '..' not in relative.parts and relative.as_posix() == row['path'],
             'unsafe archive path')
        path = root / relative
        need(not path.is_symlink() and digest(path) == (row['sha256'], row['bytes']),
             'archive digest/size mismatch: ' + row['path'])
        text = path.read_bytes().decode('utf-8-sig')
        need(not re.search(r'/home/[a-z0-9_.-]+/|[a-z]:[\\/]+Users[\\/]+[a-z0-9_.-]+', text, re.I),
             'private local path published')
    need((root / '.gitattributes').read_text() == '* -text\n', 'original-byte Git attributes')
    mapping = read(root / 'source-map.json')
    need(not mapping['privatePathsPublished'] and mapping['noPrivateProfilePathsOrContentsIncluded'],
         'private source boundary')
    for row in mapping['sources']:
        need(digest(root / row['archivePath']) == (row['sha256'], row['bytes']), 'original source mapping digest')
    native = read(root / 'current-preflight-safe.json')
    executor = read(root / 'current-preflight-execution-safe.json')
    summary = read(root / 'summary-safe.json')
    review = read(root / 'preparation-review-safe.json')
    need(executor['nativePowerShellExitCode'] == summary['nativeReadOnlyPreflightExitCode'] == 0,
         'actual native preflight exit')
    need(executor['stdoutBytes'] == (root / 'current-preflight-safe.json').stat().st_size and
         executor['stderrBytes'] == 0, 'native output size binding')
    need(executor['nativePowerShellSha256'] == native['currentNativePowerShellSha256'] ==
         '60c9b29843624dd8af6b6a7b26147753be6aeef65098b9a503cc50850afe410f', 'native interpreter identity')
    installed = native['officialInstalled']
    need(installed['sha256'] == summary['officialInstalledImageSha256'] ==
         'bad50d18a3cf42621ac03aeaf1dc527ee20384ca4c0b7eee64512954ddb9a76e' and
         installed['bytes'] == 246343656, 'official complete EXE identity')
    need(installed['signature'] == summary['officialInstalledSignature'] == 'Valid' and
         installed['signerSimpleName'] == native['officialRegisteredPublisher'] ==
         'Hangzhou DeepSeek Artificial Intelligence Co., Ltd.', 'official native signature')
    need(installed['numericPEVersion'] == summary['officialInstalledNumericPEVersion'] == '0.2.2.0' and
         native['officialRegisteredVersion'] == summary['officialInstalledVersion'] == '0.2.2-rc.1',
         'separate PE and application versions')
    need(native['originalDataElementImageExists'] and summary['originalDataElementRetained'] and
         native['originalDataElementRegisteredVersion'] == summary['originalDataElementRegisteredVersion'] == '0.13.0',
         'original DataElement installation retained')
    processes = native['currentKnownDeepSeekProcesses']
    need(len(processes) == summary['currentOriginalDataElementProcessCount'] == 4 and
         all(p['exactOriginalDataElementImage'] and not p['exactOfficialInstalledImage'] and
             p['knownProductName'] == 'DSH Desktop' for p in processes), 'active original shared-home application boundary')
    need(len({(p['pid'], p['creationFileTime']) for p in processes}) == 4, 'distinct observed process identities')
    need(all(not row['set'] for row in native['dshHomeScopes']) and
         {row['scope'] for row in native['dshHomeScopes']} == {'Process', 'User', 'Machine'} and
         not summary['originalDshHomeOverrideChanged'], 'unmodified original DSH_HOME environment')
    need(native['historicalHomeBaselineExists'] and native['historicalHomeBaseline']['baselineUsable'] and
         native['historicalHomeBaseline']['sha256'] == '8e3759cce27c2837c47fc82aff24bbabdb2d9b8aba132f93af0c6aeee15c0e4b' and
         native['historicalHomeBaseline']['bytes'] == 6212 and summary['historicalHomeBaselineHashVerified'],
         'historical home baseline binding')
    need(len(native['nativePowerShellAst']) == 4 and all(row['parseErrors'] == 0 for row in native['nativePowerShellAst']),
         'prepared source native AST checks')
    compiled = {row['sourceLabel']: row['compiled'] for row in native['nativeCSharpCompilation']}
    need(compiled == {'runtime-native.cs': True, 'ds-retention-native-v2.cs': False, 'ds-retention-json-v2.cs': True} and
         review['nativeCSharpRetentionV2Compiled'] is False and summary['preparedRetentionGuardV2CompilePassed'] is False,
         'preserved retention controller compile failure')
    need(all(row['matchesOriginalPreparation'] for row in review['preparedSources']) and
         not review['originalV2SourcesModified'] and not review['preparedNativeExecutionPerformed'],
         'original prepared-source review boundary')
    need(summary['actualOfficialGuiLaunches'] == 0 and
         summary['actualOfficialGuiVisibleWindowOrNormalQuitProven'] is False and
         summary['sharedHomeAfterOfficialStartupRetentionProven'] is False and
         summary['freshCurrentSharedHomeSnapshotCollected'] is False and
         summary['boundedNormalQuitPreconditionEstablished'] is False and
         summary['preparedSuccessorNativeCompileVerified'] is False,
         'unperformed runtime/retention boundary')
    need(summary['accountLoginRequiredForNativeInstallationClaim'] is False and
         summary['authOrModelAvailabilityClaimed'] is False and
         summary['requirementClassification']['nativeInstallationRequirementSatisfiedByHistoricalInstallationAndCurrentIdentity'],
         'explicit installation requirement classification')
    for document in (native, executor, summary, review):
        for field in ('appLaunches', 'actualOfficialGuiLaunches', 'processSignals', 'authActions', 'modelCalls',
                      'environmentWrites', 'profileWrites', 'installerInvocations'):
            if field in document:
                need(document[field] == 0, 'unexpected live action: ' + field)
    return {'status': 'pass', 'files': len(actual), 'nativeReadOnlyPreflightExitCode': 0,
            'officialNativeInstallationReconfirmed': True, 'originalDataElementProcessesObserved': 4,
            'officialGuiOrPostStartupRetentionProven': False, 'liveActions': 0, 'networkAccess': False}


def negative_tests(root):
    cases = [
        ('signature', 'current-preflight-safe.json', ['officialInstalled', 'signature'], 'NotSigned', 'official native signature'),
        ('active-original-processes', 'current-preflight-safe.json', ['currentKnownDeepSeekProcesses'], [], 'active original shared-home'),
        ('compile-failure-overclaim', 'summary-safe.json', ['preparedRetentionGuardV2CompilePassed'], True, 'preserved retention'),
        ('GUI-overclaim', 'summary-safe.json', ['actualOfficialGuiVisibleWindowOrNormalQuitProven'], True, 'unperformed runtime'),
        ('retention-overclaim', 'summary-safe.json', ['sharedHomeAfterOfficialStartupRetentionProven'], True, 'unperformed runtime'),
        ('account-requirement', 'summary-safe.json', ['accountLoginRequiredForNativeInstallationClaim'], True, 'explicit installation'),
    ]
    for label, relative, fields, value, expected in cases:
        with tempfile.TemporaryDirectory(prefix='deepseek-archive-negative-') as directory:
            copy = Path(directory) / 'archive'
            shutil.copytree(root, copy)
            document = read(copy / relative)
            target = document
            for field in fields[:-1]:
                target = target[field]
            target[fields[-1]] = value
            write(copy / relative, document)
            execution = read(copy / 'current-preflight-execution-safe.json')
            execution['stdoutBytes'] = (copy / 'current-preflight-safe.json').stat().st_size
            write(copy / 'current-preflight-execution-safe.json', execution)
            mapping = read(copy / 'source-map.json')
            for row in mapping['sources']:
                row['sha256'], row['bytes'] = digest(copy / row['archivePath'])
            write(copy / 'source-map.json', mapping)
            manifest = read(copy / 'artifact-manifest.json')
            for row in manifest['files']:
                row['sha256'], row['bytes'] = digest(copy / row['path'])
            write(copy / 'artifact-manifest.json', manifest)
            try:
                validate(copy)
            except ValueError as error:
                need(expected in str(error), label + ': unexpected rejection ' + str(error))
            else:
                raise ValueError('negative case accepted: ' + label)
    return {'status': 'pass', 'rejectedCases': len(cases), 'ordinaryDigestMetadataRefreshed': True}


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--negative', action='store_true')
    args = parser.parse_args()
    archive = Path(__file__).resolve().parent
    result = validate(archive)
    if args.negative:
        result['negativeValidation'] = negative_tests(archive)
    print(json.dumps(result, indent=2))
