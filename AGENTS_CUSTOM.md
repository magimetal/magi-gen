# AGENTS_CUSTOM.md

- rustychroma 0.2.0 `remove` threshold type is `f64`, not `f32`.
- rustychroma 0.2.0 `erode` signature is `erode(src: &[u8], dst: &mut [u8], width: usize, height: usize)`, so clone RGBA buffer for destination before eroding.
