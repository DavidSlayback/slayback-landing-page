// Frame-exact recorder for hero4-prototype.html (?capture=1). Scripts the forcefield, live typing, and a hands-off run.
// Setup: npm i playwright-core (uses the installed Chrome). Usage: node record4.mjs <outRoot> [variant]
import { chromium } from 'playwright-core';
import { mkdirSync, rmSync } from 'node:fs';

const PAGE = 'file:///C:/Users/david/repos/portfolio/spikes/hero4/hero4-prototype.html';
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
  // Hold the forcefield and drag it through the spray, release, then type.
  { variant: 'currents', palette: 'netrunner', warm: 240, frames: 240, type: typer(150, ' + whatever you type'), async act(page, f) {
    if (f === 20) { await page.mouse.move(250, 330); await page.mouse.down(); }
    if (f > 20 && f <= 110) await page.mouse.move(lerp(250, 760, (f - 20) / 90), 330 + 110 * Math.sin((f - 20) / 90 * Math.PI * 2));
    if (f === 111) { await page.mouse.up(); }
    if (f === 140) await page.mouse.move(...OFF);
  } },
  { variant: 'forge', palette: 'gold', warm: 240, frames: 240, type: typer(20, ' + SPARKS', 3), async act(page, f) {
    if (f === 90) { await page.mouse.move(480, 420); await page.mouse.down(); }
    if (f > 90 && f <= 170) await page.mouse.move(480 + 220 * Math.sin((f - 90) / 80 * Math.PI * 2), 420 - 60 * Math.cos((f - 90) / 80 * Math.PI));
    if (f === 171) { await page.mouse.up(); }
    if (f === 200) await page.mouse.move(...OFF);
  } },
  // Hands off: just the wandering torches.
  { variant: 'currents', palette: 'synthwave', warm: 240, frames: 240, type: async () => {}, async act() {} },
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
await Promise.all(CLIPS.filter(c => !only || `${c.variant}-${c.palette}` === only).map(record));
await browser.close();
