// WebGL2 renderer for the scrolling detail waveform: one full-screen triangle per frame, the
// fragment shader reads the peaks of the chosen zoom level from a texture and draws the bands,
// the beatgrid and the dimmed past. Nothing is uploaded per frame except a few uniforms.
import { BANDS, binRate, chooseLevel, type Peaks } from './peaks';
import type { WaveColors } from './colors';

/** Texture row length; peaks are wrapped into rows of this many bins. */
const ROW = 4096;
/** Most bins one pixel may combine (`chooseLevel` keeps it below). */
const MAX_SPAN = 8;

const VERTEX = `#version 300 es
void main() {
  // One triangle that covers the viewport.
  vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
  gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}`;

const FRAGMENT = `#version 300 es
precision highp float;
precision highp int;
uniform highp sampler2D u_peaks;
uniform vec2 u_size;          // device pixels
uniform float u_center;       // seconds at the playhead (middle)
uniform float u_spp;          // seconds per device pixel
uniform float u_binRate;      // bins per second at this level
uniform int u_bins;           // bins at this level
uniform vec3 u_low, u_mid, u_high, u_grid;
uniform float u_bpm, u_firstBeat; // bpm 0 = no grid
uniform float u_duration;         // seconds; no grid outside the track
uniform vec2 u_loop;              // seconds; x < 0: no loop
uniform float u_cues[8];          // seconds; < 0: empty
uniform vec3 u_loopColor, u_cueColor;
out vec4 color;

vec3 bins(float from, float to) {
  int first = max(0, int(floor(from * u_binRate)));
  int last = min(u_bins, max(first + 1, int(ceil(to * u_binRate))));
  vec3 m = vec3(0.0);
  if (to <= 0.0) return m;
  for (int k = 0; k < ${MAX_SPAN}; k++) {
    int i = first + k;
    if (i >= last) break;
    m = max(m, texelFetch(u_peaks, ivec2(i % ${ROW}, i / ${ROW}), 0).rgb);
  }
  return m;
}

void main() {
  float x = gl_FragCoord.x;
  float t = u_center + (x - u_size.x * 0.5) * u_spp;
  vec3 v = bins(t, t + u_spp);
  float a = abs(gl_FragCoord.y - u_size.y * 0.5) / (u_size.y * 0.5);
  // Premultiplied colour throughout. The loop tints the background behind the waveform.
  vec4 c = vec4(0.0);
  if (u_loop.x >= 0.0 && t >= u_loop.x && t < u_loop.y) c = vec4(u_loopColor * 0.25, 0.25);
  if (a <= v.r) c = vec4(u_low, 1.0);
  if (a <= v.g * 0.8) c = vec4(u_mid, 1.0);
  if (a <= v.b * 0.55) c = vec4(u_high, 1.0);
  // Beatgrid: a line on every beat, stronger on every fourth, only within the track.
  if (u_bpm > 0.0 && t >= 0.0 && t <= u_duration) {
    float period = 60.0 / u_bpm;
    float n = (t - u_firstBeat) / period;
    float dist = abs(fract(n + 0.5) - 0.5) * period / u_spp;
    float beat = floor(n + 0.5);
    bool down = mod(beat, 4.0) == 0.0;
    if (dist < (down ? 1.5 : 0.75)) {
      float g = down ? 0.9 : 0.45;
      c = vec4(u_grid * g, g) + c * (1.0 - g);
    }
  }
  // Hot cues: a solid line each.
  for (int k = 0; k < 8; k++) {
    if (u_cues[k] >= 0.0 && abs(t - u_cues[k]) / u_spp < 1.0) c = vec4(u_cueColor, 1.0);
  }
  // The past is dimmed.
  if (x < u_size.x * 0.5) c *= 0.5;
  color = c;
}`;

export interface DetailFrame {
  /** Loop [start, end) in seconds. */
  loop: [number, number] | null;
  /** Hot cues in seconds (null = empty), up to 8. */
  cues: Array<number | null>;
  center: number;
  /** Track length in seconds (the grid stops there). */
  duration: number;
  secondsVisible: number;
  bpm: number;
  firstBeat: number;
}

export class DetailRenderer {
  private gl: WebGL2RenderingContext;
  private program: WebGLProgram;
  private textures: WebGLTexture[] = [];
  private peaks: Peaks | null = null;
  private loc: Record<string, WebGLUniformLocation | null> = {};
  private colors: WaveColors | null = null;
  private lost = false;
  private readonly cues = new Float32Array(8);
  private readonly onLost = (e: Event) => {
    e.preventDefault(); // ask for the context back
    this.lost = true;
  };
  private readonly onRestored = () => {
    try {
      this.init();
      this.lost = false;
      this.setPeaks(this.peaks);
      this.restored();
    } catch (e) {
      console.warn('waveform context not restored', e);
    }
  };

  /** Throws when WebGL2 is unavailable or the shader does not compile. `restored` is called
   *  after the GPU context came back (sleep, GPU switch) so the owner can redraw. */
  constructor(
    private canvas: HTMLCanvasElement,
    private restored: () => void = () => {},
  ) {
    const gl = canvas.getContext('webgl2', { premultipliedAlpha: true, antialias: false });
    if (!gl) throw new Error('WebGL2 nicht verfügbar');
    this.gl = gl;
    this.program = this.init();
    canvas.addEventListener('webglcontextlost', this.onLost);
    canvas.addEventListener('webglcontextrestored', this.onRestored);
  }

