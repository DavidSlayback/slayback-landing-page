// Out of the Box at /puzzle (ticket OB8), in every build, production included (P24), with a nav item and a call to
// action on the homepage. Solutions and fingerprints come from bench/puzzle/suite-v1.json, written by the native
// generator: the page's wasm must build exactly the same levels.
import { test, expect } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { collectErrors, litFraction } from './helpers.js';

const SUITE = JSON.parse(readFileSync(fileURLToPath(new URL('../../bench/puzzle/suite-v1.json', import.meta.url)), 'utf8'));
const level = (tier, seed) => SUITE.levels.find(l => l.tier === tier && l.seed === seed);
const PRESS = { L: 'ArrowLeft', R: 'ArrowRight', U: 'ArrowUp', D: 'ArrowDown' };

async function open(page, tier, seed) {
  await page.goto(`/puzzle/?tier=${tier}&seed=${seed}`);
  await expect(page.locator('main')).toHaveAttribute('data-state', 'playing');
}
const state = page => page.evaluate(() => ({ moves: __puzzle.moves(), solved: __puzzle.solved() }));

test.describe('puzzle', () => {
  test('the homepage links it from the nav and the hero', async ({ page }) => {
    await page.route(/hero\.wasm$/, r => r.abort());   // links are what matter here, not the spark field
    await page.goto('/');
    await expect(page.getByRole('navigation', { name: 'Site' }).getByRole('link', { name: 'Puzzle' })).toHaveAttribute('href', 'puzzle/');
    await page.locator('.hero .cta[href="puzzle/"]').click();
    await expect(page).toHaveURL(/\/puzzle\/$/);
    await expect(page.locator('main')).toHaveAttribute('data-state', 'playing');
  });


  test('a tier-1 level is drawn and solved by key presses, with no errors', async ({ page }) => {
    const errors = collectErrors(page);
    const { solution, moves } = level(1, 1);
    await open(page, 1, 1);
    expect(await litFraction(page, await page.locator('#board').screenshot()), 'the board is drawn').toBeGreaterThan(0.03);
    await expect(page.locator('#optimal')).toHaveText(String(moves));
    for (const k of solution) await page.keyboard.press(PRESS[k]);
    await expect.poll(() => state(page)).toEqual({ moves, solved: true });
    await expect(page.locator('main')).toHaveAttribute('data-state', 'solved');
    await expect(page.locator('#message')).toContainText('Out!');
    expect(errors).toEqual([]);
  });

  test('undo takes a move back and restart clears them', async ({ page }) => {
    const { solution } = level(1, 1);
    await open(page, 1, 1);
    for (const k of solution.slice(0, 3)) await page.keyboard.press(PRESS[k]);
    expect((await state(page)).moves).toBe(3);
    await page.keyboard.press('z');
    expect((await state(page)).moves).toBe(2);
    await page.keyboard.press('r');
    expect((await state(page)).moves).toBe(0);
    await expect(page.locator('#moves')).toHaveText('0');
  });

  test.describe('on a phone', () => {
    test.use({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });
    const ACT = { L: '0', R: '1', U: '2', D: '3' };

    test('the touch pad shows and a level is solved by taps', async ({ page }) => {
      const errors = collectErrors(page);
      const { solution, moves } = level(1, 1);
      await open(page, 1, 1);
      await expect(page.locator('#pad')).toBeVisible();
      await expect(page.locator('.touch-help')).toBeVisible();
      for (const b of await page.locator('#pad button').all()) {
        const box = await b.boundingBox();
        expect(box.x >= 0 && box.x + box.width <= 390, `${await b.getAttribute('aria-label')} is fully on screen`).toBe(true);
      }
      for (const k of solution) await page.locator(`#pad [data-act="${ACT[k]}"]`).tap();
      await expect.poll(() => state(page)).toEqual({ moves, solved: true });
      await page.locator('#pad [data-act="undo"]').tap();
      expect((await state(page)).moves).toBe(moves - 1);
      await page.locator('#pad [data-act="restart"]').tap();
      expect((await state(page)).moves).toBe(0);
      expect(errors).toEqual([]);
    });

    test('holding a walk button keeps walking', async ({ page }) => {
      // Tier 1 level 1 starts facing right on a block; holding Left turns, walks twice (dropping to the floor), then
      // meets a block. The refusal shows the repeat was still going after the last legal move.
      await open(page, 1, 1);
      const left = page.locator('#pad [data-act="0"]');
      await left.dispatchEvent('pointerdown', { pointerId: 7, pointerType: 'touch' });
      await page.waitForTimeout(900);
      await left.dispatchEvent('pointerup', { pointerId: 7, pointerType: 'touch' });
      expect((await state(page)).moves).toBe(3);
      await expect(page.locator('#where')).toHaveText('Something is in the way.');
      await page.locator('#pad [data-act="1"]').tap();
      await page.waitForTimeout(600);
      expect((await state(page)).moves, 'a tap is one move, and the released hold stays stopped').toBe(4);
    });

    test('held sideways, the board and the pad fit without scrolling', async ({ page }) => {
      await page.setViewportSize({ width: 844, height: 390 });
      await open(page, 4, 1);
      for (const b of await page.locator('#pad button').all()) {
        const box = await b.boundingBox();
        expect(box.x >= 0 && box.x + box.width <= 844 && box.y + box.height <= 390,
          `${await b.getAttribute('aria-label')} is fully on screen`).toBe(true);
      }
      expect(await page.evaluate(() => document.scrollingElement.scrollHeight)).toBeLessThanOrEqual(391);
    });
  });

  test('the wasm builds the same levels as the native generator', async ({ page }) => {
    for (const [tier, seed] of [[1, 1], [1, 2], [2, 1], [3, 1]]) {
      await open(page, tier, seed);
      expect(await page.evaluate(() => __puzzle.fingerprint()), `tier ${tier} seed ${seed}`).toBe(level(tier, seed).fingerprint);
    }
  });

  // The other worlds (M2-M5): levels from bench/puzzle/suite-v2.json, written by the native generator.
  const V2 = JSON.parse(readFileSync(fileURLToPath(new URL('../../bench/puzzle/suite-v2.json', import.meta.url)), 'utf8'));
  for (const [world, name] of [['crumble', 'Crumble'], ['ice', 'Ice'], ['springs', 'Springs'], ['mixed', 'Mixed']]) {
    test.describe(world, () => {
      const levels = V2.levels.filter(l => l.world === world);
      const go = async (page, tier, seed) => {
        await page.goto(`/puzzle/?world=${world}&tier=${tier}&seed=${seed}`);
        await expect(page.locator('main')).toHaveAttribute('data-state', 'playing');
      };

      test('builds the native levels, says which world, and keeps it in the tier links', async ({ page }) => {
        for (const l of levels.filter(l => l.tier <= 2).slice(0, 4)) {
          await go(page, l.tier, l.seed);
          expect(await page.evaluate(() => __puzzle.fingerprint()), `tier ${l.tier} seed ${l.seed}`).toBe(l.fingerprint);
        }
        await go(page, 2, levels.find(l => l.tier === 2).seed);
        await expect(page.locator('#where')).toContainText(`${name} · Tier 2`);
        await expect(page.locator(`[data-world~="${world}"]`).first()).toBeVisible();
        await expect(page.locator('nav [data-tier="3"]')).toHaveAttribute('href', `?world=${world}&tier=3&seed=1`);
        await expect(page.locator(`nav [data-pick="${world}"]`)).toHaveAttribute('aria-current', 'true');
      });

      test('a level is solved by its native solution, with no errors', async ({ page }) => {
        const errors = collectErrors(page);
        const { seed, solution, moves } = levels.find(l => l.tier === 1);
        await go(page, 1, seed);
        for (const k of solution) await page.keyboard.press(PRESS[k]);
        await expect.poll(() => state(page)).toEqual({ moves, solved: true });
        await expect(page.locator('#message')).toContainText('Out!');
        await expect(page.locator('#message a')).toHaveAttribute('href', `?world=${world}&tier=1&seed=${seed + 1}`);
        expect(errors).toEqual([]);
      });
    });
  }

  test('the world picker keeps the tier and opens a world', async ({ page }) => {
    await open(page, 2, 3);
    await expect(page.locator('nav [data-pick="classic"]')).toHaveAttribute('aria-current', 'true');
    await page.locator('nav [data-pick="ice"]').click();
    await expect(page).toHaveURL(/\?world=ice&tier=2&seed=1$/);
    await expect(page.locator('main')).toHaveAttribute('data-state', 'playing');
    await expect(page.locator('#where')).toHaveText('Ice · Tier 2 · level 1');
  });

  test('the classic page shows no world help', async ({ page }) => {
    await open(page, 1, 1);
    await expect(page.locator('[data-world]')).toHaveCount(4);
    for (const el of await page.locator('[data-world]').all()) await expect(el).toBeHidden();
  });
});
