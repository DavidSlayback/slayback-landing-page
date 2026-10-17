// Landing-page hero: welding-spark currents around dim lettering that only their light brings up.
//
// Physics runs in Rust (crates/sim, exported by crates/hero-wasm as hero.wasm). This file owns the browser side:
// rasterising the hero's DOM text into the obstacle mask, streaming particle state out of WASM memory into
// WebGL2, and compositing. Text is drawn last as an opaque, haloed surface so the sparks can never wash it out: dim,
// neutral ink, just above barely readable, that the sparks' light map tints and brightens where they pass.
// Debug/test hooks: ?hud=1 shows timings; ?capture=1 disables the loop and exposes window.__capture.frame(dt)
// for frame-exact screenshots; ?warm=<frames> pre-runs the physics; ?n=<sparks> overrides the spark count;
// ?ink=<0..1> and ?lit=<gain> override the lettering's unlit grey and how strongly the sparks light it;
// ?gravity=name|down picks the centre of gravity (G switches it); ?currents=<gain> sets the current layer's brightness.
'use strict';
// world_set keys (sim/src/lib.rs `World::set`).
const K = { gravity: 0, wind: 1, turb: 2, drag: 3, radius: 4, friction: 5, floorFriction: 6, restitution: 7, shieldRestitution: 8,
  cursorR: 9, spawnRate: 10, source: 11, collide: 12, lifeMin: 13, lifeMax: 14, openEdges: 15, emitters: 16, launchMin: 17, launchMax: 18,
  openFloor: 19, wander: 20, flowScale: 21, inward: 22, topBottom: 23, pull: 24, eddies: 25 };
const SPARK_BASE = { radius: 1.6, friction: 0.12, floorFriction: 0.05, shieldRestitution: 0.85, cursorR: 12, collide: 0, openEdges: 1, wind: 0 };
// Sparks drift in from off-screen torches, then meander: tight currents, strong coupling to them, and per-spark wander.
// The torches sit two above the screen and two below, each jumping along its edge, so the lettering is lit from
// above and below fairly steadily (David, 2026-10-15: "from top and/or bottom so that I can rely on semi-consistent
// lighting").
// They ride the currents loosely (drag), so their launch and gravity still show, and three wandering eddies stir the
// currents (David, 2026-10-17: "random currents ... the sparks being influenced somewhat by them in addition to the
// initial force and gravity").
const PRESET = { ...SPARK_BASE, turb: 170, drag: 0.55, wander: 220, flowScale: 240, inward: 160,
  restitution: 0.5, lifeMin: 3.5, lifeMax: 8.0, emitters: 4, launchMin: 180, launchMax: 480, openFloor: 1, topBottom: 1,
  eddies: 3 };
// The centre of gravity (David: "Let's try 1 and 2"): toward the name's centre, or plain gravity down. ?gravity=name|down,
// or G switches; the choice is kept.
const GRAVITY = { name: { pull: 140, gravity: 0 }, down: { pull: 0, gravity: 220 } };
const storedGravity = (() => { try { return localStorage.getItem('slayback-gravity'); } catch { return null; } })();
const PALETTES = [
  { key: 'netrunner', label: 'Netrunner', swatch: '#a6ff00', font: '"Chakra Petch"', weight: 700, track: '.04em',
    css: { fg: '#efffd6', muted: '#7d9a52', accent: '#a6ff00' },
    gl: { uP0: [0.62, 1.0, 0.0], uP1: [0.15, 1.0, 0.2], uP2: [0.0, 1.0, 0.72], uP3: [0.88, 1.0, 0.12], uShieldCol: [0.62, 1.0, 0.05] } },
  { key: 'synthwave', label: 'Synthwave', swatch: '#ff2bd6', font: '"Orbitron"', weight: 900, track: '.06em',
    css: { fg: '#ffe9fb', muted: '#a0609a', accent: '#ff2bd6' },
    gl: { uP0: [1.0, 0.08, 0.72], uP1: [0.0, 0.9, 1.0], uP2: [1.0, 0.92, 0.05], uP3: [0.5, 0.15, 1.0], uShieldCol: [1.0, 0.18, 0.8] } },
  { key: 'gold', label: 'Human Revolution', swatch: '#ffae2b', font: '"Rajdhani"', weight: 700, track: '.16em',
    css: { fg: '#fff1d0', muted: '#a07a35', accent: '#ffae2b' },
    gl: { uP0: [1.0, 0.55, 0.04], uP1: [1.0, 0.82, 0.2], uP2: [1.0, 0.3, 0.0], uP3: [1.0, 0.95, 0.65], uShieldCol: [1.0, 0.64, 0.12] } },
];
const qs = new URLSearchParams(location.search);
let gravity = [qs.get('gravity'), storedGravity].find(g => g in GRAVITY) || 'name';
// Scheme: URL, then the visitor's saved pick, then a random one per visit.
const storedScheme = (() => { try { return localStorage.getItem('slayback-scheme'); } catch { return null; } })();
let pi = PALETTES.findIndex(p => p.key === (qs.get('palette') || storedScheme));
if (pi < 0) pi = Math.floor(Math.random() * PALETTES.length);
const compact = Math.min(innerWidth, innerHeight) < 600;
const CAP = parseInt(qs.get('n'), 10) || (compact ? 9000 : 18000);
// The lettering: its grey with no spark near (just above barely readable on the dark), and the gain on the sparks' light.
const INK = parseFloat(qs.get('ink')) || 0.09, LIT = parseFloat(qs.get('lit')) || 8;
const SHIELD_R = 40;   // forcefield radius, px; it exists only while the pointer is held
const reduceMotion = matchMedia('(prefers-reduced-motion: reduce)').matches;
const capture = qs.has('capture');
const FONT = '"Segoe UI",system-ui,-apple-system,Arial,sans-serif';
const DPR = Math.min(window.devicePixelRatio || 1, 2);
const scriptSrc = document.currentScript?.src;
const hero = document.getElementById('hero'), canvas = document.getElementById('c'), hud = document.getElementById('hud');
const tagline = document.getElementById('tagline');

