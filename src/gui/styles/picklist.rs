//! Picklists style

#![allow(clippy::module_name_repetitions)]

use iced::widget::pick_list;
use iced::{Background, Color};

use crate::gui::styles::types::palette::mix_colors;
use crate::StyleType;

#[derive(Clone, Copy, Default)]
pub enum PicklistType {
    #[default]
    Standard,
}

impl iced::overlay::menu::StyleSheet for StyleType {
    type Style = PicklistType;

    fn appearance(&self, _: &Self::Style) -> iced::overlay::menu::Appearance {
        let colors = self.get_palette();
        let ext = self.get_extension();
        iced::overlay::menu::Appearance {
            text_color: colors.text_body,
            background: Background::Color(ext.buttons_color),
            border_width: 1.0,
            border_radius: 0.0.into(),
            border_color: colors.secondary,
            selected_text_color: colors.text_body,
            selected_background: Background::Color(mix_colors(ext.buttons_color, colors.primary)),
        }
    }
}

impl pick_list::StyleSheet for StyleType {
    type Style = PicklistType;

    fn active(&self, _: &Self::Style) -> pick_list::Appearance {
        let colors = self.get_palette();
        let ext = self.get_extension();
        pick_list::Appearance {
            text_color: colors.text_body,
            placeholder_color: colors.text_body,
            handle_color: colors.text_body,
            background: Background::Color(ext.buttons_color),
            border_radius: 0.0.into(),
            border_width: 1.0,
            border_color: Color {
                a: ext.alpha_round_borders,
                ..ext.buttons_color
            },
        }
    }

    fn hovered(&self, _: &Self::Style) -> pick_list::Appearance {
        let colors = self.get_palette();
        let ext = self.get_extension();
        pick_list::Appearance {
            text_color: colors.text_body,
            placeholder_color: colors.text_body,
            handle_color: colors.text_body,
            background: Background::Color(mix_colors(ext.buttons_color, colors.primary)),
            border_radius: 0.0.into(),
            border_width: 1.0,
            border_color: colors.secondary,
        }
    }
}
