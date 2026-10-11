import { describe, expect, it } from 'vitest';
import {
  beatAt,
  beatsLabel,
  follow,
  frameOfBeat,
  gridFromTap,
  JUMP_BEATS,
  LOOP_BEATS,
  quantizeCue,
  scaleGrid,
  shiftGrid,
  tapTempo,
  tempoForRate,
  wrapHalf,
} from './beat';

const SR = 48_000;
const G = { bpm: 120, firstBeat: 0.25 };

describe('beat arithmetic (same rules as the engine)', () => {
  it('converts beats and frames', () => {
    expect(beatAt(G, 1.25 * SR, SR)).toBe(2);
    expect(frameOfBeat(G, 2, SR)).toBe(60_000);
    expect(wrapHalf(0.7)).toBeCloseTo(-0.3);
    expect(wrapHalf(-0.7)).toBeCloseTo(0.3);
  });

  it('follows at half or double tempo when that is closer', () => {
    expect(follow(174, 87.5)).toEqual({ rate: 175 / 174, beatsPerTargetBeat: 2 });
    expect(follow(85, 172)?.beatsPerTargetBeat).toBe(0.5);
    expect(follow(0, 120)).toBeNull();
  });

  it('keeps the tempo range when it reaches, else picks the narrowest that does', () => {
    expect(tempoForRate(1.05, 'ten')).toEqual({ range: 'ten', tempo: expect.closeTo(0.5) });
    expect(tempoForRate(1.12, 'six')?.range).toBe('sixteen');
    expect(tempoForRate(1.7, 'ten')).toBeNull();
  });

  it('snaps hot cues to a close beat only', () => {
    const beat2 = frameOfBeat(G, 2, SR);
    expect(quantizeCue(G, beat2 + 2000, SR)).toBe(beat2);
    expect(quantizeCue(G, beat2 + 6000, SR)).toBe(beat2 + 6000);
    expect(quantizeCue(null, 1234, SR)).toBe(1234);
  });

  it('labels pads like the controller layout', () => {
    expect(LOOP_BEATS.map(beatsLabel)).toEqual(['¼', '½', '1', '2', '4', '8', '16', '32']);
    expect(JUMP_BEATS.map(beatsLabel)).toEqual(['−½', '−1', '−4', '−16', '½', '1', '4', '16']);
  });
});

describe('tap tempo', () => {
  const taps = (ms: number, n: number, from = 0) =>
    Array.from({ length: n }, (_, i) => from + i * ms);

  it('needs four taps and measures the tempo', () => {
    expect(tapTempo(taps(500, 3))).toBeNull();
    expect(tapTempo(taps(500, 4))).toBe(120);
    expect(tapTempo(taps(468.75, 8))).toBe(128);
  });

  it('ignores a missed or doubled tap', () => {
    const t = taps(500, 8);
    t.splice(4, 1); // one tap missed: a 1000 ms gap
    expect(tapTempo(t)).toBe(120);
    expect(tapTempo([...taps(500, 6), 2620, 2700])).toBe(120); // a stray double tap
  });

  it('starts afresh after a pause', () => {
    expect(tapTempo([...taps(400, 5), ...taps(500, 4, 10_000)])).toBe(120);
  });
});

describe('grid corrections', () => {
  it('keeps the first beat within one period', () => {
    expect(shiftGrid(G, 0.01).firstBeat).toBeCloseTo(0.26);
    expect(shiftGrid(G, -0.3).firstBeat).toBeCloseTo(0.45);
    expect(shiftGrid(G, 0.3).firstBeat).toBeCloseTo(0.05);
  });

  it('halves and doubles within sane limits', () => {
    expect(scaleGrid(G, 2).bpm).toBe(240);
    expect(scaleGrid({ bpm: 300, firstBeat: 0 }, 2).bpm).toBe(400);
    expect(scaleGrid(G, 0.5)).toEqual({ bpm: 60, firstBeat: 0.25 });
  });

  it('puts the tapped beat where the tap was', () => {
    const g = gridFromTap(120, 10.3 * SR, SR);
    expect(g.bpm).toBe(120);
    expect(wrapHalf(beatAt(g, 10.3 * SR, SR))).toBeCloseTo(0);
  });
});
