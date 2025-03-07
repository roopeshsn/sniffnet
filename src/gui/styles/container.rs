//! Containers style

#![allow(clippy::module_name_repetitions)]

use iced::widget::container::Appearance;
use iced::{Background, Color};

use crate::gui::styles::style_constants::{BORDER_ROUNDED_RADIUS, BORDER_WIDTH};
use crate::gui::styles::types::gradient_type::{get_gradient_headers, GradientType};
use crate::StyleType;

#[derive(Clone, Copy, Default)]
pub enum ContainerType {
    #[default]
    Standard,
    BorderedRound,
    Tooltip,
    Badge,
    Palette,
    Neutral,
    Gradient(GradientType),
    Modal,
}

impl iced::widget::container::StyleSheet for StyleType {
    type Style = ContainerType;

    fn appearance(&self, style: &Self::Style) -> Appearance {
        let colors = self.get_palette();
        let ext = self.get_extension();
        Appearance {
            text_color: Some(match style {
                ContainerType::Gradient(_) => colors.text_headers,
                _ => colors.text_body,
            }),
            background: Some(match style {
                ContainerType::Gradient(GradientType::None) => Background::Color(colors.secondary),
                ContainerType::Tooltip => Background::Color(ext.buttons_color),
                ContainerType::BorderedRound => Background::Color(Color {
                    a: ext.alpha_round_containers,
                    ..ext.buttons_color
                }),
                ContainerType::Neutral | ContainerType::Palette => {
                    Background::Color(Color::TRANSPARENT)
                }
                ContainerType::Badge => Background::Color(Color {
                    a: ext.alpha_chart_badge,
                    ..colors.secondary
                }),
                ContainerType::Gradient(gradient_type) => Background::Gradient(
                    get_gradient_headers(&colors, *gradient_type, ext.is_nightly),
                ),
                ContainerType::Modal => Background::Color(colors.primary),
                ContainerType::Standard => Background::Color(Color::TRANSPARENT),
            }),
            border_radius: match style {
                ContainerType::BorderedRound => BORDER_ROUNDED_RADIUS.into(),
                ContainerType::Modal => {
                    [0.0, 0.0, BORDER_ROUNDED_RADIUS, BORDER_ROUNDED_RADIUS].into()
                }
                ContainerType::Tooltip => 7.0.into(),
                ContainerType::Badge => 100.0.into(),
                _ => 0.0.into(),
            },
            border_width: match style {
                ContainerType::Standard
                | ContainerType::Modal
                | ContainerType::Neutral
                | ContainerType::Gradient(_) => 0.0,
                ContainerType::Tooltip => BORDER_WIDTH / 2.0,
                ContainerType::BorderedRound => BORDER_WIDTH * 2.0,
                _ => BORDER_WIDTH,
            },
            border_color: match style {
                ContainerType::Palette => Color::BLACK,
                _ => Color {
                    a: ext.alpha_round_borders,
                    ..ext.buttons_color
                },
            },
        }
    }
}
