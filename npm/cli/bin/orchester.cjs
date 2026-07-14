#!/usr/bin/env node
'use strict';

const {
  MissingNativePackageError,
  SPAWN_ERROR_MESSAGE,
  runNativeCli,
} = require('../lib/process.cjs');

function fail(error) {
  const message = error instanceof MissingNativePackageError
    ? error.message
    : SPAWN_ERROR_MESSAGE;

  process.stderr.write(`${message}\n`);
  process.exitCode = 1;
}

try {
  runNativeCli().catch(fail);
} catch (error) {
  fail(error);
}
