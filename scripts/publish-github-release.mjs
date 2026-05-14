import fs from 'node:fs/promises';
import path from 'node:path';

const token = process.env.GH_TOKEN || process.env.GITHUB_TOKEN;
const repository = process.env.GH_REPO || process.env.GITHUB_REPOSITORY;
const tag = process.env.RELEASE_TAG;
const binaryPath = path.resolve(
  process.env.RELEASE_BINARY_PATH || 'target/release/simprint-runtime.exe'
);
const latestJsonPath = path.resolve(process.env.RELEASE_LATEST_JSON_PATH || 'latest.json');

if (!token) throw new Error('GH_TOKEN or GITHUB_TOKEN is not set');
if (!repository) throw new Error('GH_REPO or GITHUB_REPOSITORY is not set');
if (!tag) throw new Error('RELEASE_TAG is not set');

const [owner, repo] = repository.split('/');
if (!owner || !repo) throw new Error(`Invalid repository: ${repository}`);

const tagNotes = await readTagNotes(tag);
const release = await ensureRelease(tagNotes);

await uploadAsset(release, binaryPath, 'application/octet-stream');
await uploadAsset(release, latestJsonPath, 'application/json');

console.log(`GitHub release publish finished for ${tag}`);

async function readTagNotes(tagName) {
  try {
    const refResponse = await githubApi(`/repos/${owner}/${repo}/git/ref/tags/${encodeURIComponent(tagName)}`, {
      okStatuses: [200, 404],
    });

    if (refResponse.status === 404) {
      console.log(`Tag ref ${tagName} not found on GitHub, fallback to generated release notes`);
      return '';
    }

    const target = refResponse.json?.object;
    if (!target || target.type !== 'tag' || !target.sha) {
      console.log(`Tag ${tagName} is not an annotated tag on GitHub, fallback to generated release notes`);
      return '';
    }

    const tagObject = await githubApi(`/repos/${owner}/${repo}/git/tags/${target.sha}`, {
      okStatuses: [200, 404],
    });

    if (tagObject.status === 404) {
      console.log(`Annotated tag object for ${tagName} not found on GitHub, fallback to generated release notes`);
      return '';
    }

    return (tagObject.json?.message || '').trim();
  } catch (error) {
    console.warn(`Failed to read GitHub tag notes for ${tagName}: ${error.message}`);
    return '';
  }
}

async function ensureRelease(tagNotes) {
  const existing = await githubApi(
    `/repos/${owner}/${repo}/releases/tags/${encodeURIComponent(tag)}`,
    { okStatuses: [200, 404] }
  );

  if (existing.status === 200) {
    console.log(`Using existing GitHub release for ${tag}`);
    return await syncExistingRelease(existing.json, tagNotes);
  }

  console.log(`Creating GitHub release for ${tag}`);
  const created = await githubApi(`/repos/${owner}/${repo}/releases`, {
    method: 'POST',
    json: buildReleasePayload(tagNotes),
    okStatuses: [201, 422],
  });

  if (created.status === 201) {
    return created.json;
  }

  const refetched = await githubApi(`/repos/${owner}/${repo}/releases/tags/${encodeURIComponent(tag)}`);
  return refetched.json;
}

async function syncExistingRelease(release, tagNotes) {
  const desiredBody = tagNotes || '';
  const currentBody = release.body || '';
  const shouldUpdateBody = Boolean(tagNotes) && currentBody !== desiredBody;
  const shouldEnableGeneratedNotes = !tagNotes && !currentBody;

  if (!shouldUpdateBody && !shouldEnableGeneratedNotes) {
    return release;
  }

  console.log(`Updating GitHub release metadata for ${tag}`);
  const updated = await githubApi(`/repos/${owner}/${repo}/releases/${release.id}`, {
    method: 'PATCH',
    json: buildReleasePayload(tagNotes),
    okStatuses: [200],
  });

  return updated.json;
}

function buildReleasePayload(tagNotes) {
  const payload = {
    tag_name: tag,
    name: tag,
  };

  if (tagNotes) {
    payload.body = tagNotes;
  } else {
    payload.generate_release_notes = true;
  }

  return payload;
}

async function uploadAsset(release, filePath, contentType) {
  const fileName = path.basename(filePath);
  const fileBuffer = await fs.readFile(filePath);

  await deleteExistingAsset(release, fileName);

  const uploadUrl = release.upload_url.replace('{?name,label}', `?name=${encodeURIComponent(fileName)}`);
  console.log(`Uploading ${fileName}`);

  await retry(`upload ${fileName}`, async () => {
    const response = await fetch(uploadUrl, {
      method: 'POST',
      headers: {
        Authorization: `Bearer ${token}`,
        Accept: 'application/vnd.github+json',
        'Content-Type': contentType,
        'Content-Length': String(fileBuffer.length),
      },
      body: fileBuffer,
    });

    if (!response.ok) {
      const text = await response.text();
      throw new Error(`upload failed ${response.status}: ${text}`);
    }
  });
}

async function deleteExistingAsset(release, fileName) {
  const asset = (release.assets || []).find((item) => item.name === fileName);
  if (!asset) return;

  console.log(`Deleting existing asset ${fileName}`);
  await githubApi(`/repos/${owner}/${repo}/releases/assets/${asset.id}`, {
    method: 'DELETE',
    okStatuses: [204],
  });
}

async function githubApi(apiPath, options = {}) {
  const url = apiPath.startsWith('http') ? apiPath : `https://api.github.com${apiPath}`;
  const headers = {
    Authorization: `Bearer ${token}`,
    Accept: 'application/vnd.github+json',
    'X-GitHub-Api-Version': '2022-11-28',
    ...options.headers,
  };

  let body;
  if (options.json !== undefined) {
    body = JSON.stringify(options.json);
    headers['Content-Type'] = 'application/json';
  } else if (options.body !== undefined) {
    body = options.body;
  }

  const response = await fetch(url, {
    method: options.method || 'GET',
    headers,
    body,
  });

  const okStatuses = options.okStatuses || [200];
  const contentType = response.headers.get('content-type') || '';
  const payload = contentType.includes('application/json')
    ? await response.json().catch(() => null)
    : await response.text().catch(() => '');

  if (!okStatuses.includes(response.status)) {
    throw new Error(`${options.method || 'GET'} ${url} failed ${response.status}: ${formatPayload(payload)}`);
  }

  return { status: response.status, json: payload };
}

async function retry(label, fn, attempts = 4) {
  let lastError;
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    try {
      await fn();
      return;
    } catch (error) {
      lastError = error;
      if (attempt === attempts) break;
      const delayMs = attempt * 2000;
      console.warn(`${label} failed on attempt ${attempt}/${attempts}: ${error.message}`);
      console.warn(`Retrying in ${delayMs}ms`);
      await new Promise((resolve) => setTimeout(resolve, delayMs));
    }
  }
  throw lastError;
}

function formatPayload(payload) {
  if (typeof payload === 'string') return payload;
  try {
    return JSON.stringify(payload);
  } catch {
    return String(payload);
  }
}
