// SPIKE (B1): runs the debris bench matrix in Chrome and prints one BENCH json per config plus the WebGL renderer.
// Usage (from tests/e2e so @playwright/test resolves): node ../../crates/game/examples/debris-web/bench.mjs <servedDir>
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = fileURLToPath(new URL('.', import.meta.url));
const dir = process.argv[2];
const PORT = 4181;
const server = spawn(process.execPath, [join(here, '../../../../tests/e2e/serve.mjs'), dir, String(PORT)], { stdio: 'ignore' });
const args = (process.env.GL ?? '--use-angle=d3d11 --enable-gpu --ignore-gpu-blocklist').split(' ').filter(Boolean);
const browser = await chromium.launch({ channel: 'chrome', args });
const ns = (process.env.NS ?? '100,300,600,1000').split(',').map(Number);
const subs = (process.env.SUBS ?? '4,6').split(',').map(Number);
const policies = (process.env.POLICIES ?? 'none,asleep,after3').split(',');
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
  await new Promise(r => setTimeout(r, 500));
  await page.goto(`http://127.0.0.1:${PORT}/`);
  const renderer = await page.evaluate(() => {
    const gl = document.createElement('canvas').getContext('webgl2');
    const ext = gl && gl.getExtension('WEBGL_debug_renderer_info');
    return gl ? gl.getParameter(ext ? ext.UNMASKED_RENDERER_WEBGL : gl.RENDERER) : 'no webgl2';
  });
  console.log(`RENDERER ${renderer}`);
  for (const n of ns) for (const sub of subs) for (const policy of policies) {
    const p = await browser.newPage({ viewport: { width: 1280, height: 720 } });
    const errors = [];
    p.on('pageerror', e => errors.push(e.message));
    const line = new Promise(res => p.on('console', m => { if (m.text().includes('BENCH ')) res(m.text().slice(m.text().indexOf('BENCH '))); }));
    await p.goto(`http://127.0.0.1:${PORT}/?n=${n}&sub=${sub}&policy=${policy}`);
    const timeout = new Promise(res => setTimeout(() => res(`TIMEOUT n=${n} sub=${sub} policy=${policy} errors=${JSON.stringify(errors)}`), 90_000));
    console.log(await Promise.race([line, timeout]));
    if (errors.some(e => !e.includes('Using exceptions'))) console.log(`ERRORS ${JSON.stringify(errors)}`);
    await p.close();
  }
} finally {
  await browser.close();
  server.kill();
}
