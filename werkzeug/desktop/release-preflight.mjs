/**
 * What has to be true before either release workflow is dispatched.
 *
 * The two workflows validate the version in different places, and a mismatch is
 * only discovered after the run starts - which on a release means after a tag
 * and a public release exist. This asks the same questions locally, plus two the
 * workflows do not: the plugin *manifests* (which `plugin-release.mjs` checks
 * later, in the staging job) and the three Cargo lockfiles (which `--locked`
 * checks implicitly, but only once a build starts).
 *
 * It deliberately does not touch the network or pnpm-lock.yaml: the six
 * `@orchester/cli-*` entries in that lockfile can only be refreshed after the
 * npm packages exist, so a preflight that demanded them would refuse every
 * release forever. See docs/RELEASE-PROCEDURE.md for the order that resolves
 * that.
 */
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { verifyOfficialAgentPlugins } from '../npm/plugin-release.mjs';

const moduleDirectory = path.dirname(fileURLToPath(import.meta.url));

export function fail(message) {
  const error = new Error(`release-preflight: ${message}`);
  error.code = 'ORCHESTER_RELEASE_PREFLIGHT';
  throw error;
}

export function parseArguments(args) {
  const options = {};
  for (let index = 0; index < args.length; index += 1) {
    const flag = args[index];
    if (!flag.startsWith('--')) fail(`unexpected argument ${flag}`);
    const value = args[index + 1];
    if (value === undefined || value.startsWith('--')) fail(`${flag} requires a value`);
    options[flag.slice(2)] = value;
    index += 1;
  }
  return options;
}

export const SEMVER = /^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z.-]+)?$/;

/** The three files `desktop-release` reads the version from. */
export const DESKTOP_MANIFESTS = [
  'apps/desktop/package.json',
  'apps/desktop/src-tauri/tauri.conf.json',
  'apps/desktop/src-tauri/Cargo.toml',
];

/** The Cargo manifests whose lockfiles must agree with them. */
export const CARGO_ROOT_MANIFESTS = [
  'Cargo.toml',
  'apps/desktop/src-tauri/Cargo.toml',
  'apps/desktop/native-chrome/Cargo.toml',
];

/** The version declared in a JSON manifest or a Cargo manifest. */
export function manifestVersion(root, relative) {
  const file = path.join(root, relative);
  const text = fs.readFileSync(file, 'utf8');
  if (relative.endsWith('.json')) {
    const parsed = JSON.parse(text);
    return typeof parsed.version === 'string' ? parsed.version : null;
  }
  const match = /^version = "(.*)"\r?$/m.exec(text);
  return match?.[1] ?? null;
}

/** The CLI's six platform packages, as pinned in its own manifest. */
export function cliPlatformPins(root) {
  const manifest = JSON.parse(fs.readFileSync(path.join(root, 'apps/cli/package.json'), 'utf8'));
  return manifest.optionalDependencies ?? {};
}

/**
 * The plugin half, judged by the verifier the release itself runs.
 *
 * `npm-release`'s staging job calls `plugin-release.mjs`, which checks each
 * plugin package against its canonical adapter manifest, the package matrix and
 * the CLI's version. Asking the same function here means the preflight asserts
 * what CI asserts rather than a second opinion that could drift from it.
 */
export function pluginProblem(root, version) {
  try {
    verifyOfficialAgentPlugins({ expectedVersion: version, repositoryRoot: root });
    return null;
  } catch (error) {
    return error instanceof Error ? error.message : 'official plugin verification failed';
  }
}

/** The version each package of a Cargo workspace reports, with `--locked`. */
export function cargoPackageVersions(root, relative, toolchain) {
  const result = spawnSync(
    'cargo',
    [
      ...(toolchain ? [`+${toolchain}`] : []),
      'metadata',
      '--locked',
      '--no-deps',
      '--format-version',
      '1',
      '--manifest-path',
      relative,
    ],
    { cwd: root, encoding: 'utf8', shell: false },
  );
  if (result.error) return { error: `${relative}: cargo did not start: ${result.error.message}` };
  if (result.status !== 0) {
    const detail = (result.stderr ?? '').trim().split('\n')[0] ?? '';
    return { error: `${relative}: cargo metadata --locked exited ${result.status}: ${detail}` };
  }
  try {
    const metadata = JSON.parse(result.stdout ?? '');
    return {
      packages: Object.fromEntries(
        (metadata.packages ?? []).map((entry) => [entry.name, entry.version]),
      ),
    };
  } catch (error) {
    return { error: `${relative}: cargo metadata did not return JSON: ${error.message}` };
  }
}

