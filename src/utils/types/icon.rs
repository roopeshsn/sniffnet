use iced::widget::Text;
use iced::Renderer;

use crate::gui::styles::style_constants::ICONS;
use crate::StyleType;

pub enum Icon {
    ArrowBack,
    ArrowLeft,
    ArrowRight,
    ArrowsDown,
    AudioHigh,
    AudioMute,
    Bin,
    BytesThreshold,
    Clock,
    Copy,
    Dots,
    Error,
    File,
    Forbidden,
    Funnel,
    GitHub,
    Globe,
    HalfSun,
    Hourglass1,
    Hourglass2,
    Hourglass3,
    Inspect,
    Lightning,
    Notification,
    Overview,
    PacketsThreshold,
    // Restore,
    Rocket,
    Settings,
    Sniffnet,
    Star,
    Warning,
    Waves,
}

impl Icon {
    pub fn to_text(&self) -> iced::advanced::widget::Text<'static, Renderer<StyleType>> {
        Text::new(match self {
            Icon::ArrowBack => "C",
            Icon::ArrowLeft => "i",
            Icon::ArrowRight => "j",
            Icon::ArrowsDown => ":",
            Icon::AudioHigh => "Z",
            Icon::AudioMute => "Y",
            Icon::Bin => "h",
            Icon::BytesThreshold => "f",
            Icon::Clock => "9",
            Icon::Dots => "R",
            Icon::Error => "U",
            Icon::File => "8",
            Icon::Forbidden => "x",
            Icon::Funnel => "V",
            Icon::GitHub => "H",
            Icon::Globe => "c",
            Icon::HalfSun => "K",
            Icon::Hourglass1 => "1",
            Icon::Hourglass2 => "2",
            Icon::Hourglass3 => "3",
            Icon::Inspect => "5",
            Icon::Lightning => "z",
            Icon::Notification => "7",
            Icon::Overview => "d",
            Icon::PacketsThreshold => "e",
            // Icon::Restore => "k",
            Icon::Rocket => "S",
            Icon::Settings => "a",
            Icon::Sniffnet => "A",
            Icon::Star => "g",
            Icon::Warning => "T",
            Icon::Waves => "y",
            Icon::Copy => "u",
        })
        .font(ICONS)
    }

    pub fn get_hourglass(num: usize) -> iced::advanced::widget::Text<'static, Renderer<StyleType>> {
        match num {
            1 => Icon::Hourglass1.to_text(),
            2 => Icon::Hourglass2.to_text(),
            3 => Icon::Hourglass3.to_text(),
            _ => Text::new(""),
        }
    }
}
