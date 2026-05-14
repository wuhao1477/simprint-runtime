import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';

const binaryPath = requiredEnv('BINARY_PATH');
const releaseVersion = requiredEnv('RELEASE_VERSION');
const r2PublicBaseUrl = requiredEnv('R2_PUBLIC_BASE_URL').replace(/\/+$/, '');
const targetTriple = requiredEnv('TARGET_TRIPLE');
const protocolVersion = Number(requiredEnv('PROTOCOL_VERSION'));
const outputPath = process.env.OUTPUT_PATH || 'latest.json';
const notes = process.env.RELEASE_NOTES || '';

if (!Number.isInteger(protocolVersion) || protocolVersion <= 0) {
  throw new Error(`Invalid PROTOCOL_VERSION: ${process.env.PROTOCOL_VERSION}`);
}

const resolvedBinaryPath = path.resolve(binaryPath);
const fileBuffer = await fs.readFile(resolvedBinaryPath);
const stats = await fs.stat(resolvedBinaryPath);

const fullSha256 = sha256Hex(fileBuffer);

const document = {
  version: releaseVersion,
  protocol_version: protocolVersion,
  notes,
  pub_date: new Date().toISOString(),
  platforms: {
    [targetTriple]: {
      url: `${r2PublicBaseUrl}/simprint-runtime/${releaseVersion}/simprint-runtime.exe`,
      size: stats.size,
      sha256: fullSha256,
    },
  },
};

const resolvedOutputPath = path.resolve(outputPath);
await fs.mkdir(path.dirname(resolvedOutputPath), { recursive: true });
await fs.writeFile(resolvedOutputPath, `${JSON.stringify(document, null, 2)}\n`, 'utf8');

function requiredEnv(name) {
  const value = process.env[name];
  if (!value) {
    throw new Error(`${name} is not set`);
  }
  return value;
}

function sha256Hex(buffer) {
  return crypto.createHash('sha256').update(buffer).digest('hex');
}
