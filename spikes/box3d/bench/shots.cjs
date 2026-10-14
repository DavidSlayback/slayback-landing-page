// 3D spike screenshots (1280x720, real GPU): each shot is <name>=<query>@<seconds after load>.
// usage: NODE_PATH=<repo>/tests/e2e/node_modules node shots.cjs <baseURL> <outDir> <name=query@secs>...
const { chromium } = require('playwright');
const path = require('node:path');

const [base, outDir, ...shots] = process.argv.slice(2);
(async () => {
  const browser = await chromium.launch({ channel: 'chrome', headless: true, args: ['--use-angle=d3d11', '--enable-gpu', '--ignore-gpu-blocklist'] });
  const ctx = await browser.newContext({ viewport: { width: 1280, height: 720 } });
  for (const s of shots) {
    const [, name, query, secs] = s.match(/^([^=]+)=(.*)@([\d.]+)$/);
    const page = await ctx.newPage();
    page.on('pageerror', e => console.log(name, 'pageerror', String(e).slice(0, 200)));
    page.on('console', m => m.type() === 'error' && console.log(name, 'console', m.text().slice(0, 300)));
    await page.goto(`${base}/index.html?${query}`);
    await page.waitForTimeout(Number(secs) * 1000);
    const file = path.join(outDir, `${name}.png`);
    await page.screenshot({ path: file });
    console.log('shot', file);
    await page.close();
  }
  await browser.close();
})();
