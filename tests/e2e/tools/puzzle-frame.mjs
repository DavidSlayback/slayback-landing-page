// /puzzle's frame cost: how long each frame spends drawing in Rust and blitting, and how often frames come, with the
// CPU slowed to a phone's (Chrome's CPU throttling; --cpu 1 for this machine as it is). It plays a few moves first so
// tweens and sparks are in the sample.
//   node tools/puzzle-frame.mjs --tier 4 --cpu 4 --seconds 4
import { args, openGame } from './lib.mjs';

const a = args({ tier: 4, seed: 1, cpu: 4, seconds: 4, dist: undefined, swiftshader: false });
const { page, close } = await openGame(`/puzzle/?tier=${a.tier}&seed=${a.seed}`, { dist: a.dist, swiftshader: a.swiftshader });
try {
  await page.waitForFunction(() => document.querySelector('main')?.dataset.state === 'playing', null, { timeout: 60_000 });
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Emulation.setCPUThrottlingRate', { rate: a.cpu });
  const keys = ['ArrowLeft', 'ArrowRight', 'ArrowDown', 'ArrowUp'];
  const intervals = await page.evaluate(async ms => {
    const out = [];
    let last = performance.now();
    const t0 = last;
    await new Promise(done => {
      const tick = now => { out.push(now - last); last = now; now - t0 < ms ? requestAnimationFrame(tick) : done(); };
      requestAnimationFrame(tick);
    });
    return out;
  }, a.seconds * 1000).then(async r => r);
  for (const k of keys) await page.keyboard.press(k);
  const work = await page.evaluate(() => __puzzle.work());
  const stat = v => {
    const s = [...v].sort((x, y) => x - y);
    const q = p => s[Math.min(s.length - 1, Math.floor(s.length * p))].toFixed(1);
    return `median ${q(0.5)} p90 ${q(0.9)} max ${q(1)}`;
  };
  console.log(`tier ${a.tier}, CPU ${a.cpu}x slower, ${work.length} frames`);
  console.log(`  frame interval ms: ${stat(intervals)}  (${(1000 / (intervals.reduce((x, y) => x + y, 0) / intervals.length)).toFixed(0)} fps)`);
  console.log(`  draw in Rust ms:   ${stat(work.map(w => w[0]))}`);
  console.log(`  blit ms:           ${stat(work.map(w => w[1]))}`);
} finally {
  await close();
}
