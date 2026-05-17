#[cfg(test)]
use rayon::prelude::*;

/// Normalized adjustment inputs from UI sliders.
#[derive(Clone, Copy, Debug, Default)]
pub struct AdjustParams {
    /// -1..1
    pub brightness: f32,
    /// -1..1
    pub contrast: f32,
    /// -1..1
    pub saturation: f32,
    /// Degrees, -180..180
    pub hue: f32,
    /// -1..1
    pub vibrance: f32,
}

impl AdjustParams {
    /// True when every slider is at its neutral default — sampled pixels can be
    /// written straight through without the HSL roundtrip.
    pub(crate) fn is_identity(&self) -> bool {
        self.brightness.abs() < 1e-4
            && self.contrast.abs() < 1e-4
            && self.saturation.abs() < 1e-4
            && self.hue.abs() < 1e-4
            && self.vibrance.abs() < 1e-4
    }
}

#[inline]
fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

#[inline]
fn rgb_to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) * 0.5;
    if (max - min).abs() < f32::EPSILON {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l < 0.5 {
        d / (max + min)
    } else {
        d / (2.0 - max - min)
    };
    let h = if (max - r).abs() < f32::EPSILON {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if (max - g).abs() < f32::EPSILON {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } / 6.0;
    (h.fract(), s, l)
}

#[inline]
fn hue_to_rgb(p: f32, q: f32, t: f32) -> f32 {
    let mut t = t;
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        return p + (q - p) * 6.0 * t;
    }
    if t < 1.0 / 2.0 {
        return q;
    }
    if t < 2.0 / 3.0 {
        return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
    }
    p
}

#[inline]
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s.abs() < f32::EPSILON {
        return (l, l, l);
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    (
        hue_to_rgb(p, q, h + 1.0 / 3.0),
        hue_to_rgb(p, q, h),
        hue_to_rgb(p, q, h - 1.0 / 3.0),
    )
}

#[inline]
pub(crate) fn process_pixel(r: u8, g: u8, b: u8, a: u8, params: AdjustParams) -> (u8, u8, u8, u8) {
    let mut r = r as f32 / 255.0;
    let mut g = g as f32 / 255.0;
    let mut b = b as f32 / 255.0;

    // Brightness then contrast on RGB (common photo-editor order).
    r += params.brightness * 0.5;
    g += params.brightness * 0.5;
    b += params.brightness * 0.5;

    let contrast = 1.0 + params.contrast;
    r = (r - 0.5) * contrast + 0.5;
    g = (g - 0.5) * contrast + 0.5;
    b = (b - 0.5) * contrast + 0.5;

    let (mut h, mut s, l) = rgb_to_hsl(r, g, b);
    h = (h + params.hue / 360.0).fract();

    let sat_factor = 1.0 + params.saturation;
    s = clamp01(s * sat_factor);

    let (r, g, b) = hsl_to_rgb(h, s, l);

    // Vibrance: boost low-saturation pixels more (SweetFX-style).
    let max_c = r.max(g).max(b);
    let min_c = r.min(g).min(b);
    let sat_level = max_c - min_c;
    let luma = 0.299 * r + 0.587 * g + 0.114 * b;
    let vib = 1.0 + params.vibrance * (1.0 - sat_level);
    let r = clamp01(luma + (r - luma) * vib);
    let g = clamp01(luma + (g - luma) * vib);
    let b = clamp01(luma + (b - luma) * vib);

    (
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
        a,
    )
}

/// Apply adjustments from `src` into `dst` (same length, RGBA8).
/// Retained for parity testing; production renders go through `compose::apply_compose_rgba8`.
#[cfg(test)]
pub(crate) fn apply_rgba8(src: &[u8], dst: &mut [u8], params: AdjustParams) {
    debug_assert_eq!(src.len(), dst.len());
    dst.par_chunks_mut(4)
        .zip(src.par_chunks(4))
        .for_each(|(out, inp)| {
            let (r, g, b, a) = process_pixel(inp[0], inp[1], inp[2], inp[3], params);
            out[0] = r;
            out[1] = g;
            out[2] = b;
            out[3] = a;
        });
}