// Without WebGL2 or WASM the hero degrades to its plain DOM text (html.gl is never set).
function fail(msg) { canvas.hidden = true; document.documentElement.classList.remove('gl'); throw new Error(`hero disabled: ${msg}`); }
const gl = canvas.getContext('webgl2', { antialias: false, alpha: false, depth: false, stencil: false, powerPreference: 'high-performance' });
if (!gl) fail('WebGL2 unavailable');
if (!gl.getExtension('EXT_color_buffer_float')) fail('EXT_color_buffer_float unavailable');
const emptyVao = gl.createVertexArray();
gl.bindVertexArray(emptyVao);

// ---------- GLSL ----------
const HEAD = '#version 300 es\nprecision highp float;\nprecision highp int;\nprecision highp sampler2D;\n';
const PAL = `uniform vec3 uP0, uP1, uP2, uP3;
vec3 pal(float t){ t=fract(t)*4.0;
  if(t<1.0) return mix(uP0,uP1,t); if(t<2.0) return mix(uP1,uP2,t-1.0); if(t<3.0) return mix(uP2,uP3,t-2.0); return mix(uP3,uP0,t-3.0); }
float hash12(vec2 p){vec3 p3=fract(vec3(p.xyx)*.1031);p3+=dot(p3,p3.yzx+33.33);return fract((p3.x+p3.y)*p3.z);}`;
const VS_TRI = HEAD + `void main(){ vec2 p=vec2((gl_VertexID<<1)&2, gl_VertexID&2); gl_Position=vec4(p*2.0-1.0,0.0,1.0); }`;
const CLIP = `uniform vec2 uRes;
vec4 clip(vec2 p){ return vec4(p.x/uRes.x*2.0-1.0, 1.0-p.y/uRes.y*2.0, 0.0, 1.0); }
vec2 corner(){ return vec2(float(gl_VertexID&1), float(gl_VertexID>>1))*2.0-1.0; }`;

// Sparks: thin streaks along their velocity; white-hot when fresh or on impact, cooling to the palette, then dark.
// Each spark's grain (from the sim, fixed for its life) varies its width and streak length a little, so no two match.
const VS_SPARK = HEAD + PAL + CLIP + `
layout(location=0) in vec4 aPV; layout(location=1) in vec4 aEHT;   // x, y, vx, vy · energy, hue, heat, grain
uniform float uTime, uWidth, uStreak, uEmit, uGain; out vec2 vL; out float vHalf, vW; out vec3 vCol;
void main(){ vec2 c=corner(); float sp=length(aPV.zw), e=aEHT.x, heat=aEHT.z;
  vec2 dir=sp>1.0?aPV.zw/sp:vec2(1.0,0.0), nrm=vec2(-dir.y,dir.x);
  float g=aEHT.w, g2=fract(g*7.31+0.37);                  // two independent draws from one grain
  float w=uWidth*(0.75+0.55*g)*(1.0+e*1.2), hl=min(sp*uStreak*(0.6+0.85*g2),56.0)*0.5;   // width x0.75-1.3, length x0.6-1.45
  vec2 local=vec2(c.x*(hl+w), c.y*w); vL=local; vHalf=hl; vW=w;
  float flicker=0.72+0.28*sin(uTime*57.0+hash12(aPV.xy*0.37)*40.0);
  vec3 base=pal(aEHT.y);
  float h3=heat*heat*heat;
  vec3 col=mix(base, vec3(1.0,0.97,0.9), clamp(h3*h3*0.7+e*0.5,0.0,0.9)) * (0.08+1.25*heat+1.6*e) * flicker;   // white only at birth; glow fades linearly over the life
  vCol = uEmit>0.0 ? base*(0.05+0.45*heat+e)*uEmit : col*uGain;
  gl_Position=clip(aPV.xy-dir*hl+dir*local.x+nrm*local.y); }`;
const FS_SPARK = HEAD + `in vec2 vL; in float vHalf, vW; in vec3 vCol; out vec4 o;
void main(){ vec2 q=vec2(max(abs(vL.x)-vHalf,0.0), vL.y); float d2=dot(q,q)/(vW*vW); if(d2>1.0) discard;
  float along=vHalf>0.01?clamp((vL.x+vHalf)/(2.0*vHalf),0.0,1.0):1.0;
  o=vec4(vCol*exp(-d2*2.5)*(0.15+0.85*along*along),1.0); }`;

