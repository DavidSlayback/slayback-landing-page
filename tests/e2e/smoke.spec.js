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
      await expect.poll(() => logs.some(l => l.includes('comfort: reduced motion on'))).toBe(true);
    });
  });
});
