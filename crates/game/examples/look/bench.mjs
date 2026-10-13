// B2 spike bench: loads <base>/index.html?<variant> in headless Chrome and collects the BENCH console lines.
// usage (from tests/e2e, which has playwright installed):
//   node ../../crates/game/examples/look/bench.mjs <baseURL> <gpu|gpu-vsync|swiftshader> <out.jsonl> <variant>...
import { createRequire } from 'node:module';
import { appendFileSync } from 'node:fs';
import { resolve } from 'node:path';
const require = createRequire(resolve(process.cwd(), 'package.json'));
const { chromium } = require('playwright');
const [base, mode, out, ...variants] = process.argv.slice(2);
const ARGS = {
  gpu: ['--use-angle=d3d11', '--enable-gpu', '--ignore-gpu-blocklist', '--disable-gpu-vsync', '--disable-frame-rate-limit'],
  dgpu: ['--use-angle=d3d11', '--enable-gpu', '--ignore-gpu-blocklist', '--disable-gpu-vsync', '--disable-frame-rate-limit',
    '--force_high_performance_gpu'],
  'gpu-vsync': ['--use-angle=d3d11', '--enable-gpu', '--ignore-gpu-blocklist'],
  swiftshader: ['--enable-unsafe-swiftshader', '--use-angle=swiftshader'],
};
const browser = await chromium.launch({ channel: 'chrome', headless: true, args: ARGS[mode] });
const [vw, vh] = (process.env.VIEWPORT ?? '1280x720').split('x').map(Number);
const ctx = await browser.newContext({ viewport: { width: vw, height: vh } });
const probe = await ctx.newPage();
await probe.goto(base + '/index.html?n=0');
const renderer = await probe.evaluate(() => {
  const gl = document.createElement('canvas').getContext('webgl2');
  if (!gl) return 'no webgl2';
  const ext = gl.getExtension('WEBGL_debug_renderer_info');
  return ext ? gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) + ' | ' + gl.getParameter(ext.UNMASKED_VENDOR_WEBGL) : gl.getParameter(gl.RENDERER);
});
await probe.close();
console.log('RENDERER', mode, renderer, browser.version());
appendFileSync(out, JSON.stringify({ mode, renderer, args: ARGS[mode], version: browser.version() }) + '\n');
for (const v of variants) {
  const page = await ctx.newPage();
  const lines = [];
  const errors = [];
  page.on('console', m => {
    const t = m.text();
    if (t.startsWith('BENCH ')) lines.push(JSON.parse(t.slice(6)));
    else if (m.type() === 'error') errors.push(t.slice(0, 300));
  });
  page.on('pageerror', e => errors.push(String(e).slice(0, 300)));
  await page.goto(`${base}/index.html?${v}`);
  const t0 = Date.now();
  while (lines.length < 2 && Date.now() - t0 < 150_000) await page.waitForTimeout(500);
  if (v.includes('shot')) {
    await page.screenshot({ path: out.replace(/\.jsonl$/, '') + '-' + v.replace(/[^a-z0-9=]/gi, '_') + '.png' });
  }
  appendFileSync(out, JSON.stringify({ mode, viewport: [vw, vh], variant: v, reports: lines, errors: errors.slice(0, 5) }) + '\n');
  const last = lines.at(-1);
  console.log(v, last
    ? `cpu ${last.cpu_ms.avg}/${last.cpu_ms.p95} main ${last.main_ms.avg}/${last.main_ms.p95} interval ${last.interval_ms.avg}/${last.interval_ms.p95}`
    : 'NO REPORT', errors.length ? 'ERR ' + errors[0] : '');
  await page.close();
}
await browser.close();
