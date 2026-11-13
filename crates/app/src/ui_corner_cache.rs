//! Coverage depends on radius, DPI and stroke, never on position or color.
//! Retain a bounded set across frames so hundreds of identical controls do not
//! repeat the same sixteen subpixel samples for every corner tile.
use std::{cell::RefCell, collections::VecDeque, rc::Rc};

const CACHE_LIMIT: usize = 32;
type Key = (u32, u32, Option<u32>);

pub(super) struct Tile {
    pub offset: [f32; 2],
    pub coverage: f32,
}

pub(super) struct Mask {
    pub tile: f32,
    pub tiles: Vec<Tile>,
}

thread_local! {
    static CACHE: RefCell<VecDeque<(Key, Rc<Mask>)>> = const { RefCell::new(VecDeque::new()) };
}

pub(super) fn mask(radius: f32, scale: f32, stroke: Option<f32>) -> Rc<Mask> {
    let key = (radius.to_bits(), scale.to_bits(), stroke.map(f32::to_bits));
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(index) = cache.iter().position(|(old, _)| *old == key) {
            let entry = cache.remove(index).unwrap();
            let mask = entry.1.clone();
            cache.push_back(entry);
            return mask;
        }
        let steps = (radius * scale).ceil().clamp(1., 32.) as usize;
        let tile = radius / steps as f32;
        let inner_squared = stroke.map(|width| (radius - width).powi(2));
        let mut tiles = Vec::new();
        for row in 0..steps {
            for column in 0..steps {
                let dx = column as f32 * tile;
                let dy = row as f32 * tile;
                let mut covered = 0;
                for sy in 0..4 {
                    for sx in 0..4 {
                        let px = dx + (sx as f32 + 0.5) * tile / 4. - radius;
                        let py = dy + (sy as f32 + 0.5) * tile / 4. - radius;
                        let distance_squared = px * px + py * py;
                        if distance_squared <= radius * radius
                            && inner_squared.is_none_or(|inner| distance_squared >= inner)
                        {
                            covered += 1;
                        }
                    }
                }
                if covered != 0 {
                    tiles.push(Tile { offset: [dx, dy], coverage: covered as f32 / 16. });
                }
            }
        }
        let mask = Rc::new(Mask { tile, tiles });
        if cache.len() == CACHE_LIMIT { cache.pop_front(); }
        cache.push_back((key, mask.clone()));
        mask
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_reuse_coverage_but_distinguish_dpi_radius_and_stroke() {
        let fill = mask(4., 1., None);
        assert!(Rc::ptr_eq(&fill, &mask(4., 1., None)));
        for other in [mask(4., 2., None), mask(3., 1., None), mask(4., 1., Some(1.))] {
            assert!(!Rc::ptr_eq(&fill, &other));
        }
        // Never retain every DPI or resize variant for the lifetime of the app.
        for step in 1..=100 { mask(4., step as f32 / 10., None); }
        CACHE.with(|cache| assert_eq!(cache.borrow().len(), CACHE_LIMIT));
    }

    #[test]
    fn cached_masks_match_uncached_subpixel_coverage() {
        for radius in [0.5_f32, 1., 1.75, 3., 4., 6.125, 64.] {
            for scale in [1., 1.25, 1.5, 2., 3.] {
                for stroke in [None, Some(1_f32.min(radius))] {
                    let cached = mask(radius, scale, stroke);
                    let steps = (radius * scale).ceil().clamp(1., 32.) as usize;
                    let tile = radius / steps as f32;
                    let mut next = cached.tiles.iter();
                    for row in 0..steps {
                        for col in 0..steps {
                            let dx = col as f32 * tile;
                            let dy = row as f32 * tile;
                            let mut count = 0;
                            for sy in 0..4 {
                                for sx in 0..4 {
                                    let x = dx + (sx as f32 + 0.5) * tile * 0.25 - radius;
                                    let y = dy + (sy as f32 + 0.5) * tile * 0.25 - radius;
                                    let d = x * x + y * y;
                                    if d <= radius * radius
                                        && stroke.is_none_or(|s| d >= (radius - s).powi(2))
                                    { count += 1; }
                                }
                            }
                            if count != 0 {
                                let actual = next.next().expect("missing visible tile");
                                assert_eq!(actual.offset, [dx, dy]);
                                assert_eq!(actual.coverage, count as f32 / 16.);
                            }
                        }
                    }
                    assert!(next.next().is_none());
                    assert_eq!(cached.tile, tile);
                }
            }
        }
    }
}
