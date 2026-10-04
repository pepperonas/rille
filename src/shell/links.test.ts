// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { LINKS } from './links';

/**
 * Same matching as tauri-plugin-opener: `glob::Pattern` with default options, where `*` matches
 * any characters (including `/` and `.`) and `?` any single character.
 */
function globToRegExp(pattern: string): RegExp {
  let re = '';
  for (const ch of pattern) {
    if (ch === '*') re += '.*';
    else if (ch === '?') re += '.';
    else re += ch.replace(/[.+^$()[\]{}|\\/]/g, '\\$&');
  }
  return new RegExp(`^${re}$`);
}

function allowed(url: string, patterns: string[]): boolean {
  return patterns.some((p) => globToRegExp(p).test(url));
}

describe('external links', () => {
  const capability = JSON.parse(
    readFileSync(join(process.cwd(), 'src-tauri/capabilities/default.json'), 'utf8'),
  ) as { permissions: Array<string | { identifier: string; allow: Array<{ url: string }> }> };
  const opener = capability.permissions.find(
    (p): p is { identifier: string; allow: Array<{ url: string }> } =>
      typeof p === 'object' && p.identifier === 'opener:allow-open-url',
  );
  const patterns = opener?.allow.map((a) => a.url) ?? [];

  it.each(Object.entries(LINKS))('%s may be opened', (_name, url) => {
    expect(allowed(url, patterns)).toBe(true);
  });

  it('nothing else may be opened', () => {
    expect(allowed('https://evil.example/', patterns)).toBe(false);
    expect(allowed('file:///etc/passwd', patterns)).toBe(false);
    // `*` in a glob crosses dots and slashes: lookalike hosts must not slip through.
    expect(allowed('https://celox.io.evil.example/', patterns)).toBe(false);
    expect(allowed('https://github.com/pepperonas/rille-evil', patterns)).toBe(false);
    expect(allowed('https://www.paypal.com/donate/?business=someone@else.com', patterns)).toBe(false);
  });

  it('README and app use the same donate and rating links', () => {
    const readme = readFileSync(join(process.cwd(), 'README.md'), 'utf8');
    expect(readme).toContain(LINKS.donate);
    expect(readme).toContain(LINKS.rate);
  });
});
