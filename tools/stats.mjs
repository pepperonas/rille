#!/usr/bin/env node
// Computes the numbers behind the README headline badges (version, lines of code, test code,
// unit tests per suite) and writes shields.io endpoint files to .github/badges/*.json.
// CI runs this on every push to main and commits the result when a number moved, so the badges
// never go stale. A wrong badge is worse than none: if anything cannot be measured, this fails
// instead of publishing a guess.
//
//   node tools/stats.mjs            measure everything, write .github/badges/*.json
//   node tools/stats.mjs --print    measure and print, write nothing
//   node tools/stats.mjs --no-run   skip running test suites (LoC/version only; tests from files)

import { execFileSync } from 'node:child_process';
import { mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = fileURLToPath(new URL('..', import.meta.url));
const BADGES = join(ROOT, '.github', 'badges');

const SKIP_DIRS = new Set([
  'node_modules', 'target', 'dist', '.git', 'gen', 'icons', '.playwright-mcp',
  'playwright-report', 'test-results', 'temp',
]);
/** Generated files are not lines anyone wrote. */
const SKIP_FILES = new Set(['src/design/color.generated.css']);

export function walk(dir, root = ROOT) {
  const out = [];
  for (const name of readdirSync(dir)) {
    if (SKIP_DIRS.has(name) || name.startsWith('.')) continue;
    const path = join(dir, name);
    if (statSync(path).isDirectory()) out.push(...walk(path, root));
    else out.push(relative(root, path));
  }
  return out.sort();
}

/** Non-blank lines that are not whole-line comments. */
export function sloc(text, lang) {
  let n = 0;
  let inBlock = false;
  for (const raw of text.split('\n')) {
    const line = raw.trim();
    if (!line) continue;
    if (inBlock) {
      if (line.includes('*/')) inBlock = false;
      continue;
    }
    if (line.startsWith('/*')) {
      if (!line.includes('*/')) inBlock = true;
      continue;
    }
    if (lang !== 'css' && line.startsWith('//')) continue;
    if (line.startsWith('*')) continue;
    n++;
  }
  return n;
}

/**
 * Split a Rust file into production and test lines: by project convention unit tests live in a
 * trailing `#[cfg(test)] mod tests` block.
 */
export function splitRust(text) {
  const idx = text.search(/^\s*#\[cfg\(test\)\]\s*\n\s*(pub\(crate\) )?mod \w+/m);
  if (idx < 0) return { code: text, test: '' };
  return { code: text.slice(0, idx), test: text.slice(idx) };
}

export function languageOf(path) {
  if (path.endsWith('.rs')) return 'rust';
  if (/\.(ts|tsx|mjs)$/.test(path)) return 'ts';
  if (path.endsWith('.css')) return 'css';
  return null;
}

export function isTestFile(path) {
  return (
    /\.test\.(ts|tsx|mjs)$/.test(path) ||
    path.startsWith('e2e/') ||
    /^crates\/[^/]+\/tests\//.test(path) ||
    path.includes('/testutil.rs')
  );
}

export function countLines(files, read = (p) => readFileSync(join(ROOT, p), 'utf8')) {
  const code = { rust: 0, ts: 0, css: 0 };
  let test = 0;
  for (const path of files) {
    const lang = languageOf(path);
    if (!lang || SKIP_FILES.has(path) || path.startsWith('tools/') || path.endsWith('.config.ts')) {
      continue;
    }
    const text = read(path);
    if (isTestFile(path)) {
      test += sloc(text, lang);
    } else if (lang === 'rust') {
      const { code: c, test: t } = splitRust(text);
      code.rust += sloc(c, lang);
      test += sloc(t, lang);
    } else {
      code[lang] += sloc(text, lang);
    }
  }
  return { code, test };
}

export function readVersions() {
  const pkg = JSON.parse(readFileSync(join(ROOT, 'package.json'), 'utf8')).version;
  const tauri = JSON.parse(readFileSync(join(ROOT, 'src-tauri/tauri.conf.json'), 'utf8')).version;
  const cargo = /\[workspace\.package\][^[]*?version\s*=\s*"([^"]+)"/s.exec(
    readFileSync(join(ROOT, 'Cargo.toml'), 'utf8'),
  )?.[1];
  return { pkg, tauri, cargo };
}

