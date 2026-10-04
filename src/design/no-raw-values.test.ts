// @vitest-environment node
// Enforces .claude/rules/frontend-m3e.md: component CSS uses tokens only.
import { describe, expect, it } from 'vitest';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const SRC = join(process.cwd(), 'src');

function moduleCssFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return moduleCssFiles(path);
    return name.endsWith('.module.css') ? [path] : [];
  });
}

// px/em/rem literals and hex colours. `0` without unit and percentages are fine.
const RAW = /(?<![\w-])(\d*\.?\d+)(px|em|rem)\b|#[0-9a-fA-F]{3,8}\b/g;

describe('component CSS', () => {
  const files = moduleCssFiles(SRC);

  it('finds the component stylesheets', () => {
    expect(files.length).toBeGreaterThan(0);
  });

  it.each(files.map((f) => [f.replace(SRC, 'src')]))('%s uses tokens only', (rel) => {
    const css = readFileSync(join(process.cwd(), rel), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
    expect(css.match(RAW) ?? []).toEqual([]);
  });

  it('detects a raw value (counter-check)', () => {
    expect('.a { width: 32px; color: #fff }'.match(RAW)).toEqual(['32px', '#fff']);
  });
});
