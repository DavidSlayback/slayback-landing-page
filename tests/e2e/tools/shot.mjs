// Screenshots of a page at several viewport sizes, for looking at layout: the homepage, the write-ups, /puzzle.
// capture.mjs is the tool for the game's motion; this one is for still pages.
//   node tools/shot.mjs --page '/#games' --sizes 1280x800,390x844 --out <dir>
// --page   path to open, with an optional #id to scroll to (/)   --sizes  WxH viewports, comma-separated (1280x800,390x844)
// --full   the whole page instead of one screen                  --wait   ms to wait after load (300)
// --out    directory for <name>-<W>x<H>.jpg (a temp dir)         --dist, --swiftshader  see lib.mjs
// JPEG at 1x: the files stay small enough to read back, and the text stays legible at one screen.
import { mkdirSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { args, openGame } from './lib.mjs';

const a = args({ page: '/', sizes: '1280x800,390x844', full: false, wait: 300, out: '', dist: undefined, swiftshader: false });
const out = a.out ? resolve(a.out) : mkdtempSync(join(tmpdir(), 'shot-'));
mkdirSync(out, { recursive: true });
const sizes = String(a.sizes).split(',').map(s => s.split('x').map(Number));
const name = a.page.replace(/[#?].*$/, '').replace(/^\/|\/$/g, '').replace(/\//g, '-') || 'home';
const [width, height] = sizes[0];
const { page, close } = await openGame(a.page, { dist: a.dist, swiftshader: a.swiftshader, width, height });
try {
  for (const [w, h] of sizes) {
    await page.setViewportSize({ width: w, height: h });
    await page.reload();
    await page.evaluate(() => document.fonts.ready);
    // Lazy images load only near the viewport: scroll through first so a full-page shot has them all.
    if (a.full) await page.evaluate(async () => {
      for (let y = 0; y < document.body.scrollHeight; y += innerHeight) { scrollTo(0, y); await new Promise(r => setTimeout(r, 50)); }
      scrollTo(0, 0);
    });
    const hash = new URL(a.page, 'http://x').hash;
    if (hash) await page.locator(hash).scrollIntoViewIfNeeded();
    await page.waitForTimeout(a.wait);
    const path = join(out, `${name}-${w}x${h}.jpg`);
    await page.screenshot({ path, type: 'jpeg', quality: 80, fullPage: a.full === true });
    console.log(path);
  }
} finally {
  await close();
}
