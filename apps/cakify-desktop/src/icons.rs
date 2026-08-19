//! Cakify's small, consistent icon surface.
//!
//! The SVG bytes are compiled into the desktop binary so icons do not depend
//! on a working-directory-relative asset path at runtime. The source files are
//! a focused, pinned subset of Lucide Icons; see the adjacent NOTICE file.

use gpui::{prelude::*, px, svg, Rgba, Svg};

// Keep third-party notices in the standalone executable as well as packaged
// installer/portable payloads.
#[used]
static LUCIDE_LICENSE: &[u8] = include_bytes!("../../../assets/icons/lucide/LICENSE");
#[used]
static LUCIDE_NOTICE: &[u8] = include_bytes!("../../../assets/icons/lucide/NOTICE.md");

/// Icons used by the chat workspace and settings pages.
///
/// Keep this list intentionally small. Adding an icon here should correspond
/// to a visible control or status in the UI, not to a decorative glyph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IconName {
    ArrowLeft,
    ArrowUp,
    Blocks,
    Bot,
    Cable,
    Check,
    Menu,
    MessageSquare,
    Plus,
    RotateCcw,
    Search,
    Settings,
    Shield,
    Sparkles,
    Square,
    X,
}

/// Render one vendored Lucide SVG at a fixed square size.
///
/// The currentColor attribute in the upstream SVG is supplied by GPUI's text
/// color, so callers can keep icon geometry and color independent.
pub fn icon(name: IconName, size: f32, color: Rgba) -> Svg {
    svg()
        .data(icon_data(name))
        .w(px(size))
        .h(px(size))
        .text_color(color)
}

fn icon_data(name: IconName) -> &'static [u8] {
    match name {
        IconName::ArrowLeft => include_bytes!("../../../assets/icons/lucide/arrow-left.svg"),
        IconName::ArrowUp => include_bytes!("../../../assets/icons/lucide/arrow-up.svg"),
        IconName::Blocks => include_bytes!("../../../assets/icons/lucide/blocks.svg"),
        IconName::Bot => include_bytes!("../../../assets/icons/lucide/bot.svg"),
        IconName::Cable => include_bytes!("../../../assets/icons/lucide/cable.svg"),
        IconName::Check => include_bytes!("../../../assets/icons/lucide/check.svg"),
        IconName::Menu => include_bytes!("../../../assets/icons/lucide/menu.svg"),
        IconName::MessageSquare => {
            include_bytes!("../../../assets/icons/lucide/message-square.svg")
        }
        IconName::Plus => include_bytes!("../../../assets/icons/lucide/plus.svg"),
        IconName::RotateCcw => include_bytes!("../../../assets/icons/lucide/rotate-ccw.svg"),
        IconName::Search => include_bytes!("../../../assets/icons/lucide/search.svg"),
        IconName::Settings => include_bytes!("../../../assets/icons/lucide/settings.svg"),
        IconName::Shield => include_bytes!("../../../assets/icons/lucide/shield.svg"),
        IconName::Sparkles => include_bytes!("../../../assets/icons/lucide/sparkles.svg"),
        IconName::Square => include_bytes!("../../../assets/icons/lucide/square.svg"),
        IconName::X => include_bytes!("../../../assets/icons/lucide/x.svg"),
    }
}
