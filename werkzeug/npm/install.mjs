import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

import { runGlobalInstallSmoke } from './install/smoke.mjs';

export {
  FIXTURE_MARKER,
  FIXTURE_NAME,
  FIXTURE_VERSION,
  createFixtureTarball,
} from './install/fixture.mjs';
export {
  LOOPBACK_REGISTRY,
  assertInside,
  createIsolatedEnvironment,
  ensureDirectory,
  mkdirFresh,
} from './install/environment.mjs';
export {
  commandSummary,
  discoverPackageManagers,
  invokeCommand,
} from './install/command.mjs';
export {
  buildManagerPlan,
  SUPPORTED_MANAGERS,
} from './install/plans.mjs';
export {
  runGlobalInstallSmoke,
  runManagerSmoke,
} from './install/smoke.mjs';

function fail(code, message) {
  const error = new Error(message);
  error.code = code;
  throw error;
}

export function parseCommandLine(args) {
  if (args.length === 0) return { requireAll: false };
  if (args.length === 1 && args[0] === '--require-all') return { requireAll: true };
  fail('ORCHESTER_NPM_USAGE', 'usage: node werkzeug/npm/install.mjs [--require-all]');
}

const isMain = process.argv[1]
  && path.resolve(process.argv[1]) === path.resolve(fileURLToPath(import.meta.url));

if (isMain) {
  try {
    const options = parseCommandLine(process.argv.slice(2));
    const report = runGlobalInstallSmoke(options);
    for (const result of report.results) {
      if (result.status === 'skipped') {
        process.stdout.write(`SKIP ${result.name}: ${result.reason}\n`);
      } else if (result.status === 'failed') {
        const failures = [];
        if (result.executionFailure) failures.push(`entry=${result.executionFailure.code}`);
        if (!result.shimRemoved) failures.push(`residual shims=${result.residualShims.join(', ')}`);
        process.stdout.write(`FAIL ${result.name}: ${failures.join('; ')}\n`);
      } else {
        process.stdout.write(`PASS ${result.name}: ${result.marker}; shim removed=${result.shimRemoved}\n`);
      }
    }
    if (report.results.some((result) => result.status === 'failed')) {
      process.exitCode = 1;
    }
  } catch (error) {
    const message = typeof error?.code === 'string' && error.code.startsWith('ORCHESTER_NPM_')
      ? error.message
      : 'package-manager installation smoke test failed';
    process.stderr.write(`${message}\n`);
    process.exitCode = 1;
  }
}
