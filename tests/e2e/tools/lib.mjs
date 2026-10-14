// Shared by the review tools: serve a build, open the game in Chrome on the real GPU, and read `--flag value` args.
// The harness (playwright.config.js) uses SwiftShader so it runs anywhere; these tools default to the GPU, because
// what they measure (frame times, shader links, the look) is what a visitor's machine does. `--swiftshader` opts out.
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { DIST, GL_ARGS } from '../playwright.config.js';

const here = fileURLToPath(new URL('.', import.meta.url));

/** `--name value` pairs (and bare `--flag`s as true), with defaults. */
export function args(defaults) {
  const out = { ...defaults };
  const argv = process.argv.slice(2);
  for (let i = 0; i < argv.length; i++) {
    const key = argv[i].replace(/^--/, '');
    const next = argv[i + 1];
    if (next === undefined || next.startsWith('--')) out[key] = true;
    else out[key] = typeof defaults[key] === 'number' ? Number(next) : next, i++;
  }
  return out;
}

const GPU_ARGS = ['--enable-gpu', '--ignore-gpu-blocklist', '--use-angle=d3d11'];

/** Serves `dist` (default: the harness's DIST), opens `path` and waits for the game's canvas. Returns what to close. */
export async function openGame(path, { dist = DIST, swiftshader = false, width = 1280, height = 720 } = {}) {
  const port = 4300 + Math.floor(Math.random() * 500);
  const server = spawn(process.execPath, [join(here, '..', 'serve.mjs'), dist, String(port)], { stdio: 'ignore' });
  const channel = process.env.PW_CHANNEL ?? 'chrome';
  const browser = await chromium.launch({ ...(channel ? { channel } : {}), args: swiftshader ? GL_ARGS : GPU_ARGS });
  const page = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: 1 });
  page.on('pageerror', e => console.error('page error:', e.message));
  for (let i = 0; ; i++) {
    try { await page.goto(`http://127.0.0.1:${port}${path}`); break; }
    catch (e) { if (i > 50) throw e; await page.waitForTimeout(100); }
  }
  if (path.startsWith('/play')) {
    await page.waitForFunction(() => document.getElementById('game')?.width > 300, null, { timeout: 150_000 });
  }
  const close = async () => { await browser.close(); server.kill(); };
  return { browser, page, close };
}