/**
 * Every problem standing between this commit and a dispatch.
 *
 * Pure: the facts it judges arrive as arguments, so the judgement can be tested
 * without a repository, a toolchain, or a network.
 */
export function collectProblems({ version, desktopVersions, cliVersion, platformPins, pluginError, cargo, tags }) {
  const problems = [];
  if (!SEMVER.test(version)) {
    // The workflows stop at their own regex, so this does too: a malformed
    // version would otherwise report every manifest as "not equal to nonsense".
    return [`version ${version} is not pinned semver`];
  }

  for (const [relative, found] of Object.entries(desktopVersions)) {
    if (found !== version) problems.push(`${relative} is ${found ?? 'missing'}, not ${version}`);
  }
  if (cliVersion !== version) problems.push(`apps/cli/package.json is ${cliVersion ?? 'missing'}, not ${version}`);
  for (const [name, pinned] of Object.entries(platformPins)) {
    if (pinned !== version) problems.push(`${name} is pinned to ${pinned}, not ${version}`);
  }
  if (pluginError) problems.push(pluginError);
  for (const [relative, outcome] of Object.entries(cargo)) {
    if (outcome.error) {
      problems.push(outcome.error);
      continue;
    }
    for (const [name, found] of Object.entries(outcome.packages ?? {})) {
      if (found !== version) problems.push(`${relative}: ${name} is ${found}, not ${version}`);
    }
  }
  for (const [tag, commit] of Object.entries(tags)) {
    // The workflows tolerate a tag that already points at the release commit and
    // refuse any other, because a tag is the immutable name of one build.
    if (commit !== 'HEAD') problems.push(`${tag} already exists on ${commit}, which is not this commit`);
  }
  return problems;
}

/** The commits the release tags point at, for a version. */
export function tagCommits(root, version) {
  const tags = {};
  for (const tag of [`v${version}`, `desktop-v${version}`]) {
    const resolved = spawnSync('git', ['rev-list', '-n', '1', tag], {
      cwd: root,
      encoding: 'utf8',
      shell: false,
    });
    if (resolved.status !== 0) continue;
    const head = spawnSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8', shell: false });
    tags[tag] = (resolved.stdout ?? '').trim() === (head.stdout ?? '').trim() ? 'HEAD' : (resolved.stdout ?? '').trim().slice(0, 7);
  }
  return tags;
}

/** The order the two dispatches have to happen in, and why. */
export function nextSteps(version) {
  return [
    `1. Dispatch npm-release with version ${version}.  It publishes the CLI and the`,
    '   plugins, tags v' + version + ', and creates the GitHub Release.  It does not',
    '   run pnpm install, so the stale pnpm-lock.yaml does not stop it.',
    `2. Refresh pnpm-lock.yaml now that the packages exist (pnpm install --lockfile-only)`,
    `   and commit it.  Its @orchester/cli-* entries carry published integrity hashes.`,
    `3. Dispatch desktop-release with version ${version}.  Its test job runs`,
    '   pnpm install --frozen-lockfile, which fails while the lockfile disagrees.',
  ];
}

function main() {
  const options = parseArguments(process.argv.slice(2));
  const root = path.resolve(options.root ?? path.join(moduleDirectory, '../..'));
  const cliVersion = manifestVersion(root, 'apps/cli/package.json');
  const version = options.version ?? cliVersion;
  if (version === null || version === undefined) fail('a version is required (--version)');

  const desktopVersions = Object.fromEntries(
    DESKTOP_MANIFESTS.map((relative) => [relative, manifestVersion(root, relative)]),
  );
  const cargo = Object.fromEntries(
    CARGO_ROOT_MANIFESTS.map((relative) => [
      relative,
      cargoPackageVersions(root, path.join(root, relative), options.toolchain),
    ]),
  );

  const problems = collectProblems({
    version,
    desktopVersions,
    cliVersion,
    platformPins: cliPlatformPins(root),
    pluginError: pluginProblem(root, version),
    cargo,
    tags: tagCommits(root, version),
  });

  if (problems.length > 0) {
    process.stderr.write(`release-preflight: ${problems.length} problem(s) for ${version}\n`);
    for (const problem of problems) process.stderr.write(`  ${problem}\n`);
    process.exit(1);
  }

  process.stdout.write(`release-preflight: ${version} is consistent everywhere both workflows look\n`);
  for (const line of nextSteps(version)) process.stdout.write(`  ${line}\n`);
}

const invokedDirectly = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (invokedDirectly) {
  try {
    main();
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exit(1);
  }
}