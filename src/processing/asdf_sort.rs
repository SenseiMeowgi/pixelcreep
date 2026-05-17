#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AsdfSortMode {
    White,
    Black,
    Bright,
    Dark,
}

impl Default for AsdfSortMode {
    fn default() -> Self {
        Self::White
    }
}

/// Parameters for Kim Asendorf's 2010 ASDF pixel sort.
#[derive(Clone, Copy, Debug)]
pub struct AsdfSortParams {
    pub mode: AsdfSortMode,
    pub loops: u32,
    pub white_value: i32,
    pub black_value: i32,
    pub bright_value: u8,
    pub dark_value: u8,
}

impl Default for AsdfSortParams {
    fn default() -> Self {
        Self {
            mode: AsdfSortMode::White,
            loops: 1,
            white_value: -12_345_678,
            black_value: -3_456_789,
            bright_value: 127,
            dark_value: 223,
        }
    }
}

/// Faithful Rust port of the ASDF Processing sketch: each loop sorts columns
/// first, then rows, using the same signed RGB threshold comparisons.
pub fn asdf_sort(rgba: &mut [u8], width: u32, height: u32, params: AsdfSortParams) {
    let w = width as usize;
    let h = height as usize;
    let pixel_count = w.saturating_mul(h);
    if w == 0 || h == 0 || rgba.len() < pixel_count.saturating_mul(4) {
        return;
    }

    for _ in 0..params.loops.max(1) {
        for x in 0..w.saturating_sub(1) {
            sort_column(rgba, w, h, x, params);
        }
        for y in 0..h.saturating_sub(1) {
            sort_row(rgba, w, y, params);
        }
    }
}

fn sort_row(rgba: &mut [u8], width: usize, y: usize, params: AsdfSortParams) {
    let mut x = 0usize;
    let mut x_end = 0usize;

    while x_end < width.saturating_sub(1) {
        let Some(start) = first_x(rgba, width, x, y, params) else {
            break;
        };
        x = start;
        x_end = next_x(rgba, width, x, y, params);
        sort_row_span(rgba, width, y, x, x_end);
        x = x_end.saturating_add(1);
    }
}

fn sort_column(rgba: &mut [u8], width: usize, height: usize, x: usize, params: AsdfSortParams) {
    let mut y = 0usize;
    let mut y_end = 0usize;

    while y_end < height.saturating_sub(1) {
        let Some(start) = first_y(rgba, width, height, x, y, params) else {
            break;
        };
        y = start;
        y_end = next_y(rgba, width, height, x, y, params);
        sort_column_span(rgba, width, x, y, y_end);
        y = y_end.saturating_add(1);
    }
}

fn first_x(
    rgba: &[u8],
    width: usize,
    mut x: usize,
    y: usize,
    params: AsdfSortParams,
) -> Option<usize> {
    while x < width && skip_until_start(pixel(rgba, width, x, y), params) {
        x += 1;
    }
    (x < width).then_some(x)
}

fn next_x(rgba: &[u8], width: usize, x: usize, y: usize, params: AsdfSortParams) -> usize {
    let mut next = x.saturating_add(1);
    if next >= width {
        return width.saturating_sub(1);
    }
    while next < width && continue_until_end(pixel(rgba, width, next, y), params) {
        next += 1;
    }
    if next >= width {
        width.saturating_sub(1)
    } else {
        next.saturating_sub(1)
    }
}

fn first_y(
    rgba: &[u8],
    width: usize,
    height: usize,
    x: usize,
    mut y: usize,
    params: AsdfSortParams,
) -> Option<usize> {
    while y < height && skip_until_start(pixel(rgba, width, x, y), params) {
        y += 1;
    }
    (y < height).then_some(y)
}

fn next_y(
    rgba: &[u8],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    params: AsdfSortParams,
) -> usize {
    let mut next = y.saturating_add(1);
    if next >= height {
        return height.saturating_sub(1);
    }
    while next < height && continue_until_end(pixel(rgba, width, x, next), params) {
        next += 1;
    }
    if next >= height {
        height.saturating_sub(1)
    } else {
        next.saturating_sub(1)
    }
}

fn sort_row_span(rgba: &mut [u8], width: usize, y: usize, start: usize, end_exclusive: usize) {
    if end_exclusive <= start {
        return;
    }

    let mut colors = Vec::with_capacity(end_exclusive - start);
    for x in start..end_exclusive {
        colors.push(pixel(rgba, width, x, y));
    }
    colors.sort_by_key(|color| processing_color_int(*color));
    for (i, color) in colors.into_iter().enumerate() {
        set_pixel(rgba, width, start + i, y, color);
    }
}

fn sort_column_span(rgba: &mut [u8], width: usize, x: usize, start: usize, end_exclusive: usize) {
    if end_exclusive <= start {
        return;
    }

    let mut colors = Vec::with_capacity(end_exclusive - start);
    for y in start..end_exclusive {
        colors.push(pixel(rgba, width, x, y));
    }
    colors.sort_by_key(|color| processing_color_int(*color));
    for (i, color) in colors.into_iter().enumerate() {
        set_pixel(rgba, width, x, start + i, color);
    }
}

#[inline]
fn skip_until_start(color: [u8; 4], params: AsdfSortParams) -> bool {
    match params.mode {
        AsdfSortMode::White => processing_color_int(color) < params.white_value,
        AsdfSortMode::Black => processing_color_int(color) > params.black_value,
        AsdfSortMode::Bright => brightness(color) < params.bright_value,
        AsdfSortMode::Dark => brightness(color) > params.dark_value,
    }
}

