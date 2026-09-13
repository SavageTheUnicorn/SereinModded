import { execFileSync } from 'node:child_process';
import { generateNotes as defaultGenerateNotes } from '@semantic-release/release-notes-generator';

export function getLatestReleaseTag(ref = 'HEAD') {
  try {
    execFileSync('git', ['describe', '--tags', '--match', 'v[0-9]*', '--exact-match', ref], { stdio: 'ignore' });
    return execFileSync('git', ['describe', '--tags', '--match', 'v[0-9]*', '--abbrev=0', `${ref}~1`], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim();
  } catch {
    try {
      return execFileSync('git', ['describe', '--tags', '--match', 'v[0-9]*', '--abbrev=0', ref], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim();
    } catch {
      return null;
    }
  }
}

export async function generateReleaseNotes(channel, pluginConfig, context) {
  if (channel === 'production') {
    return defaultGenerateNotes(pluginConfig, context);
  }

  const targetRef = context.nextRelease?.gitHead || 'HEAD';
  const latestTag = getLatestReleaseTag(targetRef);

  if (!latestTag) {
    return defaultGenerateNotes(pluginConfig, context);
  }

  let nightlyHashes;
  try {
    nightlyHashes = new Set(
      execFileSync('git', ['rev-list', `${latestTag}..${targetRef}`], { encoding: 'utf8' })
        .split('\n')
        .map(h => h.trim())
        .filter(Boolean)
    );
  } catch {
    return defaultGenerateNotes(pluginConfig, context);
  }

  const nightlyCommits = (context.commits || []).filter(c => nightlyHashes.has(c.hash));
  const date = new Date().toISOString().slice(0, 10).replace(/-/g, '');
  const runNumber = process.env.GITHUB_RUN_NUMBER;
  const nightlyVersion = context.nextRelease?.version?.includes('nightly')
    ? context.nextRelease.version
    : (runNumber ? `${context.nextRelease.version}-nightly.${date}.${runNumber}` : `${context.nextRelease.version}-nightly`);

  return defaultGenerateNotes(pluginConfig, {
    ...context,
    commits: nightlyCommits,
    lastRelease: {
      gitTag: latestTag,
      gitHead: latestTag,
      version: latestTag.replace(/^v/, ''),
    },
    nextRelease: {
      ...context.nextRelease,
      version: nightlyVersion,
      gitTag: `v${nightlyVersion}`,
    },
  });
}