// Torch heads: a flickering arc glow where each emitter sits.
const VS_TORCH = HEAD + PAL + CLIP + `
uniform vec4 uTorch[8]; uniform float uScale; out vec2 vQ; out vec3 vCol;
void main(){ vec4 t=uTorch[gl_InstanceID]; vec2 c=corner(); vQ=c;
  vCol=mix(pal(t.w),vec3(1.0),0.55)*t.z*uScale; gl_Position=clip(t.xy+c*28.0); }`;
const FS_TORCH = HEAD + `in vec2 vQ; in vec3 vCol; out vec4 o;
void main(){ float d2=dot(vQ,vQ); if(d2>1.0) discard; o=vec4(vCol*(exp(-d2*14.0)*1.2+exp(-d2*3.5)*0.25),1.0); }`;

// Final: tone-map sparks + bloom, then the text as an opaque surface on top (dark halo; dim ink the sparks light),
// then the forcefield.
const FS_FINAL = HEAD + `
uniform sampler2D uScene, uB1, uB2, uB3, uLight, uText, uSdf, uCur;
uniform vec2 uCanvas, uSdfTexel, uRes; uniform float uBloom, uTime, uCurGain;
uniform vec3 uShieldCol; uniform float uInk, uLit; uniform vec4 uShield;   // x, y, r (CSS px), hit flash
out vec4 o;
float hash12(vec2 p){vec3 p3=fract(vec3(p.xyx)*.1031);p3+=dot(p3,p3.yzx+33.33);return fract((p3.x+p3.y)*p3.z);}
float lum(vec3 c){ return dot(c,vec3(0.3,0.5,0.2)); }
float hexEdge(vec2 p){ const vec2 r=vec2(1.0,1.7320508); vec2 h=r*0.5; vec2 a=mod(p,r)-h, b=mod(p-h,r)-h; vec2 g=dot(a,a)<dot(b,b)?a:b;
  g=abs(g); return smoothstep(0.42,0.5,max(dot(g,vec2(0.5,0.8660254)),g.x)); }
void main(){
  vec2 uv=gl_FragCoord.xy/uCanvas, tuv=vec2(uv.x,1.0-uv.y), px=tuv*uRes;
  vec3 L=texture(uLight,uv).rgb;
  vec3 b=texture(uB1,uv).rgb*0.6+texture(uB2,uv).rgb*0.7+texture(uB3,uv).rgb*0.55;
  vec3 col=1.0-exp(-(texture(uScene,uv).rgb+b*uBloom+L*0.1+texture(uCur,uv).rgb*uCurGain));   // currents faintly under it all
  // text: an opaque surface the sparks pass behind, in dim ink of no scheme colour, lit only by the sparks' own light
  vec4 t=texture(uText,tuv); float s=texture(uSdf,tuv).r;
  float halo=(1.0-smoothstep(0.0,16.0,s))*(1.0-t.r);
  col*=1.0-0.82*halo;
  vec3 lit=1.0-exp(-L*uLit);                                    // the light falling here, in the sparks' colours
  vec2 g=vec2(texture(uSdf,tuv+vec2(uSdfTexel.x,0.0)).r-texture(uSdf,tuv-vec2(uSdfTexel.x,0.0)).r,
              texture(uSdf,tuv+vec2(0.0,uSdfTexel.y)).r-texture(uSdf,tuv-vec2(0.0,uSdfTexel.y)).r);
  vec2 n2=dot(g,g)>1e-8?normalize(g):vec2(0.0);                // outward from the glyph, text space (y down)
  vec2 dl=12.0/uRes;                                           // which way the light comes from, text space too
  vec2 gl2=vec2(lum(texture(uLight,uv+vec2(dl.x,0.0)).rgb)-lum(texture(uLight,uv-vec2(dl.x,0.0)).rgb),
                lum(texture(uLight,uv-vec2(0.0,dl.y)).rgb)-lum(texture(uLight,uv+vec2(0.0,dl.y)).rgb));
  vec2 toLight=dot(gl2,gl2)>1e-10?normalize(gl2):vec2(0.0);
  float bevel=1.0-smoothstep(0.0,4.0,-s);
  float rim=max(dot(n2,toLight),0.0)*bevel;                    // the glyph edges facing the sparks catch the most
  vec3 face=vec3(uInk)+lit*(0.65+0.7*rim);
  col=mix(col, min(face,vec3(0.95)), t.r);
  col=mix(col, min(vec3(uInk*1.3)+lit*1.1,vec3(0.95)), t.g*0.85);
  // forcefield: a hex shell that flares where sparks hit it
  if(uShield.z>0.5){
    float d=length(px-uShield.xy), r=uShield.z;
    float shell=exp(-pow((d-r)/1.6,2.0))*(0.55+1.4*uShield.w);
    float inner=step(d,r)*hexEdge((px-uShield.xy)/9.0)*smoothstep(r*0.35,r,d)*(0.12+0.5*uShield.w);
    col+=uShieldCol*(shell+inner);
  }
  vec2 q=uv-0.5; col*=1.0-0.35*dot(q,q);
  col+=(hash12(gl_FragCoord.xy+fract(uTime)*311.0)-0.5)*0.008;
  o=vec4(max(col,0.0),1.0);
}`;
const FS_FADE = HEAD + `uniform float uKeep; out vec4 o; void main(){ o=vec4(0.0,0.0,0.0,1.0-uKeep); }`;

