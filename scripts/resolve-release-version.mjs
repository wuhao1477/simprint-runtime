import fs from 'node:fs/promises';

const cargoToml = await fs.readFile(new URL('../Cargo.toml', import.meta.url), 'utf8');
const match = cargoToml.match(/^version\s*=\s*"([^"]+)"/m);

if (!match) {
  throw new Error('Failed to resolve package.version from Cargo.toml');
}

const cargoVersion = match[1];
const eventName = process.env.GITHUB_EVENT_NAME || '';
const refName = process.env.GITHUB_REF_NAME || '';

if (eventName === 'push') {
  const tagVersion = refName.replace(/^v/, '');
  if (tagVersion !== cargoVersion) {
    throw new Error(
      `Tag version "${tagVersion}" does not match Cargo.toml version "${cargoVersion}"`
    );
  }
}

process.stdout.write(cargoVersion);
