use rayon::prelude::*;

use crate::processing::adjust::process_pixel;
use crate::processing::AdjustParams;

/// Viewport transform applied to the source image to produce the document image.
/// Pan is expressed in document-fractions (e.g. `pan_x_norm = 0.25` shifts the
/// image centre right by `W/4` document pixels). Zoom is a multiplier (1.0 = fit).
#[derive(Clone, Copy, Debug)]
pub struct TransformParams {
    pub rotation_deg: f32,
    pub zoom: f32,
    pub pan_x_norm: f32,
    pub pan_y_norm: f32,
}

impl Default for TransformParams {
    fn default() -> Self {
        Self {
            rotation_deg: 0.0,
            zoom: 1.0,
            pan_x_norm: 0.0,
            pan_y_norm: 0.0,
        }
    }
}

impl TransformParams {
    pub fn is_identity(&self) -> bool {
        (self.rotation_deg).abs() < 1e-4
            && (self.zoom - 1.0).abs() < 1e-4
            && self.pan_x_norm.abs() < 1e-5
            && self.pan_y_norm.abs() < 1e-5
    }
}

#[inline]
fn sample_bilinear(src: &[u8], w: u32, h: u32, sx: f32, sy: f32) -> (u8, u8, u8, u8) {
    if sx < 0.0 || sy < 0.0 {
        return (0, 0, 0, 0);
    }
    let w_i = w as i32;
    let h_i = h as i32;
    let x0 = sx.floor() as i32;
    let y0 = sy.floor() as i32;
    let x1 = x0 + 1;
    let y1 = y0 + 1;
    if x0 >= w_i || y0 >= h_i || x1 < 0 || y1 < 0 {
        return (0, 0, 0, 0);
    }
    let fx = sx - x0 as f32;
    let fy = sy - y0 as f32;
    let stride = w as usize * 4;

    let fetch = |x: i32, y: i32| -> (f32, f32, f32, f32) {
        if x < 0 || y < 0 || x >= w_i || y >= h_i {
            return (0.0, 0.0, 0.0, 0.0);
        }
        let idx = y as usize * stride + x as usize * 4;
        (
            src[idx] as f32,
            src[idx + 1] as f32,
            src[idx + 2] as f32,
            src[idx + 3] as f32,
        )
    };
    let (r00, g00, b00, a00) = fetch(x0, y0);
    let (r10, g10, b10, a10) = fetch(x1, y0);
    let (r01, g01, b01, a01) = fetch(x0, y1);
    let (r11, g11, b11, a11) = fetch(x1, y1);

    let w00 = (1.0 - fx) * (1.0 - fy);
    let w10 = fx * (1.0 - fy);
    let w01 = (1.0 - fx) * fy;
    let w11 = fx * fy;

    let r = r00 * w00 + r10 * w10 + r01 * w01 + r11 * w11;
    let g = g00 * w00 + g10 * w10 + g01 * w01 + g11 * w11;
    let b = b00 * w00 + b10 * w10 + b01 * w01 + b11 * w11;
    let a = a00 * w00 + a10 * w10 + a01 * w01 + a11 * w11;
    (
        r.round().clamp(0.0, 255.0) as u8,
        g.round().clamp(0.0, 255.0) as u8,
        b.round().clamp(0.0, 255.0) as u8,
        a.round().clamp(0.0, 255.0) as u8,
    )
}

