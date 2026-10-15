// No render pipeline is built mid-game (decisions Q72, Q73). WebGL builds a shader program the first time a new kind of
// material or mesh layout is drawn, and blocks while it links: on ANGLE/D3D11 a Bevy PBR variant took ~3.4 s, which
// froze the first punch. Everything must be drawn once while the game loads (`camera::Warm`), so a fight's blows,
// bursts, kills and throws link nothing new. Chrome's GPU trace counts the links either side of the game's
// "warm-up drawn" log.
import { test, expect } from '@playwright/test';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { DIST } from './playwright.config.js';

test('no shader is linked once the warm-up is drawn', async ({ page, browser, browserName }) => {
  test.setTimeout(240_000);
  const built = existsSync(join(DIST, 'play', 'game_bg.wasm'));
  if (process.env.REQUIRE_GAME) expect(built).toBe(true);
  test.skip(!built, 'game not in this build');
  test.skip(browserName !== 'chromium', 'GPU tracing is Chromium-only');

  const beats = [];
  let marked = false;
  page.on('console', async m => {
    const text = m.text();
    if (text.includes('warm-up drawn') && !marked) {
      marked = true;
      await page.evaluate(() => console.timeStamp('fight')).catch(() => {});
    }
    const beat = text.match(/demo: ([\w-]+)/);
    if (beat) beats.push({ beat: beat[1], marked });
  });
  await browser.startTracing(page, { categories: ['gpu', 'devtools.timeline'] });
  // In slow motion: at software WebGL's frame rate each early frame advances the game a lot (the clock caps a frame at
  // 0.25 s), and at full speed the demo's first punch landed within the warm-up's frames.
  await page.goto('/play/?demo=1&slow=0.15');
  // The demo's scripted fight: a punch, a kick, a drop-kick and a spin-throw, each a kill.
  await expect.poll(() => beats.some(b => b.beat === 'spin-throw'), { timeout: 200_000 }).toBe(true);
  const trace = JSON.parse((await browser.stopTracing()).toString());
  const events = trace.traceEvents ?? trace;
  const mark = events.find(e => e.name === 'TimeStamp' && e.args?.data?.message === 'fight');
  expect(mark, 'the warm-up was marked in the trace').toBeTruthy();
  expect(beats[0]?.marked, `the first blow came after the warm-up: ${JSON.stringify(beats)}`).toBe(true);
  const links = events.filter(e => typeof e.name === 'string' && e.name.endsWith('DoLinkProgram'));
  const before = links.filter(e => e.ts < mark.ts).length;
  const after = links.filter(e => e.ts >= mark.ts).length;
  expect(before, 'the trace sees the load linking its shaders (else this check proves nothing)').toBeGreaterThan(0);
  expect(after, 'shader programs linked during the fight: draw them once at load (camera::Warm)').toBe(0);
});
