// Frames of the game for review, on the GPU, tiled into one sheet: the way to look at motion frame by frame.
//   node tools/capture.mjs --page '/play/?demo=1&slow=0.15' --frames 64 --every 2 --crop 440:300:420:200 --out <dir>
// --page    path to open (default the slow-motion demo fight)     --wait   ms to wait before the first frame (0)
// --frames  screenshots to take (64; each ~100-200 ms apart)       --every  put every Nth frame on the sheet (1)
// --crop    w:h:x:y of each 1280x720 frame for the sheet (none)    --cols   sheet columns (8)
// --out     directory for the PNGs and sheet.jpg (a temp dir)      --dist, --swiftshader  see lib.mjs
// Read full frames for scale and framing; crops magnify (an earlier camera reading went wrong on 2x crops).
import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { args, openGame } from './lib.mjs';

const a = args({ page: '/play/?demo=1&slow=0.15', frames: 64, every: 1, crop: '', cols: 8, wait: 0, out: '', dist: undefined, swiftshader: false });
const out = a.out ? resolve(a.out) : mkdtempSync(join(tmpdir(), 'capture-'));
mkdirSync(out, { recursive: true });
const { page, close } = await openGame(a.page, { dist: a.dist, swiftshader: a.swiftshader });
try {
  await page.waitForTimeout(a.wait);
  const canvas = page.locator('#game');
  for (let i = 0; i < a.frames; i++) await canvas.screenshot({ path: join(out, `${String(i).padStart(4, '0')}.png`) });
} finally {
  await close();
}
const shown = Math.ceil(a.frames / a.every);
const rows = Math.ceil(shown / a.cols);
const filter = [
  `select='not(mod(n\\,${a.every}))'`,
  a.crop ? `crop=${a.crop}` : null,
  'scale=320:-2',
  `tile=${a.cols}x${rows}`,
].filter(Boolean).join(',');
execFileSync('ffmpeg', ['-v', 'error', '-y', '-start_number', '0', '-i', join(out, '%04d.png'), '-vf', filter, '-frames:v', '1', join(out, 'sheet.jpg')]);
console.log(`${a.frames} frames and sheet.jpg in ${out}`);
