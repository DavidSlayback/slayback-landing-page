//! A small software canvas for the neon look (P18): shapes are drawn as light into a linear RGB buffer, a blur of
//! that light is added back as glow, and the result is tonemapped into the RGBA bytes the page blits.
//!
//! A frame redraws only what moves. What stands still (walls, the doorway, blocks at rest) is drawn once and
//! [`baked`](Canvas::bake): its light and glow, and its pixels. Each frame [`begin`](Canvas::begin)s from those pixels,
//! draws each moving thing in a small [`layer`](Canvas::layer) [`composite`](Canvas::composite)d over the baked light,
//! and puts sparks over it pixel by pixel. A full redraw cost a phone about 45 ms a frame.

pub type Rgb = [f32; 3];

/// A pixel rectangle [x0, x1) × [y0, y1).
pub type Rect = [i32; 4];

pub fn scale(c: Rgb, k: f32) -> Rgb {
    [c[0] * k, c[1] * k, c[2] * k]
}

pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

pub struct Canvas {
    pub w: usize,
    pub h: usize,
    /// The light drawn this frame, linear RGB.
    light: Vec<Rgb>,
    /// Scratch for the blur.
    tmp: Vec<Rgb>,
    glow: Vec<Rgb>,
    /// What the page reads: RGBA bytes, row by row.
    pub rgba: Vec<u8>,
    curve: Vec<u8>,
    /// Drawing only lands inside this rectangle: the whole canvas, or the layer being drawn.
    clip: Rect,
    /// The still scene, baked: its light with glow added, and its pixels.
    base_light: Vec<Rgb>,
    base_rgba: Vec<u8>,
    /// How many times the still scene has been baked.
    pub bakes: usize,
}

impl Canvas {
    pub fn new(w: usize, h: usize) -> Canvas {
        let n = w * h;
        Canvas {
            w,
            h,
            light: vec![[0.0; 3]; n],
            tmp: vec![[0.0; 3]; n],
            glow: vec![[0.0; 3]; n],
            rgba: vec![255; n * 4],
            curve: tone_curve(),
            clip: [0, 0, w as i32, h as i32],
            base_light: vec![[0.0; 3]; n],
            base_rgba: vec![255; n * 4],
            bakes: 0,
        }
    }

    fn full(&self) -> Rect {
        [0, 0, self.w as i32, self.h as i32]
    }

    pub fn clear(&mut self, c: Rgb) {
        self.clip = self.full();
        self.light.fill(c);
    }

    /// Adds light to one pixel; outside the clip is ignored.
    pub fn add(&mut self, x: i32, y: i32, c: Rgb) {
        let [x0, y0, x1, y1] = self.clip;
        if x >= x0 && y >= y0 && x < x1 && y < y1 {
            let p = &mut self.light[y as usize * self.w + x as usize];
            p[0] += c[0];
            p[1] += c[1];
            p[2] += c[2];
        }
    }

    /// Adds light over the rectangle [x0, x1) × [y0, y1).
    pub fn fill(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgb) {
        let [cx0, cy0, cx1, cy1] = self.clip;
        for y in y0.max(cy0)..y1.min(cy1) {
            for x in x0.max(cx0)..x1.min(cx1) {
                self.add(x, y, c);
            }
        }
    }

    /// The outline of [x0, x1) × [y0, y1), `t` pixels thick, drawn inside it.
    pub fn frame(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, t: i32, c: Rgb) {
        self.fill(x0, y0, x1, y0 + t, c);
        self.fill(x0, y1 - t, x1, y1, c);
        self.fill(x0, y0 + t, x0 + t, y1 - t, c);
        self.fill(x1 - t, y0 + t, x1, y1 - t, c);
    }

