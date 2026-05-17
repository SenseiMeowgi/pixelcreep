use std::collections::HashSet;

use rand::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortalDirection {
    Left,
    Right,
    Up,
    Down,
}

impl Default for PortalDirection {
    fn default() -> Self {
        Self::Down
    }
}

/// Parameters for the "Sort Pixel Portal" column sort (Jeff Thompson, 2013).
#[derive(Clone, Copy, Debug)]
pub struct PortalSortParams {
    pub max_iterations: u32,
    pub dist: u32,
    pub margin: u8,
    pub mark_seeds: bool,
    pub direction: PortalDirection,
}

impl Default for PortalSortParams {
    fn default() -> Self {
        Self {
            max_iterations: 2000,
            dist: 200,
            margin: 50,
            mark_seeds: false,
            direction: PortalDirection::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PortalSortStats {
    pub iterations: u32,
    pub traversed: usize,
}

/// Sort pixel rows/columns based on direction, re-seed from the last sorted color (portal walk).
pub fn portal_sort(
    rgba: &mut [u8],
    width: u32,
    height: u32,
    params: PortalSortParams,
) -> PortalSortStats {
    let w = width as usize;
    let h = height as usize;
    let pixel_count = w * h;
    if pixel_count == 0 || rgba.len() < pixel_count * 4 {
        return PortalSortStats::default();
    }

    let dist = params.dist.max(1) as usize;
    let margin = params.margin as i32;
    let max_iterations = params.max_iterations.max(1);

    let mut traversed = HashSet::with_capacity(pixel_count.min(1024 * 1024));
    let mut rng = rand::rng();
    let mut stats = PortalSortStats::default();

    let (step, forward) = match params.direction {
        PortalDirection::Down => (w, true),
        PortalDirection::Up => (w, false),
        PortalDirection::Right => (1, true),
        PortalDirection::Left => (1, false),
    };

    let is_horizontal = matches!(params.direction, PortalDirection::Right | PortalDirection::Left);

    let mut pos = rng.random_range(0..pixel_count);

    for _ in 0..max_iterations {
        stats.iterations += 1;

        let mut colors: Vec<[u8; 4]> = Vec::with_capacity(dist);
        let mut path: Vec<usize> = Vec::with_capacity(dist);

        let start_row = pos / w;

        for i in 0..dist {
            let idx = if forward {
                pos + i * step
            } else {
                match pos.checked_sub(i * step) {
                    Some(v) => v,
                    None => break,
                }
            };

            if idx >= pixel_count {
                break;
            }
            if is_horizontal && idx / w != start_row {
                break;
            }
            if traversed.contains(&idx) {
                break;
            }

            let base = idx * 4;
            colors.push([rgba[base], rgba[base + 1], rgba[base + 2], rgba[base + 3]]);
            path.push(idx);
            traversed.insert(idx);
        }

        if colors.is_empty() {
            pos = rng.random_range(0..pixel_count);
            continue;
        }

        colors.sort_by_key(color_sort_key);

        for (idx, &px) in path.iter().enumerate() {
            let base = px * 4;
            if params.mark_seeds && idx == 0 {
                rgba[base] = 255;
                rgba[base + 1] = 0;
                rgba[base + 2] = 0;
                rgba[base + 3] = 255;
            } else {
                rgba[base..base + 4].copy_from_slice(&colors[idx]);
            }
        }

        let last = *colors.last().unwrap();
        let candidates = find_next_candidates(rgba, pixel_count, last, margin, &traversed);
        if candidates.is_empty() {
            break;
        }
        pos = candidates[rng.random_range(0..candidates.len())];
    }

    stats.traversed = traversed.len();
    stats
}

#[inline]
fn color_sort_key(c: &[u8; 4]) -> u32 {
    (u32::from(c[0]) * 299 + u32::from(c[1]) * 587 + u32::from(c[2]) * 114) / 1000
}

fn find_next_candidates(
    rgba: &[u8],
    pixel_count: usize,
    seed: [u8; 4],
    margin: i32,
    traversed: &HashSet<usize>,
) -> Vec<usize> {
    let r = i32::from(seed[0]);
    let g = i32::from(seed[1]);
    let b = i32::from(seed[2]);

    let mut results = Vec::new();
    for i in 0..pixel_count {
        if traversed.contains(&i) {
            continue;
        }
        let base = i * 4;
        let tr = i32::from(rgba[base]);
        let tg = i32::from(rgba[base + 1]);
        let tb = i32::from(rgba[base + 2]);
        if (r - tr).abs() < margin && (g - tg).abs() < margin && (b - tb).abs() < margin {
            results.push(i);
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portal_sort_runs_on_tiny_image() {
        let mut rgba = vec![
            10, 10, 10, 255, 20, 20, 20, 255, 30, 30, 30, 255, 40, 40, 40, 255,
        ];
        let stats = portal_sort(
            &mut rgba,
            2,
            2,
            PortalSortParams {
                max_iterations: 5,
                dist: 2,
                margin: 30,
                mark_seeds: false,
                direction: PortalDirection::Down,
            },
        );
        assert!(stats.iterations > 0);
    }
}
