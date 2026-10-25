// Out of the Box at /puzzle (ticket OB8). It ships in previews only (GAME_RELEASE=1) until its own release switch, so
// a production build must not have it. Solutions and fingerprints come from bench/puzzle/suite-v1.json, written by the
// native generator: the page's wasm must build exactly the same levels.
import { test, expect } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { collectErrors, litFraction } from './helpers.js';

const ON = process.env.GAME_RELEASE === '1';
const SUITE = JSON.parse(readFileSync(fileURLToPath(new URL('../../bench/puzzle/suite-v1.json', import.meta.url)), 'utf8'));
const level = (tier, seed) => SUITE.levels.find(l => l.tier === tier && l.seed === seed);
const PRESS = { L: 'ArrowLeft', R: 'ArrowRight', U: 'ArrowUp', D: 'ArrowDown' };

async function open(page, tier, seed) {
  await page.goto(`/puzzle/?tier=${tier}&seed=${seed}`);
  await expect(page.locator('main')).toHaveAttribute('data-state', 'playing');
}
const state = page => page.evaluate(() => ({ moves: __puzzle.moves(), solved: __puzzle.solved() }));

test.describe('puzzle in a production build', () => {
  test.skip(ON, 'GAME_RELEASE=1: a preview build has the puzzle');

  test('is not there yet', async ({ page }) => {
    expect((await page.request.get('/puzzle/')).status()).toBe(404);
  });
});

test.describe('puzzle', () => {
  test.skip(!ON, 'set GAME_RELEASE=1 to test a preview build');

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

  test('the wasm builds the same levels as the native generator', async ({ page }) => {
    for (const [tier, seed] of [[1, 1], [1, 2], [2, 1], [3, 1]]) {
      await open(page, tier, seed);
      expect(await page.evaluate(() => __puzzle.fingerprint()), `tier ${tier} seed ${seed}`).toBe(level(tier, seed).fingerprint);
    }
  });
});