/// Compose source `(src_w × src_h)` into document `(dst_w × dst_h)` in one pass:
/// bake the viewport transform then apply per-pixel adjustments.
/// Pixels falling outside the source rectangle become transparent black.
pub fn apply_compose_rgba8(
    src: &[u8],
    src_w: u32,
    src_h: u32,
    dst: &mut [u8],
    dst_w: u32,
    dst_h: u32,
    adjust: AdjustParams,
    transform: TransformParams,
) {
    debug_assert_eq!(src.len(), (src_w as usize) * (src_h as usize) * 4);
    debug_assert_eq!(dst.len(), (dst_w as usize) * (dst_h as usize) * 4);

    if src_w == dst_w && src_h == dst_h && transform.is_identity() {
        // Hot path: same dims + identity transform → just adjust in place.
        dst.par_chunks_mut(4)
            .zip(src.par_chunks(4))
            .for_each(|(out, inp)| {
                let (r, g, b, a) = process_pixel(inp[0], inp[1], inp[2], inp[3], adjust);
                out[0] = r;
                out[1] = g;
                out[2] = b;
                out[3] = a;
            });
        return;
    }

    // Document centre (in document pixel coords).
    let dst_cx = dst_w as f32 * 0.5;
    let dst_cy = dst_h as f32 * 0.5;
    // Source centre (in source pixel coords).
    let src_cx = src_w as f32 * 0.5;
    let src_cy = src_h as f32 * 0.5;
    // Pan offset in document pixels.
    let pan_x = transform.pan_x_norm * dst_w as f32;
    let pan_y = transform.pan_y_norm * dst_h as f32;
    let inv_zoom = 1.0 / transform.zoom.max(1e-4);
    // Inverse rotation: source-from-doc uses -theta.
    let theta = -transform.rotation_deg.to_radians();
    let cos_t = theta.cos();
    let sin_t = theta.sin();
    let dst_stride = dst_w as usize * 4;
    let src_stride = src_w as usize * 4;
    // Skip the HSL roundtrip entirely when no colour slider is engaged.
    let need_adjust = !adjust.is_identity();
    // Interior-sampler bounds: sx ∈ [0, src_w-1), sy ∈ [0, src_h-1) keeps both
    // (x0, y0) and (x0+1, y0+1) in the source rect → no per-corner bounds check.
    let sx_interior_max = (src_w as f32 - 1.0).max(0.0);
    let sy_interior_max = (src_h as f32 - 1.0).max(0.0);
    // Forward-difference step per pixel along the row (rotation+zoom are constant per row).
    let sx_step = cos_t * inv_zoom;
    let sy_step = sin_t * inv_zoom;

    dst.par_chunks_mut(dst_stride)
        .enumerate()
        .for_each(|(y, row)| {
            // Doc-centred coordinate of the first pixel in this row.
            let row_dx0 = 0.5 - dst_cx - pan_x;
            let row_dy = (y as f32 + 0.5) - dst_cy - pan_y;
            // Inverse rotate + scale → source pixel for x = 0.
            let mut sx = (row_dx0 * cos_t - row_dy * sin_t) * inv_zoom + src_cx - 0.5;
            let mut sy = (row_dx0 * sin_t + row_dy * cos_t) * inv_zoom + src_cy - 0.5;

            for x in 0..dst_w as usize {
                let (mut r, mut g, mut b, mut a) =
                    if sx >= 0.0 && sx < sx_interior_max && sy >= 0.0 && sy < sy_interior_max {
                        // Interior: both corner pairs known in bounds.
                        sample_bilinear_interior(src, src_stride, sx, sy)
                    } else {
                        sample_bilinear(src, src_w, src_h, sx, sy)
                    };
                if a == 0 {
                    r = 0;
                    g = 0;
                    b = 0;
                } else if need_adjust {
                    let p = process_pixel(r, g, b, a, adjust);
                    r = p.0;
                    g = p.1;
                    b = p.2;
                    a = p.3;
                }
                let idx = x * 4;
                row[idx] = r;
                row[idx + 1] = g;
                row[idx + 2] = b;
                row[idx + 3] = a;

                sx += sx_step;
                sy += sy_step;
            }
        });
}

