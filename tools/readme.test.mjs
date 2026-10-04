// @vitest-environment node
// Keeps the READMEs honest: every image and badge file they point to exists, both languages
// show the same pictures, and the changelog knows the current version.
import { describe, expect, it } from 'vitest';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { readVersions } from './stats.mjs';

const ROOT = join(import.meta.dirname, '..');
const read = (f) => readFileSync(join(ROOT, f), 'utf8');
const READMES = ['README.md', 'README.de.md'];

const localImages = (text) => [...text.matchAll(/src="(docs\/[^"]+\.png)"/g)].map((m) => m[1]);
const badgeFiles = (text) =>
  [...text.matchAll(/main\/(\.github\/badges\/[\w-]+\.json)/g)].map((m) => m[1]);

describe.each(READMES)('%s', (file) => {
  const text = read(file);

  it('only references images that exist', () => {
    const images = localImages(text);
    expect(images.length).toBeGreaterThan(0);
    for (const img of images) expect(existsSync(join(ROOT, img)), img).toBe(true);
  });

  it('only references badge data that exists', () => {
    const badges = badgeFiles(text);
    expect(badges).toContain('.github/badges/version.json');
    for (const b of badges) expect(existsSync(join(ROOT, b)), b).toBe(true);
  });

  it('has the donate and rating buttons', () => {
    expect(text).toContain('paypal.com/donate/?business=martin.pfeffer@celox.io');
    expect(text).toContain('g.page/r/CXgdRV3QysvxEBM/review');
  });
});

describe('translations', () => {
  it('show the same screenshots', () => {
    expect(localImages(read('README.de.md')).sort()).toEqual(localImages(read('README.md')).sort());
  });
});

describe('changelog', () => {
  it('has an entry for the current version', () => {
    expect(read('CHANGELOG.md')).toContain(`## [${readVersions().pkg}]`);
  });
});
