import { describe, expect, it } from 'vitest';
import { cueMixText, eqText, filterText } from './valueText';

describe('mixer value texts', () => {
  it('describes EQ positions in dB', () => {
    expect(eqText(0.5, false)).toBe('0.0 dB');
    expect(eqText(1, false)).toBe('+6.0 dB');
    expect(eqText(0.25, false)).toBe('-12.0 dB');
    expect(eqText(0, false)).toBe('Kill');
    expect(eqText(0.7, true)).toBe('Kill');
  });

  it('describes the filter', () => {
    expect(filterText(0.5)).toBe('aus');
    expect(filterText(0.52)).toBe('aus');
    expect(filterText(0)).toBe('Tiefpass 100 %');
    expect(filterText(1)).toBe('Hochpass 100 %');
  });
});

describe('cueMixText', () => {
  it('names the ends and the shares in between', () => {
    expect(cueMixText(0)).toBe('Nur Cue');
    expect(cueMixText(1)).toBe('Nur Master');
    expect(cueMixText(0.25)).toBe('Cue 75 % · Master 25 %');
    expect(cueMixText(NaN)).toBe('Cue 50 % · Master 50 %');
  });
});
