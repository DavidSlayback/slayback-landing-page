// Visitor-facing smoke checks against the built site. Asserts what a visitor would notice (the page renders,
// text is readable, nothing errors), never pixel-exact shader output.
import { test, expect } from '@playwright/test';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { DIST } from './playwright.config.js';
import { HOME, collectErrors, hasGl, litFraction } from './helpers.js';

test.describe('homepage', () => {
  test('hero renders WebGL text and animates with no console errors', async ({ page }) => {
    const errors = collectErrors(page);
    await page.goto(HOME);
    await expect.poll(() => hasGl(page), { message: 'html.gl is set once the first frame is drawn' }).toBe(true);

    const canvas = page.locator('#c');
    await expect(canvas).toBeVisible();
    await expect(page.locator('.name')).toHaveText(/David\s*Slayback/i);
    await expect(page.locator('.tagline')).toHaveText(/\S/);

    // The name is drawn by WebGL (its DOM copy is transparent), so its box on the canvas must be lit.
    const nameBox = await page.locator('.name').boundingBox();
    expect(await litFraction(page, await page.screenshot({ clip: nameBox })), 'the name is lit on the canvas').toBeGreaterThan(0.02);

    // The rAF loop is running: frames 500 ms apart differ.
    const a = await canvas.screenshot();
    await page.waitForTimeout(500);
    const b = await canvas.screenshot();
    expect(a.equals(b), 'hero canvas changes between frames').toBe(false);

    expect(errors).toEqual([]);
  });
});

test.describe('reduced motion', () => {
  test.use({ reducedMotion: 'reduce' });

  test('shows a still frame', async ({ page }) => {
    const errors = collectErrors(page);
    await page.goto(HOME);
    await expect.poll(() => hasGl(page)).toBe(true);
    const canvas = page.locator('#c');
    const a = await canvas.screenshot();
    await page.waitForTimeout(500);
    const b = await canvas.screenshot();
    expect(a.equals(b), 'canvas pixels identical 500 ms apart').toBe(true);
    expect(await litFraction(page, a), 'the still frame is not blank').toBeGreaterThan(0.01);
    expect(errors).toEqual([]);
  });
});

