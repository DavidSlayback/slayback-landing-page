// Clip recorder for the touch-device page, site/play/watch/: a muted H.264 MP4 plus a poster, via ffmpeg.
// Two modes:
// - Frame-exact (default; ported from spike/hero's record4.mjs): loads PAGE with ?capture=1 and steps its
//   window.__capture.frame(dt) hook one fixed dt per frame (the hero has this hook).
// - Slow motion (SLOW set): for the game, which has no capture hook. PAGE should run in slow motion at the same factor
//   (e.g. /play/?demo=1&slow=0.2 with SLOW=0.2). It screenshots as fast as it can for SECONDS of game time, timestamps
//   each frame, and gives each frame its real gap x SLOW as its duration, so the clip plays at game speed however
//   slowly the screenshots come.
//
// Usage (from tests/e2e, after `npm ci` and a build):  node record-clip.mjs [outDir]
//   DIST    built site to serve (default ../../dist)        PAGE    path + query to record (default: the hero)
//   FRAMES  frame-exact: frames at 30 fps (default 150)     SLOW, SECONDS  slow-motion mode (SECONDS default 6)
//   CROP    w:h:x:y of the 960x540 capture to keep (e.g. frame the game's box)   outDir  default ../../site/play/watch
//   The game clip: SLOW=0.2 CROP=480:270:240:262 PAGE='/play/?demo=1&slow=0.2&palette=netrunner' node record-clip.mjs
import { chromium } from '@playwright/test';
import { spawn, execFileSync } from 'node:child_process';
import { mkdtempSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { DIST, GL_ARGS } from './playwright.config.js';

const here = fileURLToPath(new URL('.', import.meta.url));
const out = resolve(process.argv[2] ?? join(here, '../../site/play/watch'));
const PORT = 4179, W = 960, H = 540, DT = 1 / 30, FRAMES = Number(process.env.FRAMES ?? 150);
const PAGE = process.env.PAGE ?? '/?palette=netrunner&warm=240';
const SLOW = process.env.SLOW ? Number(process.env.SLOW) : null;
const SECONDS = Number(process.env.SECONDS ?? 6);
const CROP = process.env.CROP ? `crop=${process.env.CROP},` : '';
const lerp = (a, b, t) => a + (b - a) * t;

// Hold the forcefield and sweep it through the sparks, then let go.
async function act(page, f) {
  if (f === 30) { await page.mouse.move(250, 400); await page.mouse.down(); }
  if (f > 30 && f <= 110) await page.mouse.move(lerp(250, 710, (f - 30) / 80), 400 + 50 * Math.sin((f - 30) / 80 * Math.PI * 2));
  if (f === 111) { await page.mouse.up(); await page.mouse.move(W / 2, H + 60); }
}

const server = spawn(process.execPath, [join(here, 'serve.mjs'), DIST, String(PORT)], { stdio: 'inherit' });
const frames = mkdtempSync(join(tmpdir(), 'clip-'));
const channel = process.env.PW_CHANNEL ?? 'chrome';   // the installed Chrome, as in the harness
const browser = await chromium.launch({ ...(channel ? { channel } : {}), args: GL_ARGS });
try {
  const page = await browser.newPage({ viewport: { width: W, height: H }, deviceScaleFactor: 1 });
  page.on('pageerror', e => console.error(e.message));
  const sep = PAGE.includes('?') ? '&' : '?';
  const url = SLOW ? `http://127.0.0.1:${PORT}${PAGE}` : `http://127.0.0.1:${PORT}${PAGE}${sep}capture=1`;
  for (let i = 0; ; i++) {   // wait for the server to listen
    try { await page.goto(url); break; }
    catch (e) { if (i > 50) throw e; await page.waitForTimeout(100); }
  }
  if (SLOW) {
    // The game: wait until it runs and settles, then grab frames against the clock.
    await page.waitForFunction(() => document.getElementById('game')?.width > 300, null, { timeout: 120_000 });
    await page.waitForTimeout(1500);
    const stamps = [];
    const start = Date.now();
    while ((Date.now() - start) * SLOW < SECONDS * 1000) {
      await page.screenshot({ path: join(frames, `f${String(stamps.length).padStart(4, '0')}.png`) });
      stamps.push(Date.now());
    }
    const list = stamps.map((t, i) => `file 'f${String(i).padStart(4, '0')}.png'\nduration ${(((stamps[i + 1] ?? t + 33) - t) * SLOW / 1000).toFixed(4)}`);
    writeFileSync(join(frames, 'list.txt'), list.join('\n') + `\nfile 'f${String(stamps.length - 1).padStart(4, '0')}.png'\n`);
  } else {
    await page.waitForFunction(() => window.__capture, null, { timeout: 120_000 });
    for (let f = 0; f < FRAMES; f++) {
      await act(page, f);
      await page.evaluate(dt => window.__capture.frame(dt), DT);
      await page.screenshot({ path: join(frames, `f${String(f).padStart(4, '0')}.png`) });
    }
  }
} finally {
  await browser.close();
  server.kill();
}

// 640x360 H.264 (plays on every phone) keeps the clip small; faststart lets it play while downloading.
const input = SLOW ? ['-f', 'concat', '-safe', '0', '-i', join(frames, 'list.txt'), '-r', '30'] : ['-framerate', '30', '-i', join(frames, 'f%04d.png')];
const ff = args => execFileSync('ffmpeg', ['-loglevel', 'error', '-y', ...input, ...args]);
ff(['-vf', `${CROP}scale=640:-2:flags=lanczos`, '-c:v', 'libx264', '-preset', 'veryslow', '-crf', '27', '-pix_fmt', 'yuv420p',
  '-movflags', '+faststart', '-an', join(out, 'clip.mp4')]);
ff(['-vf', `select=eq(n\\,${FRAMES >> 1}),${CROP}scale=640:-2:flags=lanczos`, '-frames:v', '1', '-q:v', '5', join(out, 'poster.jpg')]);
rmSync(frames, { recursive: true, force: true });
for (const f of ['clip.mp4', 'poster.jpg']) console.log(`${f}: ${Math.round(statSync(join(out, f)).size / 1024)} KB`);