// The current layer (David, 2026-10-17: "a 2 layer visualization system where one layer is back to something like the
// flow particles we initially prototyped"): tracers on the GPU, one per texel of a state texture (x, y, life, speed),
// carried by the sim's own current field (uFlow, the cells the sparks feel), drawn as faint dots into long trails.
// A tracer that runs out of life, leaves the screen or wanders into a letter is born again somewhere at random.
const FS_CUR_SIM = HEAD + `
uniform sampler2D uState, uFlow, uSdf; uniform vec2 uRes, uFlowDims; uniform float uCell, uDt, uSeed; out vec4 o;
float hash12(vec2 p){vec3 p3=fract(vec3(p.xyx)*.1031);p3+=dot(p3,p3.yzx+33.33);return fract((p3.x+p3.y)*p3.z);}
vec2 flow(vec2 p){ return texture(uFlow,(p/uCell+0.5)/uFlowDims).rg; }
void main(){
  ivec2 c=ivec2(gl_FragCoord.xy); vec4 s=texelFetch(uState,c,0);
  vec2 p=s.xy; vec2 v=flow(p); p+=flow(p+v*uDt*0.5)*uDt;   // midpoint step: smoother curves
  float life=s.z-uDt*(length(v)<6.0?6.0:1.0);   // stalled in a still pocket (by the letters): gone soon, not piling up
  if(life<=0.0||p.x<-20.0||p.x>uRes.x+20.0||p.y<-20.0||p.y>uRes.y+20.0||texture(uSdf,p/uRes).r<0.0){
    vec2 q=vec2(c)+uSeed*vec2(1013.0,727.0);
    p=vec2(hash12(q),hash12(q+19.19))*uRes; life=3.0+6.0*hash12(q+47.7);
  }
  o=vec4(p,life,length(v));
}`;
const VS_CUR_DRAW = HEAD + PAL + `
uniform sampler2D uState; uniform int uSide; uniform vec2 uRes; uniform float uSize, uGain; out vec3 vCol;
void main(){
  ivec2 c=ivec2(gl_VertexID%uSide, gl_VertexID/uSide); vec4 s=texelFetch(uState,c,0);
  float fade=smoothstep(0.0,1.0,s.z)*smoothstep(0.0,0.6,s.z);   // fade out as its life ends
  vCol=pal(fract(float(gl_VertexID)*0.6180339))*uGain*fade*(0.6+0.6*smoothstep(40.0,260.0,s.w));
  gl_Position=vec4(s.x/uRes.x*2.0-1.0, 1.0-s.y/uRes.y*2.0, 0.0, 1.0); gl_PointSize=uSize;
}`;
const FS_POINT = HEAD + `in vec3 vCol; out vec4 o; void main(){ o=vec4(vCol,1.0); }`;
const FS_DOWN = HEAD + `uniform sampler2D uSrc; uniform vec2 uDst, uTexel; out vec4 o;
void main(){ vec2 uv=gl_FragCoord.xy/uDst;
  o=vec4((texture(uSrc,uv+uTexel*vec2(-1,-1)).rgb+texture(uSrc,uv+uTexel*vec2(1,-1)).rgb
        +texture(uSrc,uv+uTexel*vec2(-1,1)).rgb+texture(uSrc,uv+uTexel*vec2(1,1)).rgb)*0.25,1.0); }`;
const FS_BLUR = HEAD + `uniform sampler2D uSrc; uniform vec2 uDst, uDir; out vec4 o;
void main(){ vec2 uv=gl_FragCoord.xy/uDst;
  vec3 c=texture(uSrc,uv).rgb*0.2270270;
  c+=(texture(uSrc,uv+uDir*1.3846154).rgb+texture(uSrc,uv-uDir*1.3846154).rgb)*0.3162162;
  c+=(texture(uSrc,uv+uDir*3.2307692).rgb+texture(uSrc,uv-uDir*3.2307692).rgb)*0.0702703;
  o=vec4(c,1.0); }`;

