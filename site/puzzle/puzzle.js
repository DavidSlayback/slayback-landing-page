// Out of the Box at /puzzle: loads puzzle.wasm (crates/puzzle-wasm), which holds the rules, the animation and the
// drawing; this file only maps keys, blits the frame Rust drew, and keeps the HUD. ?tier=1..4&seed=<n> picks the level;
// ?palette=netrunner|synthwave|gold overrides the site's colour scheme; ?touch=1 shows the touch pad on any device.
// window.__puzzle exposes the state for tests.
'use strict';
// The site's schemes (site/hero.js PALETTES): fg, muted, accent.
const SCHEMES = {
  netrunner: ['#efffd6', '#7d9a52', '#a6ff00'],
  synthwave: ['#ffe9fb', '#a0609a', '#ff2bd6'],
  gold: ['#fff1d0', '#a07a35', '#ffae2b'],
};
const KEYS = { ArrowLeft: 0, KeyA: 0, ArrowRight: 1, KeyD: 1, ArrowUp: 2, KeyW: 2, ArrowDown: 3, KeyS: 3 };
const REFUSALS = ['', 'Something is in the way.', 'The block you carry would hit something.', 'Nothing there to climb.',
  'No room overhead.', 'No block in front to lift.', 'Something is sitting on that block.', 'No room to set it down there.', ''];

const qs = new URLSearchParams(location.search);
const tier = Math.min(4, Math.max(1, parseInt(qs.get('tier'), 10) || 1));
const seed = Math.max(1, parseInt(qs.get('seed'), 10) || 1);
const stored = (() => { try { return localStorage.getItem('slayback-scheme'); } catch { return null; } })();
const scheme = SCHEMES[qs.get('palette')] || SCHEMES[stored] || SCHEMES.netrunner;
const reduceMotion = matchMedia('(prefers-reduced-motion: reduce)').matches;
// The touch pad shows on a coarse pointer (a phone or tablet), or with ?touch=1.
if (qs.has('touch') || matchMedia('(pointer: coarse)').matches) document.documentElement.classList.add('touch');

const $ = id => document.getElementById(id);
const main = document.querySelector('main'), canvas = $('board'), ctx = canvas.getContext('2d');
const overlay = $('overlay'), message = $('message');

const root = document.documentElement.style;
[['fg', 0], ['muted', 1], ['accent', 2]].forEach(([k, i]) => root.setProperty('--' + k, scheme[i]));
document.querySelectorAll('nav [data-tier]').forEach(a => a.setAttribute('aria-current', String(+a.dataset.tier === tier)));
$('where').textContent = `Tier ${tier} · level ${seed}`;

function say(html, state) {
  main.dataset.state = state;
  message.innerHTML = html;
  overlay.hidden = !html;
}

async function load() {
  const url = new URL('puzzle.wasm', document.currentScript?.src ?? location.href);
  try { return (await WebAssembly.instantiateStreaming(fetch(url), {})).instance.exports; }
  catch { return (await WebAssembly.instantiate(await (await fetch(url)).arrayBuffer(), {})).instance.exports; }
}

load().then(X => {
  // Let "Building the level…" paint first: the higher tiers take a moment to generate.
  requestAnimationFrame(() => setTimeout(() => start(X), 0));
}).catch(e => { say(`<span class="error">The puzzle needs WebAssembly: ${e.message}</span>`, 'error'); });

function start(X) {
  const g = X.game_new(tier, BigInt(seed));
  if (!g) return say(`<span class="error">No level for tier ${tier}, seed ${seed}.</span>`, 'error');
  scheme.forEach((hex, i) => {
    const n = parseInt(hex.slice(1), 16);
    X.game_color(g, i, (n >> 16 & 255) / 255, (n >> 8 & 255) / 255, (n & 255) / 255);
  });
  X.game_reduce_motion(g, reduceMotion ? 1 : 0);
  const w = X.game_width(g), h = X.game_height(g);
  canvas.width = w; canvas.height = h;
  root.setProperty('--ar', (w / h).toFixed(4));
  $('optimal').textContent = X.game_optimal(g);
  say('', 'playing');

  let solvedShown = false;
  function hud() {
    $('moves').textContent = X.game_moves(g);
    const solved = X.game_solved(g) === 1;
    if (solved && !solvedShown) {
      solvedShown = true;
      const moves = X.game_moves(g), best = X.game_optimal(g);
      const next = `?tier=${tier}&seed=${seed + 1}`;
      setTimeout(() => say(`<strong>Out!</strong><span>${moves} moves${moves === best ? ', the shortest way' : ` (shortest: ${best})`}</span>
        <span><a href="${next}" style="color:var(--accent)">Next level</a><span class="keys-only"> · <kbd>N</kbd> next, <kbd>R</kbd> again</span></span>`, 'solved'),
        reduceMotion ? 0 : 900);
    } else if (!solved && solvedShown) {
      solvedShown = false;
      say('', 'playing');
    }
  }

  // Keys and the touch pad both come through here: a key index (0 L, 1 R, 2 U, 3 D) or undo, restart, next.
  function act(a) {
    if (typeof a === 'number') {
      const r = X.game_key(g, a);
      $('where').textContent = (r && REFUSALS[r]) || `Tier ${tier} · level ${seed}`;
    } else if (a === 'undo') X.game_undo(g);
    else if (a === 'restart') X.game_restart(g);
    else if (a === 'next' && X.game_solved(g)) location.search = `?tier=${tier}&seed=${seed + 1}`;
    hud();
  }

  addEventListener('keydown', e => {
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    const a = e.code in KEYS ? KEYS[e.code]
      : { KeyZ: 'undo', Backspace: 'undo', KeyR: 'restart', KeyN: 'next' }[e.code];
    if (a === undefined) return;
    if (a !== 'restart' && a !== 'next') e.preventDefault();
    act(a);
  });

  // The touch pad (P19): a press acts at once; walking repeats while held, so a long walk isn't a dozen taps.
  for (const b of document.querySelectorAll('#pad [data-act]')) {
    const a = /^\d$/.test(b.dataset.act) ? +b.dataset.act : b.dataset.act;
    let timer = 0;
    const stop = () => { clearTimeout(timer); timer = 0; };
    b.addEventListener('pointerdown', e => {
      e.preventDefault();
      try { b.setPointerCapture(e.pointerId); } catch { /* a pointer the browser doesn't track; the press still acts */ }
      act(a);
      if (a === 0 || a === 1) {
        const again = delay => { timer = setTimeout(() => { act(a); again(140); }, delay); };
        again(380);
      }
    });
    for (const ev of ['pointerup', 'pointercancel', 'lostpointercapture']) b.addEventListener(ev, stop);
    b.addEventListener('contextmenu', e => e.preventDefault());
  }

  let last = performance.now();
  function frame(now) {
    const ptr = X.game_frame(g, Math.min(0.1, (now - last) / 1000));
    last = now;
    // Memory may have grown since the last frame, so take a fresh view every time.
    ctx.putImageData(new ImageData(new Uint8ClampedArray(X.memory.buffer, ptr, w * h * 4), w, h), 0, 0);
    requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);

  window.__puzzle = {
    fingerprint: () => '0x' + BigInt.asUintN(64, X.game_fingerprint(g)).toString(16).padStart(16, '0'),
    moves: () => X.game_moves(g),
    optimal: () => X.game_optimal(g),
    solved: () => X.game_solved(g) === 1,
  };
}
