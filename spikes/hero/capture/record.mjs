// Frame-exact recorder for spikes/hero/hero-prototype.html (?capture=1 mode).
// Setup: npm i playwright-core (uses the installed Chrome; no browser download). Then: bash encode.sh frames ../motion
// Usage: node record.mjs <outRoot>   → <outRoot>/<variant>-<palette>/f0000.png ...
import { chromium } from 'playwright-core';
import { mkdirSync, rmSync } from 'node:fs';

const PAGE = 'file:///C:/Users/david/repos/portfolio/spikes/hero/hero-prototype.html';
const CHROME = 'C:/Program Files/Google/Chrome/Application/chrome.exe';
const W = 960, H = 540, DT = 1 / 30;
const outRoot = process.argv[2];

// Each script maps frame index → pointer actions taken before that frame is simulated.
const lerp = (a, b, t) => a + (b - a) * t;
const SCRIPTS = {
  // Fly-in assembly, a cursor sweep through both lines, a click shockwave, a full scatter, reassembly.
  particles: { warm: 0, frames: 200, async act(page, f) {
    if (f >= 45 && f <= 105) {
      const t = (f - 45) / 60;
      await page.mouse.move(lerp(120, 840, t), 230 + 95 * Math.sin(t * Math.PI * 3));
    }
    if (f === 106) await page.mouse.move(W / 2, H + 50);  // leave the hero → repulsion off
    if (f === 112) { await page.mouse.move(W / 2, 290); await page.mouse.down(); await page.mouse.up(); await page.mouse.move(W / 2, H + 50); }
    if (f === 145) await page.evaluate(() => __capture.scatter());
  } },
  // Settled flow, a cursor stirring a vortex through the lower-left, then a click burst top-right.
  flow: { warm: 240, frames: 170, async act(page, f) {
    if (f >= 30 && f <= 115) {
      const a = (f - 30) / 85 * Math.PI * 3;
      await page.mouse.move(lerp(160, 520, (f - 30) / 85) + 70 * Math.cos(a), 430 + 55 * Math.sin(a));
    }
    if (f === 116) await page.mouse.move(W / 2, H + 50);
    if (f === 120) { await page.mouse.move(760, 110); await page.mouse.down(); await page.mouse.up(); await page.mouse.move(W / 2, H + 50); }
  } },
};

const browser = await chromium.launch({ executablePath: CHROME, args: ['--enable-unsafe-swiftshader', '--use-angle=swiftshader'] });
async function record(variant, palette) {
  const S = SCRIPTS[variant], dir = `${outRoot}/${variant}-${palette}`;
  rmSync(dir, { recursive: true, force: true }); mkdirSync(dir, { recursive: true });
  const page = await browser.newPage({ viewport: { width: W, height: H }, deviceScaleFactor: 1 });
  page.on('pageerror', e => console.error(variant, palette, e.message));
  await page.goto(`${PAGE}?variant=${variant}&palette=${palette}&capture=1&warm=${S.warm}`);
  await page.waitForFunction(() => window.__capture, null, { timeout: 120000 });
  for (let f = 0; f < S.frames; f++) {
    await S.act(page, f);
    await page.evaluate(dt => __capture.frame(dt), DT);
    await page.screenshot({ path: `${dir}/f${String(f).padStart(4, '0')}.png` });
    if (f % 25 === 0) console.log(variant, palette, f);
  }
  await page.close();
  console.log('done', variant, palette, S.frames);
}
await Promise.all(['netrunner', 'renaissance'].flatMap(p => ['particles', 'flow'].map(v => record(v, p))));
await browser.close();
