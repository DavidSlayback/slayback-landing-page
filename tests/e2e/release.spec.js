// The game's release switch (ticket C4). Set GAME_RELEASE=1 when the build under test was made with the switch on;
// otherwise the build must be the plain site, with no way into the game.
import { test, expect } from '@playwright/test';
import { HOME, collectErrors, hasGl, litFraction } from './helpers.js';

const ON = process.env.GAME_RELEASE === '1';
const KONAMI = ['ArrowUp', 'ArrowUp', 'ArrowDown', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'ArrowLeft', 'ArrowRight', 'b', 'a'];
const GAME_FILES = /\/play\/game(\.js|_bg\.wasm)$/;
const path = page => new URL(page.url()).pathname;
// Entry-point checks stop at the /play URL; smoke.spec.js covers the game itself.
const blockGame = page => page.route(GAME_FILES, r => r.abort());

test.describe('release switch off', () => {
  test.skip(ON, 'GAME_RELEASE=1: this build has the switch on');

  test('the homepage has no Play nav item, call to action or game link', async ({ page }) => {
    await page.goto(HOME);
    await expect(page.locator('.topnav a')).toHaveText(['Puzzle', 'GitHub']);
    await expect(page.locator('a[href*="play"]')).toHaveCount(0);
    expect((await page.request.get('/play/watch/')).status(), 'no clip page').toBe(404);
  });

  test('the easter egg does nothing', async ({ page }) => {
    await page.goto(HOME);
    for (const k of KONAMI) await page.keyboard.press(k);
    await page.waitForTimeout(500);
    expect(path(page)).toBe('/');
  });
});

test.describe('release switch on', () => {
  test.skip(!ON, 'set GAME_RELEASE=1 to test a build with the switch on');

  test('the Play nav item opens the game', async ({ page }) => {
    await blockGame(page);
    await page.goto(HOME);
    await page.getByRole('navigation', { name: 'Site' }).getByRole('link', { name: 'Play' }).click();
    await expect(page).toHaveURL(/\/play\/$/);
  });

  test('the hero call to action is drawn as hero text and opens the game', async ({ page }) => {
    const errors = collectErrors(page);
    await blockGame(page);
    await page.goto(HOME);
    await expect.poll(() => hasGl(page)).toBe(true);
    const cta = page.locator('.hero .cta[href="play/"]');
    await expect(cta).toHaveAttribute('data-collide', '');   // an obstacle: the sparks flow around it
    // Like the name, WebGL draws it (its DOM copy is transparent), so its box on the canvas must be lit.
    await page.waitForTimeout(300);   // obstacle rebuild after fonts settle
    expect(await litFraction(page, await page.screenshot({ clip: await cta.boundingBox() })), 'the CTA is lit').toBeGreaterThan(0.02);
    expect(errors).toEqual([]);
    await cta.click();
    await expect(page).toHaveURL(/\/play\/$/);
  });

  test('the Konami code opens the game', async ({ page }) => {
    await blockGame(page);
    await page.goto(HOME);
    for (const k of KONAMI) await page.keyboard.press(k);
    await expect(page).toHaveURL(/\/play\/$/);
  });

  test.describe('on a touch device', () => {
    test.use({ hasTouch: true, isMobile: true, viewport: { width: 390, height: 844 } });

    test('Play leads to the game itself, not the clip page (Q78)', async ({ page }) => {
      await blockGame(page);
      await page.goto(HOME);
      await page.getByRole('navigation', { name: 'Site' }).getByRole('link', { name: 'Play' }).tap();
      await expect(page).toHaveURL(/\/play\/$/);
      await page.waitForTimeout(500);
      expect(path(page), 'not sent on to the clip').toBe('/play/');
    });
  });
});
