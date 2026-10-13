// Frame-exact clip recorder (ported from spike/hero's record4.mjs) for the touch-device page, site/play/watch/.
// Loads a page with ?capture=1, steps its window.__capture.frame(dt) hook one fixed dt per frame, screenshots each
// frame, and encodes a muted H.264 MP4 plus a poster with ffmpeg. Wall-clock speed never shows in the output.
// The game has no capture hook yet, so the default page is the hero: a placeholder clip until the release ticket.
//
// Usage (from tests/e2e, after `npm ci` and a build):  node record-clip.mjs [outDir]
//   DIST    built site to serve (default ../../dist)        PAGE    path + query to record (default: the hero)
//   FRAMES  frames at 30 fps (default 150)                  outDir  default ../../site/play/watch
import { chromium } from '@playwright/test';
import { spawn, execFileSync } from 'node:child_process';
import { mkdtempSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { DIST, GL_ARGS } from './playwright.config.js';

const here = fileURLToPath(new URL('.', import.meta.url));
const out = resolve(process.argv[2] ?? join(here, '../../site/play/watch'));
const PORT = 4179, W = 960, H = 540, DT = 1 / 30, FRAMES = Number(process.env.FRAMES ?? 150);
const PAGE = process.env.PAGE ?? '/?palette=netrunner&warm=240';
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
  for (let i = 0; ; i++) {   // wait for the server to listen
    try { await page.goto(`http://127.0.0.1:${PORT}${PAGE}${sep}capture=1`); break; }
    catch (e) { if (i > 50) throw e; await page.waitForTimeout(100); }
  }
  await page.waitForFunction(() => window.__capture, null, { timeout: 120_000 });
  for (let f = 0; f < FRAMES; f++) {
    await act(page, f);
    await page.evaluate(dt => window.__capture.frame(dt), DT);
    await page.screenshot({ path: join(frames, `f${String(f).padStart(4, '0')}.png`) });
  }
} finally {
  await browser.close();
  server.kill();
}

// 640x360 H.264 (plays on every phone) keeps the clip small; faststart lets it play while downloading.
const ff = args => execFileSync('ffmpeg', ['-loglevel', 'error', '-y', '-framerate', '30', '-i', join(frames, 'f%04d.png'), ...args]);
ff(['-vf', 'scale=640:-2:flags=lanczos', '-c:v', 'libx264', '-preset', 'veryslow', '-crf', '27', '-pix_fmt', 'yuv420p',
  '-movflags', '+faststart', '-an', join(out, 'clip.mp4')]);
ff(['-vf', `select=eq(n\\,${FRAMES >> 1}),scale=640:-2:flags=lanczos`, '-frames:v', '1', '-q:v', '5', join(out, 'poster.jpg')]);
rmSync(frames, { recursive: true, force: true });
for (const f of ['clip.mp4', 'poster.jpg']) console.log(`${f}: ${Math.round(statSync(join(out, f)).size / 1024)} KB`);
