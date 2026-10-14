// Shader links and GPU stalls from page load, on the GPU, from Chrome's trace. WebGL links a program the first time a
// new material or mesh layout is drawn; on ANGLE/D3D11 a Bevy PBR variant took ~3.4 s (decisions Q72, Q73). Use it
// to see what the load costs and to find a stall's cause (the harness's shaders.spec.js only guards the fight).
//   node tools/trace-shaders.mjs --page '/play/?demo=1' --seconds 14 --out trace.json
import { writeFileSync } from 'node:fs';
import { args, openGame } from './lib.mjs';

const a = args({ page: '/play/?demo=1', seconds: 14, out: '', dist: undefined, swiftshader: false });
// Open a blank page first, so tracing covers the game's whole load.
const { browser, page, close } = await openGame('/', { dist: a.dist, swiftshader: a.swiftshader });
try {
  await browser.startTracing(page, { categories: ['gpu', 'devtools.timeline'] });
  const t0 = Date.now();
  await page.goto(new URL(a.page, page.url()).href);
  await page.waitForFunction(() => document.getElementById('game')?.width > 300, null, { timeout: 150_000 });
  console.log(`canvas ready ${Date.now() - t0} ms after navigation`);
  await page.waitForTimeout(a.seconds * 1000);
  const buffer = await browser.stopTracing();
  if (a.out) writeFileSync(a.out, buffer);
  const events = (j => j.traceEvents ?? j)(JSON.parse(buffer.toString()));
  const timed = events.filter(e => e.ph === 'X' && typeof e.dur === 'number');
  const start = Math.min(...timed.map(e => e.ts));
  const links = timed.filter(e => e.name.endsWith('DoLinkProgram'));
  const stalls = timed.filter(e => e.name === 'CommandBuffer::Flush' && e.dur > 100_000);
  console.log(`shader links: ${links.length}`);
  console.log(`GPU stalls over 100 ms: ${stalls.map(e => `${((e.ts - start) / 1e6).toFixed(2)}s:${Math.round(e.dur / 1e3)}ms`).join(' ') || 'none'}`);
} finally {
  await close();
}
