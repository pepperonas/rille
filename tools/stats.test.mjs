// @vitest-environment node
import { describe, expect, it } from 'vitest';
import {
  badges,
  compact,
  countLines,
  isTestFile,
  parseCargoList,
  parsePlaywrightList,
  readVersions,
  sloc,
  splitRust,
} from './stats.mjs';

describe('sloc', () => {
  it('drops blank lines and whole-line comments', () => {
    const text = '// a\nlet x = 1;\n\n/* block\n still */\nlet y = 2; // trailing stays\n * doc\n';
    expect(sloc(text, 'ts')).toBe(2);
  });

  it('keeps CSS lines that merely contain //', () => {
    expect(sloc('a { background: url(//x) }', 'css')).toBe(1);
  });
});

describe('splitRust', () => {
  it('separates the trailing test module', () => {
    const { code, test } = splitRust('fn a() {}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {}\n}\n');
    expect(sloc(code, 'rust')).toBe(1);
    expect(sloc(test, 'rust')).toBe(5);
  });

  it('treats files without tests as all code', () => {
    expect(splitRust('fn a() {}').test).toBe('');
  });
});

describe('countLines', () => {
  it('sorts files into code per language and test code', () => {
    const files = {
      'src/a.ts': 'const a = 1;\nconst b = 2;',
      'src/a.test.ts': 'it("x", () => {});',
      'src/a.module.css': '.a {}',
      'src/design/color.generated.css': ':root {}',
      'crates/x/src/lib.rs': 'fn a() {}\n#[cfg(test)]\nmod tests {}',
      'crates/x/tests/it.rs': 'fn t() {}',
      'README.md': '# not code',
    };
    const r = countLines(Object.keys(files), (p) => files[p]);
    expect(r.code).toEqual({ rust: 1, ts: 2, css: 1 });
    expect(r.test).toBe(1 + 2 + 1);
  });

  it('recognises test files', () => {
    expect(isTestFile('e2e/deck.spec.ts')).toBe(true);
    expect(isTestFile('crates/rille-midi/tests/x.rs')).toBe(true);
    expect(isTestFile('src/App.tsx')).toBe(false);
  });
});

describe('test counting', () => {
  it('counts cargo test listings', () => {
    expect(parseCargoList('a::b: test\nc: test\nsomething: bench\n\n')).toBe(2);
  });

  it('reads the playwright total', () => {
    expect(parsePlaywrightList('Listing tests:\n  …\nTotal: 9 tests in 1 file')).toBe(9);
    expect(parsePlaywrightList('Total: 1 test in 1 file')).toBe(1);
    expect(parsePlaywrightList('no tests')).toBeNull();
  });
});

describe('badges', () => {
  it('formats like the other celox repos', () => {
    expect(compact(987)).toBe('987');
    expect(compact(11_300)).toBe('11.3k');
    expect(compact(2000)).toBe('2k');
  });

  it('produces shields endpoint payloads', () => {
    const b = badges({
      version: '1.2.3',
      loc: { code: { rust: 1000, ts: 500, css: 100 }, test: 800 },
      tests: { rust: 10, frontend: 5, e2e: 2 },
    });
    expect(b.version).toMatchObject({ schemaVersion: 1, label: 'version', message: '1.2.3' });
    expect(b['unit-tests'].message).toBe('15');
    expect(b.loc.message).toBe('1.6k');
    expect(b['tests-e2e'].message).toBe('2');
  });
});

describe('version', () => {
  it('is the same in Cargo.toml, package.json and tauri.conf.json', () => {
    const v = readVersions();
    expect(v.cargo).toBe(v.pkg);
    expect(v.tauri).toBe(v.pkg);
  });
});