/// Bilinear sampler with bounds elision. Caller guarantees
/// `0 <= sx < src_w - 1` and `0 <= sy < src_h - 1`, so `(x0, y0)` and the
/// `(x0+1, y0+1)` corner all live inside the source rectangle.
#[inline]
fn sample_bilinear_interior(src: &[u8], src_stride: usize, sx: f32, sy: f32) -> (u8, u8, u8, u8) {
    let x0 = sx as usize;
    let y0 = sy as usize;
    let fx = sx - x0 as f32;
    let fy = sy - y0 as f32;

    let base = y0 * src_stride + x0 * 4;
    // Single bounds check per row slice; per-channel indexing is elided.
    let row0 = &src[base..base + 8];
    let row1 = &src[base + src_stride..base + src_stride + 8];

    let w00 = (1.0 - fx) * (1.0 - fy);
    let w10 = fx * (1.0 - fy);
    let w01 = (1.0 - fx) * fy;
    let w11 = fx * fy;

    let lerp = |c00: u8, c10: u8, c01: u8, c11: u8| -> u8 {
        let v = c00 as f32 * w00 + c10 as f32 * w10 + c01 as f32 * w01 + c11 as f32 * w11;
        v.round().clamp(0.0, 255.0) as u8
    };
    (
        lerp(row0[0], row0[4], row1[0], row1[4]),
        lerp(row0[1], row0[5], row1[1], row1[5]),
        lerp(row0[2], row0[6], row1[2], row1[6]),
        lerp(row0[3], row0[7], row1[3], row1[7]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::processing::adjust::apply_rgba8;

    fn fixture(w: u32, h: u32) -> Vec<u8> {
        let mut v = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                v.push((x * 3) as u8);
                v.push((y * 5) as u8);
                v.push(((x ^ y) * 7) as u8);
                v.push(255);
            }
        }
        v
    }

    #[test]
    fn identity_matches_apply_rgba8() {
        let w = 32u32;
        let h = 24u32;
        let src = fixture(w, h);
        let cases = [
            AdjustParams::default(),
            AdjustParams { brightness: 0.2, contrast: -0.1, saturation: 0.3, hue: 30.0, vibrance: -0.2 },
        ];
        for params in cases {
            let mut a = vec![0u8; src.len()];
            apply_rgba8(&src, &mut a, params);
            let mut b = vec![0u8; src.len()];
            apply_compose_rgba8(&src, w, h, &mut b, w, h, params, TransformParams::default());
            assert_eq!(a, b, "identity compose must match apply_rgba8 for {:?}", params);
        }
    }

    /// Per-pixel reference implementation: the simple inverse-affine + bounds-checked
    /// bilinear sampler this module had before forward differencing. Used as a
    /// fixed point for the optimised slow path.
    fn reference_compose(
        src: &[u8],
        src_w: u32,
        src_h: u32,
        dst: &mut [u8],
        dst_w: u32,
        dst_h: u32,
        adjust: AdjustParams,
        transform: TransformParams,
    ) {
        let dst_cx = dst_w as f32 * 0.5;
        let dst_cy = dst_h as f32 * 0.5;
        let src_cx = src_w as f32 * 0.5;
        let src_cy = src_h as f32 * 0.5;
        let pan_x = transform.pan_x_norm * dst_w as f32;
        let pan_y = transform.pan_y_norm * dst_h as f32;
        let inv_zoom = 1.0 / transform.zoom.max(1e-4);
        let theta = -transform.rotation_deg.to_radians();
        let cos_t = theta.cos();
        let sin_t = theta.sin();
        let dst_stride = dst_w as usize * 4;
        for y in 0..dst_h as usize {
            for x in 0..dst_w as usize {
                let xo = x as f32 + 0.5;
                let yo = y as f32 + 0.5;
                let dx = xo - dst_cx - pan_x;
                let dy = yo - dst_cy - pan_y;
                let rx = dx * cos_t - dy * sin_t;
                let ry = dx * sin_t + dy * cos_t;
                let sx = rx * inv_zoom + src_cx - 0.5;
                let sy = ry * inv_zoom + src_cy - 0.5;
                let (r, g, b, a) = sample_bilinear(src, src_w, src_h, sx, sy);
                let (r, g, b, a) = if a == 0 {
                    (0, 0, 0, 0)
                } else {
                    process_pixel(r, g, b, a, adjust)
                };
                let i = y * dst_stride + x * 4;
                dst[i] = r;
                dst[i + 1] = g;
                dst[i + 2] = b;
                dst[i + 3] = a;
            }
        }
    }

    fn assert_close(actual: &[u8], expected: &[u8], tol: u8, label: &str) {
        assert_eq!(actual.len(), expected.len(), "{} length mismatch", label);
        for (i, (&a, &e)) in actual.iter().zip(expected.iter()).enumerate() {
            let diff = a.max(e) - a.min(e);
            assert!(
                diff <= tol,
                "{} mismatch at byte {}: actual={} expected={} diff={}",
                label, i, a, e, diff
            );
        }
    }

    #[test]
    fn slow_path_matches_reference() {
        let src_w = 64u32;
        let src_h = 48u32;
        let src = fixture(src_w, src_h);

        let cases: &[(AdjustParams, TransformParams, (u32, u32), &str)] = &[
            // Pure pan, default colours, same dims (exercises Change A skip).
            (
                AdjustParams::default(),
                TransformParams { rotation_deg: 0.0, zoom: 1.0, pan_x_norm: 0.1, pan_y_norm: 0.0 },
                (src_w, src_h),
                "pure pan",
            ),
            // Pan + 90°-rotated container, default colours.
            (
                AdjustParams::default(),
                TransformParams { rotation_deg: 0.0, zoom: 1.0, pan_x_norm: 0.05, pan_y_norm: -0.07 },
                (src_h, src_w),
                "pan + rotated container",
            ),
            // Rotation + zoom, default colours (Change A skip on the long path).
            (
                AdjustParams::default(),
                TransformParams { rotation_deg: 17.0, zoom: 1.3, pan_x_norm: 0.0, pan_y_norm: 0.0 },
                (src_w, src_h),
                "rotation + zoom, no colour",
            ),
            // Non-zero adjustments + non-trivial transform (exercises both paths).
            (
                AdjustParams { brightness: 0.15, contrast: -0.05, saturation: 0.2, hue: 12.0, vibrance: 0.0 },
                TransformParams { rotation_deg: 8.0, zoom: 1.1, pan_x_norm: 0.04, pan_y_norm: 0.03 },
                (src_w, src_h),
                "colour + transform",
            ),
        ];

        for (adjust, transform, (dw, dh), label) in cases {
            let mut actual = vec![0u8; (dw * dh * 4) as usize];
            let mut expected = vec![0u8; (dw * dh * 4) as usize];
            apply_compose_rgba8(&src, src_w, src_h, &mut actual, *dw, *dh, *adjust, *transform);
            reference_compose(&src, src_w, src_h, &mut expected, *dw, *dh, *adjust, *transform);
            // ±1/255 tolerance for bilinear arithmetic-ordering differences.
            assert_close(&actual, &expected, 1, label);
        }
    }
}
