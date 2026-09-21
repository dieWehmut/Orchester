'use strict';

const targets = require('../targets.json');

class UnsupportedTargetError extends Error {
  constructor(platform, arch) {
    super(`Unsupported target: ${platform}/${arch}`);
    this.name = 'UnsupportedTargetError';
    this.code = 'ORCHESTER_UNSUPPORTED_TARGET';
    this.platform = platform;
    this.arch = arch;
  }
}

function resolveTarget(platform = process.platform, arch = process.arch) {
  const target = targets.find(
    (candidate) => candidate.platform === platform && candidate.arch === arch,
  );

  if (!target) {
    throw new UnsupportedTargetError(platform, arch);
  }

  return target;
}

module.exports = {
  UnsupportedTargetError,
  resolveTarget,
};