test.describe('/play', () => {
  test('game loads with no console errors', async ({ page }) => {
    test.setTimeout(180_000);
    const built = existsSync(join(DIST, 'play', 'game_bg.wasm'));
    if (process.env.REQUIRE_GAME) expect(built, `REQUIRE_GAME is set but ${DIST}/play/game_bg.wasm is missing`).toBe(true);
    test.skip(!built, 'game not in this build (dist/play/game_bg.wasm missing; SKIP_GAME=1 builds omit it)');

    const errors = collectErrors(page);
    await page.goto('/play/');
    // Bevy sizes the canvas to its parent once the app is running (an unstarted canvas is 300 px wide).
    await page.waitForFunction(() => document.getElementById('game').width !== 300, null, { timeout: 150_000 });
    await page.waitForTimeout(2000);
    // winit starts Bevy's loop by throwing out of init(); the page catches it, but Chrome may still log it.
    expect(errors.filter(e => !e.includes('Using exceptions for control flow'))).toEqual([]);
  });

  test('the start overlay shows, a click starts the game and Esc pauses it', async ({ page }) => {
    test.setTimeout(180_000);
    const built = existsSync(join(DIST, 'play', 'game_bg.wasm'));
    if (process.env.REQUIRE_GAME) expect(built).toBe(true);
    test.skip(!built, 'game not in this build');
    const errors = collectErrors(page);
    const logs = [];
    page.on('console', m => logs.push(m.text()));
    const after = (from, text) => () => logs.slice(from).some(l => l.includes(text));
    await page.goto('/play/');
    await expect.poll(after(0, 'phase start'), { timeout: 150_000 }).toBe(true);
    const canvas = page.locator('#game');
    await page.waitForTimeout(1000);
    // The overlay dims the box behind it (the controls' text is small): far fewer bright pixels until it starts. The
    // bar is 0.5: G4's pale muted arena still clears 0.35 under the overlay (55% both ways; at 0.5, 2% against 56%).
    const dimmed = await litFraction(page, await canvas.screenshot(), 0.5);
    let from = logs.length;
    await canvas.click();
    await expect.poll(after(from, 'phase playing')).toBe(true);
    await page.waitForTimeout(1000);
    const lit = await litFraction(page, await canvas.screenshot(), 0.5);
    expect(lit, `the overlay is gone (lit ${lit} vs ${dimmed} under it)`).toBeGreaterThan(dimmed * 3);
    expect(logs.slice(from).some(l => l.includes('phase paused')), 'the focused page keeps playing').toBe(false);
    from = logs.length;
    await page.keyboard.press('Escape');
    await expect.poll(after(from, 'phase paused')).toBe(true);
    expect(errors.filter(e => !e.includes('Using exceptions for control flow'))).toEqual([]);
  });

  test('a scripted run climbs out to box 2 with no console errors', async ({ page }) => {
    test.setTimeout(180_000);
    const built = existsSync(join(DIST, 'play', 'game_bg.wasm'));
    if (process.env.REQUIRE_GAME) expect(built).toBe(true);
    test.skip(!built, 'game not in this build');
    const errors = collectErrors(page);
    const logs = [];
    page.on('console', m => logs.push(m.text()));
    // The demo's stairs run: a debris staircase up past each rim, and the player walks out of box after box.
    await page.goto('/play/?demo=1&stairs=1');
    await expect.poll(() => logs.some(l => l.includes('box 2 started')), { timeout: 120_000 }).toBe(true);
    expect(errors.filter(e => !e.includes('Using exceptions for control flow'))).toEqual([]);
  });

  test.describe('with reduced motion', () => {
    test.use({ reducedMotion: 'reduce' });

    test('the game softens its flashes and sparks', async ({ page }) => {
      test.setTimeout(180_000);
      const built = existsSync(join(DIST, 'play', 'game_bg.wasm'));
      if (process.env.REQUIRE_GAME) expect(built).toBe(true);
      test.skip(!built, 'game not in this build');
      const logs = [];
      page.on('console', m => logs.push(m.text()));
      await page.goto('/play/');
      await page.waitForFunction(() => document.getElementById('game').width !== 300, null, { timeout: 150_000 });
      // Besides softer flashes and fewer bursts, the game drops the screen shake and the squash, and says so.
      await expect.poll(() => logs.some(l => l.includes('comfort: reduced motion on (no shake or squash'))).toBe(true);
    });
  });

  test('?motion=reduce turns reduced motion on without the media query', async ({ page }) => {
    test.setTimeout(180_000);
    const built = existsSync(join(DIST, 'play', 'game_bg.wasm'));
    if (process.env.REQUIRE_GAME) expect(built).toBe(true);
    test.skip(!built, 'game not in this build');
    const logs = [];
    page.on('console', m => logs.push(m.text()));
    await page.goto('/play/?motion=reduce');
    await page.waitForFunction(() => document.getElementById('game').width !== 300, null, { timeout: 150_000 });
    await expect.poll(() => logs.some(l => l.includes('comfort: reduced motion on'))).toBe(true);
  });

  test('the demo fight plays its beats under reduced motion with no console errors', async ({ page }) => {
    test.setTimeout(180_000);
    const built = existsSync(join(DIST, 'play', 'game_bg.wasm'));
    if (process.env.REQUIRE_GAME) expect(built).toBe(true);
    test.skip(!built, 'game not in this build');
    const errors = collectErrors(page);
    const logs = [];
    page.on('console', m => logs.push(m.text()));
    // The scripted fight by synthetic key presses: a punch, a kick, a drop-kick and a spin-throw, each a kill.
    await page.goto('/play/?demo=1&motion=reduce');
    for (const beat of ['demo: punch', 'demo: kick', 'demo: drop-kick', 'demo: spin-throw']) {
      await expect.poll(() => logs.some(l => l.includes(beat)), { timeout: 150_000 }).toBe(true);
    }
    expect(logs.some(l => l.includes('comfort: reduced motion on')), 'reduced motion from the query').toBe(true);
    expect(errors.filter(e => !e.includes('Using exceptions for control flow'))).toEqual([]);
  });
});
