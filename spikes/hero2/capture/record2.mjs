// Frame-exact recorder for spikes/hero2/hero2-prototype.html (?capture=1 mode).
// Setup: npm i playwright-core (uses the installed Chrome). Usage: node record2.mjs <outRoot> [variant]
import { chromium } from 'playwright-core';
import { mkdirSync, rmSync } from 'node:fs';

const PAGE = 'file:///C:/Users/david/repos/portfolio/spikes/hero2/hero2-prototype.html';
const CHROME = 'C:/Program Files/Google/Chrome/Application/chrome.exe';
const W = 960, H = 540, DT = 1 / 30, OFF = [W / 2, H + 60];
const outRoot = process.argv[2];
const lerp = (a, b, t) => a + (b - a) * t;

// Each clip: variant, scheme, pre-run frames, length, and pointer actions before frame f is simulated.
const CLIPS = [
  { variant: 'hypercube', palette: 'synthwave', warm: 0, frames: 240, async act(page, f) {
    if (f === 100) { await page.mouse.move(700, 200); await page.mouse.down(); }               // drag: rotate in 4D
    if (f > 100 && f <= 150) await page.mouse.move(700 - (f - 100) * 4, 200 + (f - 100) * 3);
    if (f === 151) { await page.mouse.up(); await page.mouse.move(...OFF); }
    if (f >= 170 && f <= 225) await page.mouse.move(lerp(300, 860, (f - 170) / 55), 420);   // gravity well sweep
    if (f === 226) await page.mouse.move(...OFF);
  } },
  { variant: 'sandfall', palette: 'netrunner', warm: 600, frames: 240, async act(page, f) {
    if (f >= 30 && f <= 110) await page.mouse.move(lerp(150, 810, (f - 30) / 80), 90 + 30 * Math.sin(f * 0.2));  // pour
    if (f === 111) await page.mouse.move(...OFF);
    if (f === 150) { await page.mouse.move(480, 470); await page.mouse.down(); await page.mouse.up(); await page.mouse.move(...OFF); }
  } },
  { variant: 'vortex', palette: 'gold', warm: 0, frames: 240, async act(page, f) {
    if (f >= 150 && f <= 195) await page.mouse.move(lerp(200, 760, (f - 150) / 45), 250);
    if (f === 196) await page.mouse.move(...OFF);
    if (f === 205) { await page.mouse.move(480, 240); await page.mouse.down(); await page.mouse.up(); await page.mouse.move(...OFF); }
  } },
];

const browser = await chromium.launch({ executablePath: CHROME, args: ['--enable-unsafe-swiftshader', '--use-angle=swiftshader'] });
async function record(C) {
  const dir = `${outRoot}/${C.variant}-${C.palette}`;
  rmSync(dir, { recursive: true, force: true }); mkdirSync(dir, { recursive: true });
  const page = await browser.newPage({ viewport: { width: W, height: H }, deviceScaleFactor: 1 });
  page.on('pageerror', e => console.error(C.variant, e.message));
  await page.goto(`${PAGE}?variant=${C.variant}&palette=${C.palette}&capture=1&warm=${C.warm}`);
  await page.waitForFunction(() => window.__capture, null, { timeout: 300000 });
  for (let f = 0; f < C.frames; f++) {
    await C.act(page, f);
    await page.evaluate(dt => __capture.frame(dt), DT);
    await page.screenshot({ path: `${dir}/f${String(f).padStart(4, '0')}.png` });
    if (f % 40 === 0) console.log(C.variant, f);
  }
  await page.close();
  console.log('done', C.variant);
}
const only = process.argv[3]; await Promise.all(CLIPS.filter(c => !only || c.variant === only).map(record));
await browser.close();
