import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';

const DEFAULT_BASE_URL = 'https://pub-d9427ecc5980437ba1445fc79ea593bd.r2.dev';
const DEFAULT_CHANNEL_PATH = 'simprint-runtime';
const DEFAULT_ARTIFACT_NAME = 'simprint-runtime.exe';
const DEFAULT_TARGET_TRIPLE = 'x86_64-pc-windows-msvc';
const DEFAULT_PROTOCOL_VERSION = 1;

const args = parseArgs(process.argv.slice(2));

if (args.help) {
  printHelp();
  process.exit(0);
}

const binaryPath = requiredArg(args, 'binary');
const inputVersion = requiredArg(args, 'version');
const releaseVersion = normalizeReleaseVersion(inputVersion);
const downloadVersion = stripVersionPrefix(releaseVersion);
const targetTriple = args.target || DEFAULT_TARGET_TRIPLE;
const protocolVersion = parsePositiveInteger(
  args['protocol-version'] || String(DEFAULT_PROTOCOL_VERSION),
  'protocol-version'
);
const outputPath = args.output || 'latest.json';
const notes = args.notes || '';

const resolvedBinaryPath = path.resolve(binaryPath);
const artifactName = args['artifact-name'] || DEFAULT_ARTIFACT_NAME;
const pubDate = args['pub-date'] || new Date().toISOString();
const downloadUrl = buildDownloadUrl({
  directUrl: args.url,
  baseUrl: args['base-url'] || DEFAULT_BASE_URL,
  channelPath: args['channel-path'] || DEFAULT_CHANNEL_PATH,
  version: downloadVersion,
  artifactName,
});

const fileBuffer = await fs.readFile(resolvedBinaryPath);
const stats = await fs.stat(resolvedBinaryPath);

const fullSha256 = sha256Hex(fileBuffer);

const document = {
  version: releaseVersion,
  protocol_version: protocolVersion,
  notes,
  pub_date: pubDate,
  platforms: {
    [targetTriple]: {
      url: downloadUrl,
      size: stats.size,
      sha256: fullSha256,
    },
  },
};

const resolvedOutputPath = path.resolve(outputPath);
await fs.mkdir(path.dirname(resolvedOutputPath), { recursive: true });
await fs.writeFile(resolvedOutputPath, `${JSON.stringify(document, null, 2)}\n`, 'utf8');

console.log(`[generate-latest-json-local] Wrote ${resolvedOutputPath}`);

function parseArgs(argv) {
  const result = {};

  for (let index = 0; index < argv.length; index += 1) {
    const raw = argv[index];
    if (!raw.startsWith('--')) {
      throw new Error(`Unexpected argument: ${raw}`);
    }

    const key = raw.slice(2);
    if (!key) {
      throw new Error('Invalid empty flag');
    }

    if (key === 'help') {
      result.help = true;
      continue;
    }

    const value = argv[index + 1];
    if (value === undefined || value.startsWith('--')) {
      throw new Error(`Missing value for --${key}`);
    }

    result[key] = value;
    index += 1;
  }

  return result;
}

function requiredArg(argsObject, key) {
  const value = argsObject[key];
  if (!value) {
    throw new Error(`--${key} is required`);
  }
  return value;
}

function parsePositiveInteger(raw, key) {
  const value = Number(raw);
  if (!Number.isInteger(value) || value <= 0) {
    throw new Error(`--${key} must be a positive integer`);
  }
  return value;
}

function normalizeReleaseVersion(raw) {
  const trimmed = String(raw).trim();
  if (!trimmed) {
    throw new Error('--version cannot be empty');
  }

  return trimmed.startsWith('v') ? trimmed : `v${trimmed}`;
}

function stripVersionPrefix(version) {
  return version.startsWith('v') ? version.slice(1) : version;
}

function buildDownloadUrl({ directUrl, baseUrl, channelPath, version, artifactName }) {
  if (directUrl) {
    return directUrl;
  }

  const normalizedBaseUrl = baseUrl.replace(/\/+$/, '');
  const normalizedChannelPath = String(channelPath).replace(/^\/+|\/+$/g, '');
  return `${normalizedBaseUrl}/${normalizedChannelPath}/${version}/${artifactName}`;
}

function sha256Hex(buffer) {
  return crypto.createHash('sha256').update(buffer).digest('hex');
}

function printHelp() {
  console.log(`Usage:
  node ./scripts/generate-latest-json-local.mjs \\
    --binary target/release/simprint-runtime.exe \\
    --version 0.1.0 \\
    --output latest.json

Required:
  --binary            Path to the runtime binary
  --version           Release version written into latest.json

Download URL:
  --url               Full download URL written to platforms.<target>.url
  --base-url          Base public URL used to build the download URL
                     Default: ${DEFAULT_BASE_URL}

Optional:
  --artifact-name     File name in the download URL, default: ${DEFAULT_ARTIFACT_NAME}
  --channel-path      URL path segment before version, default: ${DEFAULT_CHANNEL_PATH}
  --target            Target triple, default: ${DEFAULT_TARGET_TRIPLE}
  --protocol-version  Protocol version, default: ${DEFAULT_PROTOCOL_VERSION}
  --output            Output path, default: latest.json
  --notes             Release notes text
  --pub-date          Override pub_date, default: current ISO timestamp
  --help              Show this help

Examples:
  node ./scripts/generate-latest-json-local.mjs \\
    --binary target/release/simprint-runtime.exe \\
    --version 0.1.2

  node ./scripts/generate-latest-json-local.mjs \\
    --binary target/release/simprint-runtime.exe \\
    --version 0.1.0 \\
    --url https://pub.example.com/simprint-runtime/0.1.0/simprint-runtime.exe
`);
}