    /// A line from (x0, y0) to (x1, y1), `width` pixels across, with soft edges.
    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, c: Rgb) {
        let (dx, dy) = (x1 - x0, y1 - y0);
        let len2 = (dx * dx + dy * dy).max(1e-6);
        let reach = width / 2.0 + 1.0;
        let (bx0, bx1) = ((x0.min(x1) - reach).floor() as i32, (x0.max(x1) + reach).ceil() as i32);
        let (by0, by1) = ((y0.min(y1) - reach).floor() as i32, (y0.max(y1) + reach).ceil() as i32);
        for y in by0..=by1 {
            for x in bx0..=bx1 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let t = (((px - x0) * dx + (py - y0) * dy) / len2).clamp(0.0, 1.0);
                let d = ((px - x0 - dx * t).powi(2) + (py - y0 - dy * t).powi(2)).sqrt();
                let a = (width / 2.0 + 0.5 - d).clamp(0.0, 1.0);
                if a > 0.0 {
                    self.add(x, y, scale(c, a));
                }
            }
        }
    }

    /// A small square of light added over the still scene straight into the frame's pixels, as [`spark`](Canvas::spark)
    /// does: for many small things (win fragments) that don't need their own glow.
    pub fn speck(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgb) {
        for y in y0.max(0)..y1.min(self.h as i32) {
            for x in x0.max(0)..x1.min(self.w as i32) {
                let i = y as usize * self.w + x as usize;
                let b = self.base_light[i];
                let lit = self.toned([b[0] + c[0], b[1] + c[1], b[2] + c[2]]);
                for (o, v) in self.rgba[i * 4..i * 4 + 3].iter_mut().zip(lit) {
                    *o = (*o).max(v);
                }
            }
        }
    }

    /// A filled disc of radius `r` with a soft edge.
    pub fn disc(&mut self, cx: f32, cy: f32, r: f32, c: Rgb) {
        let (x0, x1) = ((cx - r - 1.0).floor() as i32, (cx + r + 1.0).ceil() as i32);
        let (y0, y1) = ((cy - r - 1.0).floor() as i32, (cy + r + 1.0).ceil() as i32);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                let a = (r + 0.5 - d).clamp(0.0, 1.0);
                if a > 0.0 {
                    self.add(x, y, scale(c, a));
                }
            }
        }
    }

    /// Glow (a blur of the light, about `radius` pixels, added back at `gain`), then a soft tonemap into `rgba`.
    ///
    /// The glow has no fine detail, so it is worked at half size: the light averaged down 2 × 2, blurred there, and
    /// scaled back up bilinearly as it is added. That is a quarter of the pixels for the costliest part of a frame.
    pub fn finish(&mut self, radius: usize, gain: f32) {
        let (w, h, sw, sh) = (self.w, self.h, self.w / 2, self.h / 2);
        if radius > 0 && gain > 0.0 && sw > 0 && sh > 0 {
            for y in 0..sh {
                for x in 0..sw {
                    let i = 2 * y * w + 2 * x;
                    let (a, b, c, d) = (self.light[i], self.light[i + 1], self.light[i + w], self.light[i + w + 1]);
                    self.glow[y * sw + x] =
                        scale([a[0] + b[0] + c[0] + d[0], a[1] + b[1] + c[1] + d[1], a[2] + b[2] + c[2] + d[2]], 0.25);
                }
            }
            let r = radius.div_ceil(2);
            for _ in 0..2 {
                blur_lines(&self.glow, &mut self.tmp, sw, sh, sw, 1, r);
                blur_lines(&self.tmp, &mut self.glow, sh, sw, 1, sw, r);
            }
            // Up across each half-size row into `tmp` (w × sh), then up the columns, adding into the light.
            let along = |x: usize, n: usize| {
                let f = (x as f32 + 0.5) / 2.0 - 0.5;
                let i0 = (f.floor().max(0.0) as usize).min(n - 1);
                (i0, (i0 + 1).min(n - 1), (f - i0 as f32).clamp(0.0, 1.0))
            };
            for y in 0..sh {
                for x in 0..w {
                    let (x0, x1, t) = along(x, sw);
                    self.tmp[y * w + x] = mix(self.glow[y * sw + x0], self.glow[y * sw + x1], t);
                }
            }
            for y in 0..h {
                let (y0, y1, t) = along(y, sh);
                for x in 0..w {
                    let g = mix(self.tmp[y0 * w + x], self.tmp[y1 * w + x], t);
                    let p = &mut self.light[y * w + x];
                    p[0] += g[0] * gain;
                    p[1] += g[1] * gain;
                    p[2] += g[2] * gain;
                }
            }
        }
        let curve = &self.curve;
        for (o, p) in self.rgba.as_chunks_mut::<4>().0.iter_mut().zip(&self.light) {
            for k in 0..3 {
                o[k] = curve[((p[k] * CURVE_STEPS) as usize).min(CURVE_LEN - 1)];
            }
            o[3] = 255;
        }
    }
}

impl Canvas {
    /// Glows and tonemaps what was drawn since [`clear`](Canvas::clear), and keeps it as the still scene.
    pub fn bake(&mut self, radius: usize, gain: f32) {
        self.finish(radius, gain);
        self.base_light.copy_from_slice(&self.light);
        self.base_rgba.copy_from_slice(&self.rgba);
        self.bakes += 1;
    }

    /// Starts a frame from the still scene's pixels.
    pub fn begin(&mut self) {
        self.rgba.copy_from_slice(&self.base_rgba);
    }

    /// Starts drawing a moving thing into `r` alone, from no light.
    pub fn layer(&mut self, r: Rect) {
        let [x0, y0, x1, y1] = r;
        self.clip = [x0.max(0), y0.max(0), x1.min(self.w as i32), y1.min(self.h as i32)];
        let [x0, y0, x1, y1] = self.clip;
        for y in y0..y1.max(y0) {
            self.light[y as usize * self.w + x0 as usize..y as usize * self.w + x1.max(x0) as usize].fill([0.0; 3]);
        }
    }

