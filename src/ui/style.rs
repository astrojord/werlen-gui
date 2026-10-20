//! fonts, text sizes, colours, and small widgets shared across tabs

use egui::epaint::text::{FontInsert, FontPriority, InsertFontFamily};
use egui::{Color32, FontData, FontFamily, RichText};
use egui_material_icons::MaterialIcon;
use egui_material_icons::icons::ICON_WAND_STARS;

use crate::shop::Rarity;

/// a colour with a variant for each theme
#[derive(Clone, Copy)]
pub(super) struct Themed {
    dark: Color32,
    light: Color32,
}

impl Themed {
    const fn rgb(dark: (u8, u8, u8), light: (u8, u8, u8)) -> Self {
        Self {
            dark: Color32::from_rgb(dark.0, dark.1, dark.2),
            light: Color32::from_rgb(light.0, light.1, light.2),
        }
    }

    /// the variant for `ui`'s current theme
    pub(super) fn get(self, ui: &egui::Ui) -> Color32 {
        if ui.visuals().dark_mode {
            self.dark
        } else {
            self.light
        }
    }
}

/// error red, legible on both themes as is
pub(super) const ERR_COLOR: Color32 = Color32::from_rgb(230, 80, 80);
pub(super) const OK_COLOR: Themed = Themed::rgb((90, 190, 110), (30, 130, 55));
const BANNER_COLOR: Themed = Themed::rgb((185, 140, 255), (105, 55, 180));
/// fill for the primary Generate button (its text is black on either theme)
const ACCENT: Color32 = Color32::from_rgb(255, 200, 100);

/// text sizes for W95FA, which runs small for its nominal size
const TEXT_SIZE: f32 = 12.5;
const SMALL_SIZE: f32 = 11.0;
const HEADING_SIZE: f32 = 20.0;
pub(super) const SECTION_SIZE: f32 = 17.0;
pub(super) const TAB_SIZE: f32 = 15.0;
const GENERATE_SIZE: f32 = 18.0;
const FONT_W95FA: &[u8] = include_bytes!("../../assets/fonts/w95fa.otf");

/// register the W95FA text font and Material icons, and set text style sizes.
/// The monospace font (used by the banner) is left alone
pub fn setup_fonts(ctx: &egui::Context) {
    egui_material_icons::initialize(ctx);
    ctx.add_font(FontInsert::new(
        "w95fa",
        FontData::from_static(FONT_W95FA),
        vec![InsertFontFamily {
            family: FontFamily::Proportional,
            priority: FontPriority::Highest,
        }],
    ));
    ctx.all_styles_mut(|style| {
        use egui::TextStyle;
        for (text_style, size) in [
            (TextStyle::Small, SMALL_SIZE),
            (TextStyle::Body, TEXT_SIZE),
            (TextStyle::Button, TEXT_SIZE),
            (TextStyle::Heading, HEADING_SIZE),
        ] {
            style
                .text_styles
                .insert(text_style, egui::FontId::proportional(size));
        }
    });
}

/// "<icon> text", for buttons and labels; the icon font is a fallback for
/// ordinary text, so the codepoint renders inline
pub(super) fn with_icon(icon: MaterialIcon, text: &str) -> String {
    format!("{} {text}", icon.codepoint)
}

/// the block-letter WERLEN'S banner from the TUI
const BANNER: &str = include_str!("../../assets/werlen_ascii.txt");
const BANNER_SIZE: f32 = 12.0;

/// draw [`BANNER`] in purple, with lines packed tight so the block characters join up
pub(super) fn banner(ui: &mut egui::Ui) {
    let color = BANNER_COLOR.get(ui);
    let format = |color| egui::TextFormat {
        font_id: egui::FontId::monospace(BANNER_SIZE),
        color,
        line_height: Some(BANNER_SIZE),
        ..Default::default()
    };
    // the ░ background shading is drawn faint so the letters stand out
    let mut job = egui::text::LayoutJob::default();
    for ch in BANNER.trim_end().chars() {
        let color = if ch == '░' {
            color.gamma_multiply(0.25)
        } else {
            color
        };
        job.append(ch.encode_utf8(&mut [0; 4]), 0.0, format(color));
    }
    ui.label(job).on_hover_text("Werlen's Ware Generator");
}

/// the big accent-coloured Generate shop button
pub(super) fn generate_button(ui: &mut egui::Ui) -> egui::Response {
    let button = egui::Button::new(
        RichText::new(with_icon(ICON_WAND_STARS, "Generate shop"))
            .size(GENERATE_SIZE)
            .strong()
            .color(Color32::BLACK),
    )
    .fill(ACCENT)
    .min_size(egui::vec2(160.0, 34.0));
    ui.add(button)
}

/// rarity dropdown restricted to the given range
pub(super) fn rarity_combo(
    ui: &mut egui::Ui,
    id: &str,
    value: &mut Rarity,
    allowed: impl std::ops::RangeBounds<Rarity>,
) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(RichText::new(value.to_string()).color(rarity_color(*value).get(ui)))
        .show_ui(ui, |ui| {
            for rarity in Rarity::ALL.into_iter().filter(|r| allowed.contains(r)) {
                let text = RichText::new(rarity.to_string()).color(rarity_color(rarity).get(ui));
                ui.selectable_value(value, rarity, text);
            }
        });
}

/// badge and label colour for a rarity
pub(super) fn rarity_color(rarity: Rarity) -> Themed {
    match rarity {
        Rarity::Common => Themed::rgb((185, 190, 200), (95, 100, 110)),
        Rarity::Uncommon => Themed::rgb((80, 200, 100), (25, 125, 45)),
        Rarity::Rare => Themed::rgb((70, 170, 240), (15, 100, 175)),
        Rarity::VeryRare => Themed::rgb((195, 120, 240), (125, 45, 175)),
        Rarity::Legendary => Themed::rgb((240, 175, 50), (170, 100, 0)),
    }
}

/// scroll levels share the rarity palette, in bands
pub(super) fn spell_color(level: usize) -> Themed {
    let rarity = match level {
        0 | 1 => Rarity::Common,
        2 | 3 => Rarity::Uncommon,
        4 | 5 => Rarity::Rare,
        6..=8 => Rarity::VeryRare,
        _ => Rarity::Legendary,
    };
    rarity_color(rarity)
}
