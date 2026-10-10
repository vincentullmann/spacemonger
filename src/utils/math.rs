//! Small interpolation helpers.

/// Geometric interpolation from `a` to `b` at `t` (even steps in log space; for scales).
pub fn geo_lerp(a: f64, b: f64, t: f64) -> f64 {
    a * (b / a).powf(t)
}

/// Cubic ease-out: fast start, gentle stop.
pub fn ease_out_cubic(t: f64) -> f64 {
    1.0 - (1.0 - t).powi(3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolation() {
        assert!((geo_lerp(1.0, 4.0, 0.5) - 2.0).abs() < 1e-12);
        assert_eq!(ease_out_cubic(0.0), 0.0);
        assert_eq!(ease_out_cubic(1.0), 1.0);
    }
}
