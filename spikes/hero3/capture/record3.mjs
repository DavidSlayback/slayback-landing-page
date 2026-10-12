// Frame-exact recorder for hero3-prototype.html (?capture=1). Scripts the cursor, a click burst, and live typing.
// Setup: npm i playwright-core (uses the installed Chrome). Usage: node record3.mjs <outRoot> [variant]
import { chromium } from 'playwright-core';
import { mkdirSync, rmSync } from 'node:fs';

const PAGE = 'file:///C:/Users/david/repos/portfolio/spikes/hero3/hero3-prototype.html';
const W = 960, H = 540, DT = 1 / 30, OFF = [W / 2, H + 60];
const lerp = (a, b, t) => a + (b - a) * t;
const [outRoot, only] = process.argv.slice(2);

// Typing: click the tagline, jump to the end, then one character every `every` frames (the page rebuilds obstacles ~40 ms later).
function typer(start, text, every = 3) {
  return async (page, f) => {
    if (f === start) { await page.click('#tagline'); await page.keyboard.press('End'); await page.mouse.move(...OFF); }
    const k = (f - start - 1) / every;
    if (f > start && Number.isInteger(k) && k < text.length) { await page.keyboard.type(text[k]); await page.waitForTimeout(90); }
  };
}
const CLIPS = [
  { variant: 'dunes', palette: 'netrunner', warm: 300, frames: 240, type: typer(130, ' + whatever you type'), async act(page, f) {
    if (f >= 20 && f <= 90) await page.mouse.move(lerp(120, 840, (f - 20) / 70), 455 - 120 * Math.sin((f - 20) / 70 * Math.PI));
    if (f === 91) await page.mouse.move(...OFF);
    if (f === 105) { await page.mouse.move(470, 500); await page.mouse.down(); await page.mouse.up(); await page.mouse.move(...OFF); }
  } },
  { variant: 'currents', palette: 'synthwave', warm: 300, frames: 240, type: typer(130, ' + whatever you type'), async act(page, f) {
    if (f >= 20 && f <= 100) { const a = (f - 20) / 80 * Math.PI * 2; await page.mouse.move(480 + 300 * Math.cos(a), 200 + 90 * Math.sin(a)); }
    if (f === 101) await page.mouse.move(...OFF);
    if (f === 112) { await page.mouse.move(250, 120); await page.mouse.down(); await page.mouse.up(); await page.mouse.move(...OFF); }
  } },
  { variant: 'downpour', palette: 'gold', warm: 300, frames: 240, type: typer(15, ' + RAIN ON THIS', 2), async act(page, f) {
    if (f >= 110 && f <= 175) await page.mouse.move(lerp(100, 860, (f - 110) / 65), 470);
    if (f === 176) await page.mouse.move(...OFF);
    if (f === 195) { await page.mouse.move(700, 515); await page.mouse.down(); await page.mouse.up(); await page.mouse.move(...OFF); }
  } },
];

const browser = await chromium.launch({ executablePath: 'C:/Program Files/Google/Chrome/Application/chrome.exe', args: ['--enable-unsafe-swiftshader', '--use-angle=swiftshader'] });
async function record(C) {
  const dir = `${outRoot}/${C.variant}-${C.palette}`;
  rmSync(dir, { recursive: true, force: true }); mkdirSync(dir, { recursive: true });
  const page = await browser.newPage({ viewport: { width: W, height: H }, deviceScaleFactor: 1 });
  page.on('pageerror', e => console.error(C.variant, e.message));
  await page.goto(`${PAGE}?variant=${C.variant}&palette=${C.palette}&capture=1&warm=${C.warm}`);
  await page.waitForFunction(() => window.__capture, null, { timeout: 600000 });
  for (let f = 0; f < C.frames; f++) {
    await C.act(page, f); await C.type(page, f);
    await page.evaluate(dt => __capture.frame(dt), DT);
    await page.screenshot({ path: `${dir}/f${String(f).padStart(4, '0')}.png` });
    if (f % 40 === 0) console.log(C.variant, f);
  }
  await page.close();
  console.log('done', C.variant);
}
await Promise.all(CLIPS.filter(c => !only || c.variant === only).map(record));
await browser.close();