function shader(type, src) {
  const s = gl.createShader(type); gl.shaderSource(s, src); gl.compileShader(s);
  if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) fail(gl.getShaderInfoLog(s));
  return s;
}
function program(vs, fs) {
  const p = gl.createProgram();
  gl.attachShader(p, shader(gl.VERTEX_SHADER, vs)); gl.attachShader(p, shader(gl.FRAGMENT_SHADER, fs));
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) fail(gl.getProgramInfoLog(p));
  const u = {};
  for (let i = 0, n = gl.getProgramParameter(p, gl.ACTIVE_UNIFORMS); i < n; i++) {
    const { name, type } = gl.getActiveUniform(p, i);
    u[name.replace(/\[0\]$/, '')] = { loc: gl.getUniformLocation(p, name), type };
  }
  return { p, u };
}
const PROG = {
  spark: program(VS_SPARK, FS_SPARK), torch: program(VS_TORCH, FS_TORCH), final: program(VS_TRI, FS_FINAL),
  curSim: program(VS_TRI, FS_CUR_SIM), curDraw: program(VS_CUR_DRAW, FS_POINT),
  down: program(VS_TRI, FS_DOWN), blur: program(VS_TRI, FS_BLUR), fade: program(VS_TRI, FS_FADE),
};
const SETTER = { [gl.FLOAT]: 'uniform1fv', [gl.FLOAT_VEC2]: 'uniform2fv', [gl.FLOAT_VEC3]: 'uniform3fv', [gl.FLOAT_VEC4]: 'uniform4fv', [gl.INT]: 'uniform1iv' };
function use(P, textures, uniforms) {
  gl.useProgram(P.p);
  let unit = 0;
  for (const [name, t] of Object.entries(textures)) {
    gl.activeTexture(gl.TEXTURE0 + unit); gl.bindTexture(gl.TEXTURE_2D, t);
    if (P.u[name]) gl.uniform1i(P.u[name].loc, unit);
    unit++;
  }
  for (const [name, v] of Object.entries(uniforms)) {
    const u = P.u[name]; if (!u || u.type === gl.SAMPLER_2D) continue;
    gl[SETTER[u.type]](u.loc, typeof v === 'number' ? [v] : v);
  }
}

// ---------- GL resources ----------
const owned = [];
function mkTex(w, h, ifmt, fmt, type, data, filter = gl.NEAREST, keep = false) {
  const t = gl.createTexture(); if (!keep) owned.push(['Texture', t]);
  gl.bindTexture(gl.TEXTURE_2D, t);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, filter);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, filter);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  if (data instanceof HTMLCanvasElement) gl.texImage2D(gl.TEXTURE_2D, 0, ifmt, fmt, type, data);
  else gl.texImage2D(gl.TEXTURE_2D, 0, ifmt, w, h, 0, fmt, type, data);
  return t;
}
function mkFbo(t) {
  const f = gl.createFramebuffer(); owned.push(['Framebuffer', f]);
  gl.bindFramebuffer(gl.FRAMEBUFFER, f); gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, t, 0);
  return f;
}
const hdr = (w, h) => { const t = mkTex(w, h, gl.RGBA16F, gl.RGBA, gl.HALF_FLOAT, null, gl.LINEAR); return { t, f: mkFbo(t), w, h }; };
function freeAll() { for (const [kind, o] of owned) gl['delete' + kind](o); owned.length = 0; }
function pass(fbo, w, h) { gl.bindFramebuffer(gl.FRAMEBUFFER, fbo); gl.viewport(0, 0, w, h); }
const tri = () => gl.drawArrays(gl.TRIANGLES, 0, 3);

// ---------- Physics (Rust → WASM) ----------
let X = null, world = 0;
async function loadPhysics() {
  const url = new URL('hero.wasm', scriptSrc || document.baseURI);
  try {
    // Streaming compile needs the server to send application/wasm; fall back to a buffered compile otherwise.
    try { X = (await WebAssembly.instantiateStreaming(fetch(url), {})).instance.exports; }
    catch { X = (await WebAssembly.instantiate(await (await fetch(url)).arrayBuffer(), {})).instance.exports; }
  } catch (e) { fail(`physics engine unavailable (${e.message})`); }
}
function applyPreset() {
  const p = { ...PRESET, ...GRAVITY[gravity], spawnRate: CAP / 2.5 };
  for (const [k, v] of Object.entries(p)) if (k in K) X.world_set(world, K[k], v);
}