#[inline]
fn continue_until_end(color: [u8; 4], params: AsdfSortParams) -> bool {
    match params.mode {
        AsdfSortMode::White => processing_color_int(color) > params.white_value,
        AsdfSortMode::Black => processing_color_int(color) < params.black_value,
        AsdfSortMode::Bright => brightness(color) > params.bright_value,
        AsdfSortMode::Dark => brightness(color) < params.dark_value,
    }
}

#[inline]
fn pixel(rgba: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
    let base = (y * width + x) * 4;
    [rgba[base], rgba[base + 1], rgba[base + 2], rgba[base + 3]]
}

#[inline]
fn set_pixel(rgba: &mut [u8], width: usize, x: usize, y: usize, color: [u8; 4]) {
    let base = (y * width + x) * 4;
    rgba[base..base + 4].copy_from_slice(&color);
}

#[inline]
fn processing_color_int(color: [u8; 4]) -> i32 {
    (0xff00_0000u32
        | (u32::from(color[0]) << 16)
        | (u32::from(color[1]) << 8)
        | u32::from(color[2])) as i32
}

#[inline]
fn brightness(color: [u8; 4]) -> u8 {
    color[0].max(color[1]).max(color[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(values: &[(u8, u8, u8, u8)]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|&(r, g, b, a)| [r, g, b, a])
            .collect()
    }

    fn params(mode: AsdfSortMode) -> AsdfSortParams {
        AsdfSortParams {
            mode,
            loops: 1,
            white_value: -12_345_678,
            black_value: -3_456_789,
            bright_value: 127,
            dark_value: 223,
        }
    }

    #[test]
    fn white_mode_sorts_detected_row_interval_with_processing_endpoint() {
        let mut rgba = rgb(&[
            (80, 0, 0, 255),
            (250, 0, 0, 10),
            (210, 0, 0, 20),
            (230, 0, 0, 30),
        ]);

        asdf_sort(&mut rgba, 4, 2, params(AsdfSortMode::White));

        assert_eq!(
            rgba,
            rgb(&[
                (80, 0, 0, 255),
                (210, 0, 0, 20),
                (250, 0, 0, 10),
                (230, 0, 0, 30),
            ])
        );
    }

    #[test]
    fn column_pass_runs_before_row_pass() {
        let mut rgba = rgb(&[
            (250, 0, 0, 255),
            (20, 0, 0, 255),
            (210, 0, 0, 255),
            (20, 0, 0, 255),
            (230, 0, 0, 255),
            (20, 0, 0, 255),
            (240, 0, 0, 255),
            (20, 0, 0, 255),
        ]);

        asdf_sort(&mut rgba, 2, 4, params(AsdfSortMode::White));

        assert_eq!(
            rgba,
            rgb(&[
                (210, 0, 0, 255),
                (20, 0, 0, 255),
                (230, 0, 0, 255),
                (20, 0, 0, 255),
                (250, 0, 0, 255),
                (20, 0, 0, 255),
                (240, 0, 0, 255),
                (20, 0, 0, 255),
            ])
        );
    }

    #[test]
    fn black_mode_uses_signed_rgb_threshold() {
        let mut rgba = rgb(&[
            (220, 0, 0, 255),
            (10, 0, 0, 255),
            (40, 0, 0, 255),
            (20, 0, 0, 255),
        ]);
        let mut p = params(AsdfSortMode::Black);
        p.black_value = -14_000_000;

        asdf_sort(&mut rgba, 4, 2, p);

        assert_eq!(
            rgba,
            rgb(&[
                (220, 0, 0, 255),
                (10, 0, 0, 255),
                (40, 0, 0, 255),
                (20, 0, 0, 255),
            ])
        );
    }

    #[test]
    fn bright_and_dark_modes_use_max_channel_brightness() {
        let mut bright = rgb(&[
            (10, 10, 10, 255),
            (140, 10, 10, 255),
            (130, 10, 10, 255),
            (150, 10, 10, 255),
        ]);
        asdf_sort(&mut bright, 4, 2, params(AsdfSortMode::Bright));
        assert_eq!(
            bright,
            rgb(&[
                (10, 10, 10, 255),
                (130, 10, 10, 255),
                (140, 10, 10, 255),
                (150, 10, 10, 255),
            ])
        );

        let mut dark = rgb(&[
            (250, 250, 250, 255),
            (10, 10, 10, 255),
            (40, 40, 40, 255),
            (20, 20, 20, 255),
        ]);
        asdf_sort(&mut dark, 4, 2, params(AsdfSortMode::Dark));
        assert_eq!(
            dark,
            rgb(&[
                (250, 250, 250, 255),
                (10, 10, 10, 255),
                (40, 40, 40, 255),
                (20, 20, 20, 255),
            ])
        );
    }

    #[test]
    fn multiple_loops_run_without_changing_buffer_shape() {
        let mut rgba = rgb(&[
            (250, 0, 0, 1),
            (210, 0, 0, 2),
            (230, 0, 0, 3),
            (220, 0, 0, 4),
        ]);
        let mut p = params(AsdfSortMode::White);
        p.loops = 3;

        asdf_sort(&mut rgba, 4, 2, p);

        assert_eq!(rgba.len(), 16);
    }
}
