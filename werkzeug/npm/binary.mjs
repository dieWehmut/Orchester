function binaryError(code, message) {
  const error = new Error(message);
  error.code = code;
  throw error;
}

function expectedMachine(target) {
  if (target.arch === 'x64') {
    return target.platform === 'linux' ? 0x3e : target.platform === 'darwin' ? 0x01000007 : 0x8664;
  }
  return target.platform === 'linux' ? 0xb7 : target.platform === 'darwin' ? 0x0100000c : 0xaa64;
}

function validateElf(bytes, target) {
  if (bytes.length < 20 || !bytes.subarray(0, 4).equals(Buffer.from([0x7f, 0x45, 0x4c, 0x46])) || bytes[4] !== 2 || bytes[5] !== 1) {
    binaryError('ORCHESTER_NPM_BINARY_INVALID_FORMAT', `native executable is not a 64-bit little-endian ELF for ${target.rustTarget}`);
  }
  if (bytes.readUInt16LE(18) !== expectedMachine(target)) {
    binaryError('ORCHESTER_NPM_BINARY_WRONG_ARCH', `native executable architecture does not match ${target.rustTarget}`);
  }
}

function validateMachO(bytes, target) {
  if (bytes.length < 8 || !bytes.subarray(0, 4).equals(Buffer.from([0xcf, 0xfa, 0xed, 0xfe]))) {
    binaryError('ORCHESTER_NPM_BINARY_INVALID_FORMAT', `native executable is not a 64-bit little-endian Mach-O for ${target.rustTarget}`);
  }
  if (bytes.readUInt32LE(4) !== expectedMachine(target)) {
    binaryError('ORCHESTER_NPM_BINARY_WRONG_ARCH', `native executable architecture does not match ${target.rustTarget}`);
  }
}

function validatePe(bytes, target) {
  if (bytes.length < 0x40 || bytes[0] !== 0x4d || bytes[1] !== 0x5a) {
    binaryError('ORCHESTER_NPM_BINARY_INVALID_FORMAT', `native executable is not a PE image for ${target.rustTarget}`);
  }
  const peOffset = bytes.readUInt32LE(0x3c);
  if (peOffset > bytes.length - 6 || !bytes.subarray(peOffset, peOffset + 4).equals(Buffer.from([0x50, 0x45, 0, 0]))) {
    binaryError('ORCHESTER_NPM_BINARY_INVALID_FORMAT', `native executable is not a PE image for ${target.rustTarget}`);
  }
  if (bytes.readUInt16LE(peOffset + 4) !== expectedMachine(target)) {
    binaryError('ORCHESTER_NPM_BINARY_WRONG_ARCH', `native executable architecture does not match ${target.rustTarget}`);
  }
}

export function validateNativeHeader(input, target) {
  const bytes = Buffer.from(input);
  if (target.platform === 'linux') {
    validateElf(bytes, target);
  } else if (target.platform === 'darwin') {
    validateMachO(bytes, target);
  } else if (target.platform === 'win32') {
    validatePe(bytes, target);
  } else {
    binaryError('ORCHESTER_NPM_BINARY_INVALID_FORMAT', `unsupported native executable platform ${target.platform}`);
  }
}