    /// Glows the layer and lays it over the still scene's light, tonemapped into the frame's pixels.
    pub fn composite(&mut self, radius: usize, gain: f32) {
        let [x0, y0, x1, y1] = self.clip.map(|v| v as usize);
        self.clip = self.full();
        let (rw, rh) = (x1.saturating_sub(x0), y1.saturating_sub(y0));
        if rw == 0 || rh == 0 {
            return;
        }
        for y in 0..rh {
            self.glow[y * rw..(y + 1) * rw]
                .copy_from_slice(&self.light[(y0 + y) * self.w + x0..(y0 + y) * self.w + x1]);
        }
        for _ in 0..2 {
            blur_lines(&self.glow, &mut self.tmp, rw, rh, rw, 1, radius);
            blur_lines(&self.tmp, &mut self.glow, rh, rw, 1, rw, radius);
        }
        for y in 0..rh {
            for x in 0..rw {
                let i = (y0 + y) * self.w + x0 + x;
                let (b, l, g) = (self.base_light[i], self.light[i], self.glow[y * rw + x]);
                let p = [b[0] + l[0] + g[0] * gain, b[1] + l[1] + g[1] * gain, b[2] + l[2] + g[2] * gain];
                self.tone(i, p);
            }
        }
    }

    /// A spark: a soft dot added over the still scene's light, straight into the frame's pixels (brightest wins where
    /// they overlap).
    pub fn spark(&mut self, cx: f32, cy: f32, r: f32, c: Rgb) {
        let (x0, x1) = (((cx - r - 1.0).floor() as i32).max(0), ((cx + r + 1.0).ceil() as i32).min(self.w as i32 - 1));
        let (y0, y1) = (((cy - r - 1.0).floor() as i32).max(0), ((cy + r + 1.0).ceil() as i32).min(self.h as i32 - 1));
        for y in y0..=y1 {
            for x in x0..=x1 {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                let a = (r + 0.5 - d).clamp(0.0, 1.0);
                if a > 0.0 {
                    let i = y as usize * self.w + x as usize;
                    let b = self.base_light[i];
                    let lit = self.toned([b[0] + c[0] * a, b[1] + c[1] * a, b[2] + c[2] * a]);
                    for (o, v) in self.rgba[i * 4..i * 4 + 3].iter_mut().zip(lit) {
                        *o = (*o).max(v);
                    }
                }
            }
        }
    }

    fn toned(&self, p: Rgb) -> [u8; 3] {
        p.map(|v| self.curve[((v * CURVE_STEPS) as usize).min(CURVE_LEN - 1)])
    }

    fn tone(&mut self, i: usize, p: Rgb) {
        let lit = self.toned(p);
        self.rgba[i * 4..i * 4 + 3].copy_from_slice(&lit);
    }
}

/// The tonemap as a table: entries per unit of light, and how many (light above 8 is white anyway).
const CURVE_STEPS: f32 = 256.0;
const CURVE_LEN: usize = 2048;

/// 1 - e^-1.4x keeps bright overlaps from clipping flat and lets white-hot cores read white. Wasm has no `exp`
/// instruction, so three calls a pixel cost as much as the rest of the frame: it is looked up instead.
fn tone_curve() -> Vec<u8> {
    (0..CURVE_LEN).map(|i| ((1.0 - (-(i as f32 + 0.5) / CURVE_STEPS * 1.4).exp()) * 255.0 + 0.5) as u8).collect()
}

/// A running-sum box blur of `radius` along `lines` lines of `len` pixels, edges clamped. Pixel `i` of line `l` is at
/// `l * line_step + i * step`, so one function blurs rows (`step` 1) and columns (`line_step` 1).
fn blur_lines(src: &[Rgb], dst: &mut [Rgb], len: usize, lines: usize, line_step: usize, step: usize, radius: usize) {
    let (r, last) = (radius as isize, len as isize - 1);
    let k = 1.0 / (2 * radius + 1) as f32;
    for l in 0..lines {
        let at = |i: isize| l * line_step + i.clamp(0, last) as usize * step;
        let mut sum = [0.0f32; 3];
        for i in -r..=r {
            let p = src[at(i)];
            sum = [sum[0] + p[0], sum[1] + p[1], sum[2] + p[2]];
        }
        for i in 0..len as isize {
            dst[at(i)] = scale(sum, k);
            let (out, inn) = (src[at(i - r)], src[at(i + r + 1)]);
            sum = [sum[0] + inn[0] - out[0], sum[1] + inn[1] - out[1], sum[2] + inn[2] - out[2]];
        }
    }
}