// ---------- Text → obstacles ----------
// Every word of every [data-collide] element is drawn where the DOM laid it out: red = fill, green = outline.
function rasterizeText(scale, w, h, outline) {
  const c = document.createElement('canvas'); c.width = Math.max(1, w); c.height = Math.max(1, h);
  const x = c.getContext('2d', { willReadFrequently: true });
  x.fillStyle = '#000'; x.fillRect(0, 0, c.width, c.height); x.setTransform(scale, 0, 0, scale, 0, 0);
  const hr = hero.getBoundingClientRect();
  for (const el of hero.querySelectorAll('[data-collide]')) {
    const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
    for (let node; (node = walker.nextNode());) {
      const cs = getComputedStyle(node.parentElement), fs = parseFloat(cs.fontSize);
      x.font = `${cs.fontStyle} ${cs.fontWeight} ${cs.fontSize} ${cs.fontFamily}`;
      if ('letterSpacing' in x) x.letterSpacing = `${parseFloat(cs.letterSpacing) || 0}px`;
      const upper = cs.textTransform === 'uppercase';
      for (const m of node.data.matchAll(/\S+/g)) {
        const range = document.createRange(); range.setStart(node, m.index); range.setEnd(node, m.index + m[0].length);
        const r = range.getClientRects()[0]; if (!r) continue;
        const word = upper ? m[0].toUpperCase() : m[0], met = x.measureText(word);
        const asc = met.fontBoundingBoxAscent, desc = met.fontBoundingBoxDescent;
        const bx = r.left - hr.left, by = r.top - hr.top + (r.height - (asc + desc)) / 2 + asc;
        x.globalCompositeOperation = 'source-over'; x.fillStyle = 'rgb(255,0,0)'; x.fillText(word, bx, by);
        if (outline) { x.globalCompositeOperation = 'lighter'; x.strokeStyle = 'rgb(0,255,0)'; x.lineWidth = Math.max(1, fs * 0.03); x.strokeText(word, bx, by); }
      }
    }
  }
  return c;
}
let textTex = null, sdfTex = null, sdfSize = [1, 1], obstacleTimer = 0;
function updateObstacles() {
  if (!world) return;
  if (textTex) gl.deleteTexture(textTex);
  textTex = mkTex(0, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, rasterizeText(DPR, canvas.width, canvas.height, true), gl.LINEAR, true);
  const mw = Math.ceil(W * 0.5), mh = Math.ceil(H * 0.5), px = rasterizeText(0.5, mw, mh, false).getContext('2d').getImageData(0, 0, mw, mh).data;
  const maskPtr = X.world_mask_alloc(world, mw, mh, 0.5);   // may grow WASM memory: take the buffer view after
  const mask = new Uint8Array(X.memory.buffer, maskPtr, mw * mh);
  for (let i = 0; i < mw * mh; i++) mask[i] = px[i * 4];
  X.world_mask_commit(world);
  if (sdfTex) gl.deleteTexture(sdfTex);
  sdfTex = mkTex(mw, mh, gl.R16F, gl.RED, gl.FLOAT, new Float32Array(X.memory.buffer, X.world_sdf_ptr(world), mw * mh), gl.LINEAR, true);
  sdfSize = [mw, mh];
}
const scheduleObstacles = () => { clearTimeout(obstacleTimer); obstacleTimer = setTimeout(updateObstacles, 40); };
new MutationObserver(scheduleObstacles).observe(hero, { subtree: true, childList: true, characterData: true });

// ---------- Colour schemes (site feature: swatches in the nav) ----------
const swatches = document.querySelector('.schemes');
for (const [i, P] of PALETTES.entries()) {
  const b = document.createElement('button'); b.type = 'button'; b.title = P.label; b.setAttribute('aria-label', `${P.label} colour scheme`);
  b.style.setProperty('--sw', P.swatch); b.onclick = () => setScheme(i, true); swatches.append(b);
}
const pal = () => PALETTES[pi];
async function setScheme(i, remember) {
  pi = i;
  const P = pal(), root = document.documentElement.style;
  for (const [k, v] of Object.entries(P.css)) root.setProperty('--' + k, v);
  root.setProperty('--font', `${P.font}, ${FONT}`); root.setProperty('--weight', P.weight); root.setProperty('--track', P.track);
  swatches.querySelectorAll('button').forEach((b, k) => b.setAttribute('aria-pressed', String(k === i)));
  if (remember) { try { localStorage.setItem('slayback-scheme', P.key); } catch {} }
  await Promise.race([document.fonts.load(`${P.weight} 100px ${P.font}`).catch(() => {}), new Promise(r => setTimeout(r, 2500))]);
  scheduleObstacles();   // the new face reflows the text: rebuild obstacles, keep the simulation running
}

// ---------- Input: small deflector on hover, forcefield while pressed ----------
const mouse = { x: 0, y: 0, active: false };
const shield = { r: 0, target: 0, flash: 0 };
addEventListener('pointermove', e => {
  const r = hero.getBoundingClientRect();
  mouse.x = e.clientX - r.left; mouse.y = e.clientY - r.top;
  mouse.active = mouse.y >= 0 && mouse.y <= r.height && mouse.x >= 0 && mouse.x <= r.width;
}, { passive: true });
document.documentElement.addEventListener('pointerleave', () => { mouse.active = false; });
addEventListener('pointerdown', e => {
  if (e.target.closest('.schemes, a, button, [contenteditable]')) return;
  const r = hero.getBoundingClientRect(); if (e.clientY - r.top > r.height) return;
  mouse.x = e.clientX - r.left; mouse.y = e.clientY - r.top; mouse.active = true;
  shield.target = SHIELD_R;
});
const dropShield = () => { shield.target = 0; };
addEventListener('pointerup', dropShield);
addEventListener('pointercancel', dropShield);
function updateShield(dt) {
  shield.r += (shield.target - shield.r) * Math.min(1, dt * (shield.target > shield.r ? 18 : 30));
  if (shield.r < 0.5 && shield.target === 0) shield.r = 0;
  X.world_shield(world, mouse.x, mouse.y, shield.r > 4 ? shield.r : 0);
}
addEventListener('keydown', e => {
  if (e.target.closest('input, textarea, [contenteditable]')) return;
  if (e.key === 'p') setScheme((pi + 1) % PALETTES.length, true);
  if (e.key === 'g' && world) {
    gravity = gravity === 'name' ? 'down' : 'name';
    try { localStorage.setItem('slayback-gravity', gravity); } catch {}
    applyPreset();
  }
});

