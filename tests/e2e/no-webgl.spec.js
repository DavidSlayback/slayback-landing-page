// No-WebGL fallback: its own file because disabling WebGL needs a separate browser launch (a new worker).
import { test, expect } from '@playwright/test';
import { hasGl } from './helpers.js';

test.use({ launchOptions: { args: ['--disable-webgl', '--disable-webgl2', '--disable-3d-apis'] } });

test('no-WebGL fallback: text stays readable DOM and html.gl is never set', async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('#c'), 'hero.js hides the canvas when WebGL2 is unavailable').toBeHidden();
  await page.waitForTimeout(500);
  expect(await hasGl(page)).toBe(false);

  const name = page.locator('.name');
  await expect(name).toBeVisible();
  await expect(name).toHaveText(/David\s*Slayback/i);
  const color = await name.evaluate(el => getComputedStyle(el).color);
  const [r, g, b, alpha = 1] = color.match(/[\d.]+/g).map(Number);
  expect(alpha, `.name colour ${color} is not transparent`).toBeGreaterThan(0.5);
  expect(0.2126 * r + 0.7152 * g + 0.0722 * b, `.name colour ${color} is light on the black page`).toBeGreaterThan(128);
});
