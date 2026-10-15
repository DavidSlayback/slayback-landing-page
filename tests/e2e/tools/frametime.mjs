// Frame times on the GPU: the slow frames (with when they happened), median, p99 and max. Run it on the demo fight to
// catch hitches on first use (a blow, a kill, a burst).
//   node tools/frametime.mjs --page '/play/?demo=1' --seconds 16 --slow 25
import { args, openGame } from './lib.mjs';

const a = args({ page: '/play/?demo=1', seconds: 16, slow: 25, dist: undefined, swiftshader: false });
const { page, close } = await openGame(a.page, { dist: a.dist, swiftshader: a.swiftshader });
try {
  const frames = await page.evaluate(async ms => {
    const out = [];
    let last = performance.now();
    const t0 = last;
    await new Promise(done => {
      const tick = now => { out.push([now - t0, now - last]); last = now; now - t0 < ms ? requestAnimationFrame(tick) : done(); };
      requestAnimationFrame(tick);
    });
    return out;
  }, a.seconds * 1000);
  const slow = frames.filter(([, d]) => d > a.slow).map(([t, d]) => `${(t / 1000).toFixed(2)}s:${d.toFixed(0)}ms`);
  const sorted = frames.map(f => f[1]).sort((x, y) => x - y);
  const at = q => sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * q))].toFixed(1);
  console.log(`frames ${frames.length} | median ${at(0.5)} ms | p99 ${at(0.99)} ms | max ${at(1)} ms`);
  console.log(`over ${a.slow} ms: ${slow.join(' ') || 'none'}`);
} finally {
  await close();
}