// ---------- Build / loop ----------
let W = 0, H = 0, scene, light, lightTmp, bloom, vbo, sparkVao, building = Promise.resolve();
// The current layer: tracer state ping-ponged between two float textures, their trails, and the field they ride.
const TRACERS = 256, CUR_GAIN = parseFloat(qs.get('currents')) || 0.5;   // 65,536 tracers; ?currents=<gain>
let cur, curTrail, flowTex, flowDims = [1, 1, 1];
const build = () => (building = building.then(buildNow));
async function buildNow() {
  await setScheme(pi, false);
  await document.fonts.ready;
  if (!X) await loadPhysics();
  freeAll();
  W = hero.clientWidth; H = hero.clientHeight;
  const cw = canvas.width = Math.round(W * DPR), ch = canvas.height = Math.round(H * DPR);
  scene = hdr(cw, ch); light = hdr(Math.ceil(cw / 4), Math.ceil(ch / 4)); lightTmp = hdr(light.w, light.h);
  bloom = [2, 4, 8].map(d => [hdr(Math.ceil(cw / d), Math.ceil(ch / d)), hdr(Math.ceil(cw / d), Math.ceil(ch / d))]);
  if (world) X.world_free(world);
  world = X.world_new(CAP, W, H, 1 + ((Math.random() * 1e9) | 0));
  // Tracers start scattered with staggered lives; their trails are half resolution (they're soft anyway).
  const seed = new Float32Array(TRACERS * TRACERS * 4);
  for (let i = 0; i < TRACERS * TRACERS; i++) seed.set([Math.random() * W, Math.random() * H, Math.random() * 9, 0], i * 4);
  cur = [0, 1].map(() => { const t = mkTex(TRACERS, TRACERS, gl.RGBA32F, gl.RGBA, gl.FLOAT, seed); return { t, f: mkFbo(t) }; });
  curTrail = hdr(Math.ceil(cw / 2), Math.ceil(ch / 2));
  flowDims = [0, 1, 2].map(k => X.world_flow_dims(world, k));
  flowTex = mkTex(flowDims[0], flowDims[1], gl.RG16F, gl.RG, gl.FLOAT, null, gl.LINEAR);
  if (!vbo) {
    vbo = gl.createBuffer(); sparkVao = gl.createVertexArray();
    gl.bindVertexArray(sparkVao); gl.bindBuffer(gl.ARRAY_BUFFER, vbo);
    gl.enableVertexAttribArray(0); gl.vertexAttribPointer(0, 4, gl.FLOAT, false, 32, 0); gl.vertexAttribDivisor(0, 1);
    gl.enableVertexAttribArray(1); gl.vertexAttribPointer(1, 4, gl.FLOAT, false, 32, 16); gl.vertexAttribDivisor(1, 1);
    gl.bindVertexArray(emptyVao);
  }
  gl.bindBuffer(gl.ARRAY_BUFFER, vbo); gl.bufferData(gl.ARRAY_BUFFER, CAP * 32, gl.DYNAMIC_DRAW);
  updateObstacles();   // obstacles first, so torches are placed clear of the text
  applyPreset();
}
let resizeTimer = 0;
addEventListener('resize', () => { clearTimeout(resizeTimer); resizeTimer = setTimeout(build, 150); });

