//! A small software canvas for the neon look (P18): shapes are drawn as light into a linear RGB buffer, a blur of
//! that light is added back as glow, and the result is tonemapped into the RGBA bytes the page blits.

pub type Rgb = [f32; 3];

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
        }
    }

    pub fn clear(&mut self, c: Rgb) {
        self.light.fill(c);
    }

    /// Adds light to one pixel; off-canvas is ignored.
    pub fn add(&mut self, x: i32, y: i32, c: Rgb) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            let p = &mut self.light[y as usize * self.w + x as usize];
            p[0] += c[0];
            p[1] += c[1];
            p[2] += c[2];
        }
    }

    /// Adds light over the rectangle [x0, x1) × [y0, y1).
    pub fn fill(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgb) {
        for y in y0.max(0)..y1.min(self.h as i32) {
            for x in x0.max(0)..x1.min(self.w as i32) {
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

    /// Glow (a blur of the light, `radius` pixels, added back at `gain`), then a soft tonemap into `rgba`.
    pub fn finish(&mut self, radius: usize, gain: f32) {
        if radius > 0 && gain > 0.0 {
            self.glow.copy_from_slice(&self.light);
            for _ in 0..2 {
                box_blur(&mut self.glow, &mut self.tmp, self.w, self.h, radius);
            }
            for (p, g) in self.light.iter_mut().zip(&self.glow) {
                p[0] += g[0] * gain;
                p[1] += g[1] * gain;
                p[2] += g[2] * gain;
            }
        }
        for (o, p) in self.rgba.as_chunks_mut::<4>().0.iter_mut().zip(&self.light) {
            for k in 0..3 {
                // 1 - e^-x keeps bright overlaps from clipping flat, and lets white-hot cores read white.
                o[k] = ((1.0 - (-p[k] * 1.4).exp()) * 255.0 + 0.5) as u8;
            }
            o[3] = 255;
        }
    }
}

/// One horizontal then one vertical running-sum box blur of `radius`, edges clamped.
fn box_blur(img: &mut [Rgb], tmp: &mut [Rgb], w: usize, h: usize, radius: usize) {
    let r = radius as i32;
    let k = 1.0 / (2 * radius + 1) as f32;
    let pass = |src: &[Rgb], dst: &mut [Rgb], len: usize, lines: usize, at: &dyn Fn(usize, usize) -> usize| {
        for line in 0..lines {
            let mut sum = [0.0f32; 3];
            for i in -r..=r {
                let p = src[at(line, i.clamp(0, len as i32 - 1) as usize)];
                sum = [sum[0] + p[0], sum[1] + p[1], sum[2] + p[2]];
            }
            for i in 0..len {
                dst[at(line, i)] = scale(sum, k);
                let out = src[at(line, (i as i32 - r).clamp(0, len as i32 - 1) as usize)];
                let inn = src[at(line, (i as i32 + r + 1).clamp(0, len as i32 - 1) as usize)];
                sum = [sum[0] + inn[0] - out[0], sum[1] + inn[1] - out[1], sum[2] + inn[2] - out[2]];
            }
        }
    };
    pass(img, tmp, w, h, &|y, x| y * w + x);
    pass(tmp, img, h, w, &|x, y| y * w + x);
}
