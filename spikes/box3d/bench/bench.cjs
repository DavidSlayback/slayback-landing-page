// 3D spike bench: loads <base>/index.html?<variant> in headless Chrome on the real GPU and collects the BENCH
// console lines until BENCH_DONE. Same Chrome flags as the B2 spike (crates/game/examples/look/bench.mjs on spike/2.5d).
// usage: NODE_PATH=<repo>/tests/e2e/node_modules node bench.cjs <baseURL> <gpu|gpu-vsync> <out.jsonl> <variant>...
const { chromium } = require('playwright');
const { appendFileSync } = require('node:fs');

const [base, mode, out, ...variants] = process.argv.slice(2);
const ARGS = {
  gpu: ['--use-angle=d3d11', '--enable-gpu', '--ignore-gpu-blocklist', '--disable-gpu-vsync', '--disable-frame-rate-limit'],
  'gpu-vsync': ['--use-angle=d3d11', '--enable-gpu', '--ignore-gpu-blocklist'],
};

(async () => {
  const browser = await chromium.launch({ channel: 'chrome', headless: true, args: ARGS[mode] });
  const ctx = await browser.newContext({ viewport: { width: 1280, height: 720 } });
  const probe = await ctx.newPage();
  await probe.goto(base + '/index.html?bench=0');
  const renderer = await probe.evaluate(() => {
    const gl = document.createElement('canvas').getContext('webgl2');
    if (!gl) return 'no webgl2';
    const ext = gl.getExtension('WEBGL_debug_renderer_info');
    return ext ? gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER);
  });
  await probe.close();
  console.log('RENDERER', mode, renderer, browser.version());
  appendFileSync(out, JSON.stringify({ mode, renderer, args: ARGS[mode], version: browser.version() }) + '\n');
  for (const v of variants) {
    const page = await ctx.newPage();
    const lines = [];
    const errors = [];
    let done = false;
    page.on('console', m => {
      const t = m.text();
      if (t.startsWith('BENCH ')) lines.push(JSON.parse(t.slice(6)));
      else if (t === 'BENCH_DONE') done = true;
      else if (m.type() === 'error') errors.push(t.slice(0, 300));
    });
    page.on('pageerror', e => errors.push(String(e).slice(0, 300)));
    await page.goto(`${base}/index.html?${v}`);
    const t0 = Date.now();
    while (!done && Date.now() - t0 < 180_000) await page.waitForTimeout(500);
    appendFileSync(out, JSON.stringify({ mode, variant: v, done, reports: lines, errors: errors.slice(0, 5) }) + '\n');
    for (const r of lines) {
      if (r.phase) {
        console.log(v, r.phase, `interval ${r.interval_ms.avg}/${r.interval_ms.p95} (max ${r.interval_ms.max}, >16.7ms: ${r.interval_ms.over_16_7})`,
          `cpu ${r.cpu_ms.avg}/${r.cpu_ms.p95} main ${r.main_ms.avg}/${r.main_ms.p95}`,
          `phys/frame ${r.phys_ms_per_frame.avg}/${r.phys_ms_per_frame.p95} steps ${r.steps} frames ${r.frames} awake ${r.awake_avg}`);
      } else console.log(v, JSON.stringify(r));
    }
    if (!done) console.log(v, 'TIMEOUT', errors[0] ?? '');
    else if (errors.length) console.log(v, 'ERR', errors[0]);
    await page.close();
  }
  await browser.close();
})();
