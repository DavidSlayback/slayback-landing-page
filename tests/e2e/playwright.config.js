// Browser smoke harness for the built site (spec: Testing Decisions, seam 3). Serves DIST (default: repo dist/).
import { defineConfig } from '@playwright/test';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';

const here = fileURLToPath(new URL('.', import.meta.url));
export const DIST = resolve(here, process.env.DIST ?? '../../dist');
const PORT = Number(process.env.PORT ?? 4173);

// No GPU locally or on CI runners: WebGL2 comes from SwiftShader. CI uses Playwright's managed Chromium;
// elsewhere the installed Chrome (set PW_CHANNEL= to use a managed browser instead).
export const GL_ARGS = ['--enable-unsafe-swiftshader', '--use-angle=swiftshader'];
const channel = process.env.PW_CHANNEL ?? (process.env.CI ? '' : 'chrome');

export default defineConfig({
  testDir: '.',
  // Headroom for CI's software WebGL (canvas screenshots of the hero took 15-50 s there).
  timeout: 120_000,
  expect: { timeout: 30_000 },
  forbidOnly: !!process.env.CI,
  retries: 0,
  workers: 1,
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
  use: {
    baseURL: `http://127.0.0.1:${PORT}`,
    viewport: { width: 1280, height: 720 },
    ...(channel ? { channel } : {}),
    // Autoplay allowed: Chrome on Linux kept the game's AudioContext suspended with no gesture, so the demo was
    // silent. The wake-on-gesture path is still tested: that test suspends the context itself, then presses a key.
    launchOptions: { args: [...GL_ARGS, '--autoplay-policy=no-user-gesture-required'] },
  },
  webServer: {
    command: `node serve.mjs "${DIST}" ${PORT}`,
    url: `http://127.0.0.1:${PORT}/`,
    reuseExistingServer: !process.env.CI,
    timeout: 15_000,
  },
});
