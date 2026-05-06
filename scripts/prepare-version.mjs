import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

function normalizeTagToVersion(input) {
  if (!input) return null;
  const raw = String(input).trim();
  if (!raw) return null;
  const version = raw.startsWith('v') ? raw.slice(1) : raw;
  if (!/^\d+\.\d+\.\d+([\-+].+)?$/.test(version)) {
    console.log(
      `[prepare-version] Ignore non-version input "${raw}". Expected like v1.2.3 or 1.2.3`
    );
    return null;
  }
  return version;
}

function updateCargoTomlVersion(content, version) {
  const pattern = /(\[package\][\s\S]*?\nversion\s*=\s*")([^"]+)(")/m;
  const match = content.match(pattern);

  if (!match) {
    throw new Error('Failed to locate [package].version in Cargo.toml');
  }

  if (match[2] === version) {
    return { changed: false, content };
  }

  return {
    changed: true,
    content: content.replace(pattern, `$1${version}$3`),
  };
}

const version =
  normalizeTagToVersion(process.env.VERSION) ??
  normalizeTagToVersion(process.env.RELEASE_TAG) ??
  normalizeTagToVersion(process.env.GITHUB_REF_NAME);

if (!version) {
  console.log('[prepare-version] No tag/version detected. Skipping version update.');
  process.exit(0);
}

const cargoTomlPath = fileURLToPath(new URL('../Cargo.toml', import.meta.url));
const cargoToml = await fs.readFile(cargoTomlPath, 'utf8');
const { changed, content } = updateCargoTomlVersion(cargoToml, version);

if (changed) {
  await fs.writeFile(cargoTomlPath, content, 'utf8');
  console.log(`[prepare-version] Updated Cargo.toml -> package.version=${version}`);
} else {
  console.log(`[prepare-version] Kept Cargo.toml -> package.version=${version}`);
}

process.stdout.write(version);