  /** Compile the program and look up its uniforms (again after a context loss). */
  private init(): WebGLProgram {
    const gl = this.gl;
    this.program = link(gl, VERTEX, FRAGMENT);
    this.textures = [];
    for (const name of [
      'u_peaks',
      'u_size',
      'u_center',
      'u_spp',
      'u_binRate',
      'u_bins',
      'u_low',
      'u_mid',
      'u_high',
      'u_grid',
      'u_bpm',
      'u_firstBeat',
      'u_duration',
      'u_loop',
      'u_cues',
      'u_loopColor',
      'u_cueColor',
    ]) {
      this.loc[name] = gl.getUniformLocation(this.program, name);
    }
    return this.program;
  }

  setColors(colors: WaveColors) {
    this.colors = colors;
  }

  /** Upload every level once; frames then only pick one. */
  setPeaks(peaks: Peaks | null) {
    const gl = this.gl;
    this.textures.forEach((t) => gl.deleteTexture(t));
    this.textures = [];
    this.peaks = peaks;
    if (!peaks || this.lost) return;
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
    for (const level of peaks.levels) {
      const bins = level.length / BANDS;
      const rows = Math.max(1, Math.ceil(bins / ROW));
      const data = new Uint8Array(ROW * rows * BANDS);
      data.set(level);
      const tex = gl.createTexture();
      gl.bindTexture(gl.TEXTURE_2D, tex);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGB8, ROW, rows, 0, gl.RGB, gl.UNSIGNED_BYTE, data);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
      this.textures.push(tex);
    }
  }

  draw(f: DetailFrame) {
    const { gl, canvas, peaks, colors } = this;
    if (this.lost) return;
    gl.viewport(0, 0, canvas.width, canvas.height);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    if (!peaks || !colors || canvas.width === 0) return;
    const spp = f.secondsVisible / canvas.width;
    const level = chooseLevel(peaks, spp, MAX_SPAN / 2);
    const tex = this.textures[level];
    if (!tex) return;
    gl.useProgram(this.program);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.uniform1i(this.loc.u_peaks ?? null, 0);
    gl.uniform2f(this.loc.u_size ?? null, canvas.width, canvas.height);
    gl.uniform1f(this.loc.u_center ?? null, f.center);
    gl.uniform1f(this.loc.u_spp ?? null, spp);
    gl.uniform1f(this.loc.u_binRate ?? null, binRate(peaks, level));
    gl.uniform1i(this.loc.u_bins ?? null, (peaks.levels[level]?.length ?? 0) / BANDS);
    const rgb = (c: [number, number, number]) => c.map((v) => v / 255) as [number, number, number];
    gl.uniform3f(this.loc.u_low ?? null, ...rgb(colors.low));
    gl.uniform3f(this.loc.u_mid ?? null, ...rgb(colors.mid));
    gl.uniform3f(this.loc.u_high ?? null, ...rgb(colors.high));
    gl.uniform3f(this.loc.u_grid ?? null, ...rgb(colors.grid));
    gl.uniform1f(this.loc.u_bpm ?? null, f.bpm > 0 ? f.bpm : 0);
    gl.uniform1f(this.loc.u_firstBeat ?? null, f.firstBeat);
    gl.uniform1f(this.loc.u_duration ?? null, f.duration);
    gl.uniform2f(this.loc.u_loop ?? null, f.loop?.[0] ?? -1, f.loop?.[1] ?? -1);
    // Reused every frame: no garbage at display rate.
    this.cues.fill(-1);
    f.cues.slice(0, 8).forEach((c, k) => (this.cues[k] = c ?? -1));
    gl.uniform1fv(this.loc.u_cues ?? null, this.cues);
    gl.uniform3f(this.loc.u_loopColor ?? null, ...rgb(colors.loop));
    gl.uniform3f(this.loc.u_cueColor ?? null, ...rgb(colors.cue));
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  }

  /** Free the GPU context right away: a deck change mounts a new renderer, and browsers
   *  only allow a handful of live contexts. */
  dispose() {
    this.canvas.removeEventListener('webglcontextlost', this.onLost);
    this.canvas.removeEventListener('webglcontextrestored', this.onRestored);
    this.textures.forEach((t) => this.gl.deleteTexture(t));
    this.gl.deleteProgram(this.program);
    this.gl.getExtension('WEBGL_lose_context')?.loseContext();
  }
}

function link(gl: WebGL2RenderingContext, vs: string, fs: string): WebGLProgram {
  const compile = (type: number, src: string) => {
    const s = gl.createShader(type);
    if (!s) throw new Error('Shader konnte nicht angelegt werden');
    gl.shaderSource(s, src);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) {
      throw new Error(gl.getShaderInfoLog(s) ?? 'Shader-Fehler');
    }
    return s;
  };
  const p = gl.createProgram();
  gl.attachShader(p, compile(gl.VERTEX_SHADER, vs));
  gl.attachShader(p, compile(gl.FRAGMENT_SHADER, fs));
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) {
    throw new Error(gl.getProgramInfoLog(p) ?? 'Shader-Fehler');
  }
  return p;
}
