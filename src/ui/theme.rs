//! Native-macOS-feel color + radius tokens. Dark appearance only (matches the
//! current product look). Surfaces are intentionally translucent so the
//! window's NSVisualEffectView blur reads through.

use gpui::{Hsla, hsla};

pub fn sidebar_tint() -> Hsla {
    hsla(0.0, 0.0, 1.0, 0.04)
}
pub fn detail_tint() -> Hsla {
    hsla(0.0, 0.0, 0.0, 0.10)
}
pub fn card_bg() -> Hsla {
    hsla(0.0, 0.0, 1.0, 0.05)
}
pub fn row_hover() -> Hsla {
    hsla(0.0, 0.0, 1.0, 0.06)
}
pub fn row_active() -> Hsla {
    hsla(0.0, 0.0, 1.0, 0.10)
}
pub fn pill_bg() -> Hsla {
    hsla(0.0, 0.0, 0.06, 0.72)
}
pub fn pill_border() -> Hsla {
    hsla(0.0, 0.0, 1.0, 0.08)
}
pub fn divider() -> Hsla {
    hsla(0.0, 0.0, 1.0, 0.08)
}

pub fn text_primary() -> Hsla {
    hsla(0.0, 0.0, 0.97, 1.0)
}
pub fn text_secondary() -> Hsla {
    hsla(0.0, 0.0, 0.97, 0.62)
}
pub fn text_tertiary() -> Hsla {
    hsla(0.0, 0.0, 0.97, 0.40)
}

pub fn good() -> Hsla {
    hsla(0.36, 0.55, 0.55, 1.0)
}
pub fn warn() -> Hsla {
    hsla(0.10, 0.85, 0.60, 1.0)
}

pub fn vu_idle() -> Hsla {
    hsla(0.0, 0.0, 1.0, 0.28)
}
