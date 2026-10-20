//! the Settings tab

use std::path::Path;

use egui::{RichText, ThemePreference};
use egui_material_icons::icons::{
    ICON_BRIGHTNESS_AUTO, ICON_DARK_MODE, ICON_FOLDER_OPEN, ICON_LIGHT_MODE,
};

use super::WerlenApp;
use super::style::{ERR_COLOR, SECTION_SIZE, rarity_combo, with_icon};
use crate::shop::{MAX_SCROLL_LEVEL, MAX_STOCK_COUNT, Notice};

impl WerlenApp {
    pub(super) fn settings(&mut self, ui: &mut egui::Ui) {
        let mut open_folder = false;
        egui::ScrollArea::vertical().show(ui, |ui| {
            let settings = &mut self.shop.settings;

            settings_section(ui, "Scrolls", |ui| {
                setting_label(ui, "Number of scrolls", "How many scrolls the Scrolls tab stocks");
                ui.add(egui::Slider::new(&mut settings.max_scrolls, 0..=MAX_STOCK_COUNT));
                ui.end_row();

                setting_label(
                    ui,
                    "Highest level",
                    "Scrolls are drawn from cantrips up to this level, weighted towards lower levels",
                );
                ui.add(level_slider(&mut settings.max_scroll_level));
                ui.end_row();
            });

            settings_section(ui, "Items", |ui| {
                setting_label(ui, "Number of items", "How many items the Items tab stocks");
                ui.add(egui::Slider::new(&mut settings.max_items, 0..=MAX_STOCK_COUNT));
                ui.end_row();

                setting_label(
                    ui,
                    "Highest rarity",
                    "Items are drawn from Common up to this rarity, weighted towards lower rarities",
                );
                rarity_combo(ui, "max_item_rarity", &mut settings.max_item_rarity, ..);
                ui.end_row();
            });

            settings_section(ui, "Specials", |ui| {
                setting_label(
                    ui,
                    "Number of specials",
                    "How many entries the Specials tab stocks, from both items and scrolls",
                );
                ui.add(egui::Slider::new(&mut settings.max_specials, 0..=MAX_STOCK_COUNT));
                ui.end_row();

                setting_label(
                    ui,
                    "Item rarities",
                    "Special items are drawn from this rarity range",
                );
                ui.horizontal(|ui| {
                    // min/max special rarity can't cross each other
                    let max = settings.max_special_rarity;
                    rarity_combo(
                        ui,
                        "min_special_rarity",
                        &mut settings.min_special_rarity,
                        ..=max,
                    );
                    ui.label("to");
                    let min = settings.min_special_rarity;
                    rarity_combo(
                        ui,
                        "max_special_rarity",
                        &mut settings.max_special_rarity,
                        min..,
                    );
                });
                ui.end_row();

                setting_label(
                    ui,
                    "Lowest scroll level",
                    "Special scrolls are drawn from this level up to 9",
                );
                ui.add(level_slider(&mut settings.min_special_scroll_level));
                ui.end_row();
            });

            settings_section(ui, "Appearance", |ui| {
                setting_label(ui, "Theme", "Follow the system theme, or force light or dark");
                theme_buttons(ui);
                ui.end_row();
            });

            settings_section(ui, "Database", |ui| {
                setting_label(
                    ui,
                    "Location",
                    "Catalogue, settings, and the last shop are stored here",
                );
                ui.horizontal(|ui| match &self.shop.db_path {
                    Some(path) => {
                        ui.monospace(path.display().to_string());
                        open_folder = ui.button(with_icon(ICON_FOLDER_OPEN, "Open folder")).clicked();
                    }
                    None => {
                        ui.colored_label(ERR_COLOR, "in memory — changes won't be saved");
                    }
                });
                ui.end_row();
            });
        });

        if open_folder
            && let Some(dir) = self.shop.db_path.as_deref().and_then(Path::parent)
            && let Err(err) = open_in_file_manager(dir)
        {
            self.shop.notice = Some(Notice::Error(format!("couldn't open folder: {err}")));
        }
    }
}

/// a titled block of two-column settings rows
fn settings_section(ui: &mut egui::Ui, title: &str, add_rows: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(6.0);
    ui.label(RichText::new(title).strong().size(SECTION_SIZE));
    ui.separator();
    egui::Grid::new(title)
        .num_columns(2)
        .min_col_width(180.0)
        .spacing([24.0, 10.0])
        .show(ui, add_rows);
    ui.add_space(10.0);
}

fn setting_label(ui: &mut egui::Ui, text: &str, help: &str) {
    ui.label(text).on_hover_text(help);
}

/// 0..=9 spell level slider that shows 0 as "Cantrip"
fn level_slider(level: &mut usize) -> egui::Slider<'_> {
    egui::Slider::new(level, 0..=MAX_SCROLL_LEVEL)
        .custom_formatter(|n, _| {
            if n == 0.0 {
                "Cantrip".to_string()
            } else {
                n.to_string()
            }
        })
        .custom_parser(|text| {
            if text.trim().eq_ignore_ascii_case("cantrip") {
                Some(0.0)
            } else {
                text.trim().parse().ok()
            }
        })
}

/// system/dark/light theme picker
fn theme_buttons(ui: &mut egui::Ui) {
    let current = ui.ctx().options(|options| options.theme_preference);
    let mut preference = current;
    ui.horizontal(|ui| {
        for (value, icon, label) in [
            (ThemePreference::System, ICON_BRIGHTNESS_AUTO, "System"),
            (ThemePreference::Dark, ICON_DARK_MODE, "Dark"),
            (ThemePreference::Light, ICON_LIGHT_MODE, "Light"),
        ] {
            ui.selectable_value(&mut preference, value, with_icon(icon, label));
        }
    });
    if preference != current {
        ui.ctx().set_theme(preference);
    }
}

/// open a directory in the platform's file manager
fn open_in_file_manager(dir: &Path) -> std::io::Result<()> {
    let program = if cfg!(target_os = "windows") {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(program)
        .arg(dir)
        .spawn()
        .map(drop)
}
