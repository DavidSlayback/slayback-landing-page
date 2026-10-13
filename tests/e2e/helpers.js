// Shared page probes for the smoke specs.

/** The homepage with a lighter hero: these checks are about behaviour (lit text, animation, no errors), and CI's
 *  software WebGL renders the full spark count slowly enough to brush the test timeout. */
export const HOME = '/?n=3000';

/** Every console error and uncaught page error, as readable strings. */
export function collectErrors(page) {
  const errors = [];
  page.on('console', m => { if (m.type() === 'error') errors.push(`console.error: ${m.text()}`); });
  page.on('pageerror', e => errors.push(`pageerror: ${e.message}`));
  return errors;
}

export const hasGl = page => page.evaluate(() => document.documentElement.classList.contains('gl'));

/** Fraction of pixels in a PNG whose luminance exceeds `min` (decoded in the page; no image dependency). */
export async function litFraction(page, png, min = 0.25) {
  return page.evaluate(async ([b64, min]) => {
    const bmp = await createImageBitmap(await (await fetch(`data:image/png;base64,${b64}`)).blob());
    const ctx = new OffscreenCanvas(bmp.width, bmp.height).getContext('2d');
    ctx.drawImage(bmp, 0, 0);
    const px = ctx.getImageData(0, 0, bmp.width, bmp.height).data;
    let lit = 0;
    for (let i = 0; i < px.length; i += 4) if ((0.2126 * px[i] + 0.7152 * px[i + 1] + 0.0722 * px[i + 2]) / 255 > min) lit++;
    return lit / (px.length / 4);
  }, [png.toString('base64'), min]);
}