/** Parse `cargo test -- --list --format terse` output. */
export function parseCargoList(text) {
  return text.split('\n').filter((l) => /: test$/.test(l.trim())).length;
}

/** Parse the last line of `playwright test --list`: "Total: 9 tests in 1 file". */
export function parsePlaywrightList(text) {
  const m = /Total: (\d+) tests?/.exec(text);
  return m ? Number(m[1]) : null;
}

function run(cmd, args) {
  return execFileSync(cmd, args, { cwd: ROOT, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], maxBuffer: 64 << 20 });
}

function measureTests() {
  const rust = parseCargoList(run('cargo', ['test', '--workspace', '--quiet', '--', '--list', '--format', 'terse']));
  const report = join(tmpdir(), `rille-vitest-${process.pid}.json`);
  run('pnpm', ['exec', 'vitest', 'run', '--reporter=json', `--outputFile=${report}`]);
  const vitestJson = JSON.parse(readFileSync(report, 'utf8'));
  rmSync(report, { force: true });
  const frontend = vitestJson.numTotalTests;
  const e2e = parsePlaywrightList(run('pnpm', ['exec', 'playwright', 'test', '--list']));
  for (const [name, n] of Object.entries({ rust, frontend, e2e })) {
    if (!Number.isInteger(n) || n <= 0) throw new Error(`could not count ${name} tests (${n})`);
  }
  return { rust, frontend, e2e };
}

/** 12 345 → "12.3k", 987 → "987" (the style brutus and flipper use). */
export function compact(n) {
  return n >= 1000 ? `${(n / 1000).toFixed(1).replace(/\.0$/, '')}k` : String(n);
}

export function badges({ version, loc, tests }) {
  const total = tests.rust + tests.frontend;
  const codeTotal = loc.code.rust + loc.code.ts + loc.code.css;
  const b = (label, message, color, logo) => ({
    schemaVersion: 1,
    label,
    message,
    color,
    ...(logo ? { namedLogo: logo, logoColor: 'white' } : {}),
  });
  return {
    version: b('version', version, '5B4BFF', 'tauri'),
    'unit-tests': b('unit tests', String(total), '2E9E5B', 'rust'),
    loc: b('lines of code', compact(codeTotal), '4B6BDF', 'rust'),
    'test-code': b('test code', compact(loc.test), '2E9E5B', 'rust'),
    'tests-rust': b('Rust tests', String(tests.rust), 'DEA584', 'rust'),
    'tests-frontend': b('frontend tests', String(tests.frontend), '6E9F18', 'vitest'),
    'tests-e2e': b('e2e tests', String(tests.e2e), '2EAD33', 'playwright'),
    'loc-rust': b('Rust', compact(loc.code.rust), 'DEA584', 'rust'),
    'loc-ts': b('TypeScript', compact(loc.code.ts), '3178C6', 'typescript'),
    'loc-css': b('CSS', compact(loc.code.css), '663399', 'css'),
  };
}

function main() {
  const args = new Set(process.argv.slice(2));
  const versions = readVersions();
  if (new Set(Object.values(versions)).size !== 1) {
    throw new Error(`version mismatch: ${JSON.stringify(versions)}`);
  }
  const loc = countLines(walk(ROOT));
  const tests = measureTests();
  const all = badges({ version: versions.pkg, loc, tests });
  if (args.has('--print')) {
    console.log(JSON.stringify({ version: versions.pkg, loc, tests }, null, 2));
    return;
  }
  mkdirSync(BADGES, { recursive: true });
  for (const [name, data] of Object.entries(all)) {
    writeFileSync(join(BADGES, `${name}.json`), `${JSON.stringify(data, null, 2)}\n`);
  }
  console.log(`badges: v${versions.pkg}, ${all.loc.message} LoC, ${all['unit-tests'].message} unit tests, ${tests.e2e} e2e`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main();