function blurInto(a, tmp, spread) {
  use(PROG.blur, { uSrc: a.t }, { uDst: [tmp.w, tmp.h], uDir: [spread / a.w, 0] }); pass(tmp.f, tmp.w, tmp.h); tri();
  use(PROG.blur, { uSrc: tmp.t }, { uDst: [a.w, a.h], uDir: [0, spread / a.h] }); pass(a.f, a.w, a.h); tri();
}
function runBloom() {
  let src = scene;
  for (const [a, b] of bloom) {
    use(PROG.down, { uSrc: src.t }, { uDst: [a.w, a.h], uTexel: [1 / src.w, 1 / src.h] }); pass(a.f, a.w, a.h); tri();
    blurInto(a, b, 1.4);
    src = a;
  }
}
function drawSparksAndTorches(P, emit, torches) {
  gl.enable(gl.BLEND); gl.blendFunc(gl.ONE, gl.ONE);
  gl.bindVertexArray(sparkVao);
  use(PROG.spark, {}, { uRes: [W, H], uTime: simT, uWidth: emit ? 6.0 : 1.0, uStreak: 0.05, uEmit: emit, uGain: 0.55, ...P });
  gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, active);
  gl.bindVertexArray(emptyVao);
  if (torches.length) {
    use(PROG.torch, {}, { uRes: [W, H], uTorch: torches, uScale: emit ? 1.5 : 1.0, ...P });
    gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, torches.length / 4);
  }
  gl.disable(gl.BLEND);
}
let simT = 0, simMs = 0, active = 0;
function present(dt) {
  if (!world || !textTex) return;
  simT += dt;
  X.world_cursor(world, mouse.x, mouse.y, mouse.active && shield.r <= 4 ? 1 : 0);
  updateShield(dt);
  const t0 = performance.now(); X.world_step(world, dt); simMs = simMs * 0.9 + (performance.now() - t0) * 0.1;
  shield.flash = Math.max(shield.flash * Math.pow(0.02, dt), Math.min(1, X.world_shield_hits(world) / 30));
  active = X.world_active(world);
  gl.bindBuffer(gl.ARRAY_BUFFER, vbo); gl.bufferSubData(gl.ARRAY_BUFFER, 0, new Float32Array(X.memory.buffer, X.world_out_ptr(world), active * 8));
  const ne = X.world_emitter_count(world), em = new Float32Array(X.memory.buffer, X.world_emitters_ptr(world), ne * 8), torches = [];
  for (let i = 0; i < ne; i++) torches.push(em[i * 8], em[i * 8 + 1], em[i * 8 + 3], em[i * 8 + 4]);
  const P = pal().gl;
  // 1. light map: sparks and torches at quarter resolution, blurred wide
  pass(light.f, light.w, light.h); gl.clearColor(0, 0, 0, 1); gl.clear(gl.COLOR_BUFFER_BIT);
  drawSparksAndTorches(P, 1.0, torches);
  blurInto(light, lightTmp, 3.0); blurInto(light, lightTmp, 6.0);
  // 1b. currents: move the tracers through this frame's field, then draw them over their fading trails
  gl.bindTexture(gl.TEXTURE_2D, flowTex);
  gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, flowDims[0], flowDims[1], gl.RG, gl.FLOAT,
    new Float32Array(X.memory.buffer, X.world_flow_ptr(world), flowDims[0] * flowDims[1] * 2));
  use(PROG.curSim, { uState: cur[0].t, uFlow: flowTex, uSdf: sdfTex },
    { uRes: [W, H], uFlowDims: [flowDims[0], flowDims[1]], uCell: flowDims[2], uDt: dt, uSeed: Math.random() * 100 });
  pass(cur[1].f, TRACERS, TRACERS); tri();
  cur.reverse();
  pass(curTrail.f, curTrail.w, curTrail.h);
  gl.enable(gl.BLEND); gl.blendFunc(gl.ZERO, gl.ONE_MINUS_SRC_ALPHA);
  use(PROG.fade, {}, { uKeep: Math.pow(0.975, dt * 60) }); tri();   // long trails: streams, not specks
  gl.blendFunc(gl.ONE, gl.ONE);
  use(PROG.curDraw, { uState: cur[0].t }, { uSide: TRACERS, uRes: [W, H], uSize: Math.max(1, DPR * 0.75), uGain: 0.012, ...P });
  gl.drawArrays(gl.POINTS, 0, TRACERS * TRACERS);
  gl.disable(gl.BLEND);
  // 2. scene: sparks and torches only, over fading trails (text is composited on top later, outside the bloom)
  pass(scene.f, scene.w, scene.h);
  gl.enable(gl.BLEND); gl.blendFunc(gl.ZERO, gl.ONE_MINUS_SRC_ALPHA);   // dst *= keep: frame-rate independent trails
  use(PROG.fade, {}, { uKeep: Math.pow(0.86, dt * 60) }); tri(); gl.disable(gl.BLEND);
  drawSparksAndTorches(P, 0.0, torches);
  runBloom();
  pass(null, canvas.width, canvas.height);
  use(PROG.final, { uScene: scene.t, uB1: bloom[0][0].t, uB2: bloom[1][0].t, uB3: bloom[2][0].t, uLight: light.t, uText: textTex, uSdf: sdfTex, uCur: curTrail.t },
    { uCanvas: [canvas.width, canvas.height], uSdfTexel: [1 / sdfSize[0], 1 / sdfSize[1]], uRes: [W, H], uBloom: 0.7, uTime: simT, uInk: INK, uLit: LIT, uCurGain: CUR_GAIN,
      uShield: [mouse.x, mouse.y, shield.r, shield.flash], ...P });
  tri();
  document.documentElement.classList.add('gl');
}
let last = 0, frames = 0, fpsT = 0, fps = 0;
function frame(now) {
  requestAnimationFrame(frame);
  const t = now / 1000, dt = last ? Math.min(t - last, 1 / 30) : 1 / 60; last = t;
  frames++; if (t - fpsT > 0.5) { fps = frames / (t - fpsT); frames = 0; fpsT = t;
    hud.textContent = `Currents / ${pal().label} — ${active.toLocaleString()} sparks · physics ${simMs.toFixed(1)} ms (Rust/WASM) · ${fps.toFixed(0)} fps · gravity: ${gravity}\np: scheme · g: gravity`; }
  if (scrollY > H) return;
  present(dt);
}
if (capture || !qs.has('hud')) hud.hidden = true;   // debug readout: ?hud=1
build().then(() => {
  for (let i = 0, n = parseInt(qs.get('warm'), 10) || 0; i < n; i++) { X.world_step(world, 1 / 60); simT += 1 / 60; }  // physics only
  if (capture) window.__capture = { frame: present };
  else if (reduceMotion) {
    for (let i = 0; i < 120; i++) X.world_step(world, 1 / 60);
    present(1 / 60);
    hud.textContent = 'Reduced motion is on: showing a still frame. Click the hero to animate.';
    canvas.addEventListener('pointerdown', () => requestAnimationFrame(frame), { once: true });
  } else requestAnimationFrame(frame);
});
