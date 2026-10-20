use std::path::Path;

use egui::{Color32, Key, RichText};
use egui_extras::{Column, TableBuilder};

use crate::shop::{MAX_SCROLL_LEVEL, POOL_MAX, Rarity, Shop, StockItem, StockStatus};

const OK_COLOR: Color32 = Color32::from_rgb(90, 190, 110);
const ERR_COLOR: Color32 = Color32::from_rgb(230, 80, 80);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Tab {
    #[default]
    Scrolls,
    Items,
    Specials,
    Settings,
}

impl Tab {
    const ALL: [Tab; 4] = [Tab::Scrolls, Tab::Items, Tab::Specials, Tab::Settings];

    fn title(self) -> &'static str {
        match self {
            Tab::Scrolls => "Scrolls",
            Tab::Items => "Items",
            Tab::Specials => "Specials",
            Tab::Settings => "Settings",
        }
    }
}

pub struct WerlenApp {
    shop: Shop,
    tab: Tab,
    /// Show the "generate before loading" popup?
    stock_error: bool,
    /// Editable copy of the stock source path, applied on reload
    stock_source_text: String,
}

impl Default for WerlenApp {
    fn default() -> Self {
        Self {
            shop: Shop::open(),
            tab: Tab::default(),
            stock_error: false,
            stock_source_text: String::new(),
        }
    }
}

impl WerlenApp {
    fn reload_stock(&mut self) {
        self.shop
            .import_csv(Path::new(self.stock_source_text.trim()));
    }

    fn generate(&mut self) {
        if !self.shop.generate_shop() {
            self.stock_error = true;
        }
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.heading(RichText::new("Werlen's Ware Generator 🧙").strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button("🎲 Generate")
                    .on_hover_text("Generate a new shop")
                    .clicked()
                {
                    self.generate();
                }
                if ui
                    .button("⟳ Reload stock")
                    .on_hover_text("Reload the stock source CSV")
                    .clicked()
                {
                    self.reload_stock();
                }
            });
        });

        let (scrolls, items) = self.shop.pool_sizes();
        let (status_text, status_color) = match &self.shop.status {
            StockStatus::Ready if scrolls + items == 0 => (
                "● Database empty — import a CSV to add stock".to_string(),
                ERR_COLOR,
            ),
            StockStatus::Ready => (
                format!("● Database: {scrolls} scrolls / {items} items"),
                OK_COLOR,
            ),
            StockStatus::Imported(summary) => (
                format!(
                    "● Imported CSV — {} added, {} updated. Database: {scrolls} scrolls / {items} items",
                    summary.added, summary.updated
                ),
                OK_COLOR,
            ),
            StockStatus::Error(msg) => (format!("✕ Load error: {msg}"), ERR_COLOR),
        };
        ui.label(RichText::new(status_text).color(status_color));

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            for tab in Tab::ALL {
                let label = RichText::new(tab.title()).size(15.0);
                ui.selectable_value(&mut self.tab, tab, label);
            }
        });
        ui.add_space(4.0);
    }

    fn content(&mut self, ui: &mut egui::Ui) {
        let (stock, level_header, kind) = match self.tab {
            Tab::Settings => return self.settings(ui),
            Tab::Scrolls => (&self.shop.scroll_stock, "Level", "scroll"),
            Tab::Items => (&self.shop.item_stock, "Rarity", "item"),
            Tab::Specials => (&self.shop.special_stock, "Level/Rarity", "special stock"),
        };

        if !self.shop.has_stock() {
            ui.centered_and_justified(|ui| {
                ui.weak(format!(
                    "Click Generate to generate a shop and see {kind} inventory here."
                ));
            });
            return;
        }

        stock_table(ui, self.tab, stock, level_header);
    }

    fn settings(&mut self, ui: &mut egui::Ui) {
        let settings = &mut self.shop.settings;
        let mut load = false;
        egui::Grid::new("settings")
            .num_columns(2)
            .spacing([24.0, 10.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Max scrolls");
                ui.add(egui::DragValue::new(&mut settings.max_scrolls).range(0..=POOL_MAX));
                ui.end_row();

                ui.label("Max scroll level");
                ui.add(
                    egui::DragValue::new(&mut settings.max_scroll_level)
                        .range(0..=MAX_SCROLL_LEVEL),
                );
                ui.end_row();

                ui.label("Min special scroll level");
                ui.add(
                    egui::DragValue::new(&mut settings.min_special_scroll_level)
                        .range(0..=MAX_SCROLL_LEVEL),
                );
                ui.end_row();

                ui.label("Max items");
                ui.add(egui::DragValue::new(&mut settings.max_items).range(0..=POOL_MAX));
                ui.end_row();

                ui.label("Max item rarity");
                rarity_combo(ui, "max_item_rarity", &mut settings.max_item_rarity, ..);
                ui.end_row();

                ui.label("Max specials");
                ui.add(egui::DragValue::new(&mut settings.max_specials).range(0..=POOL_MAX));
                ui.end_row();

                // min/max special rarity can't cross each other
                ui.label("Min special rarity");
                let max = settings.max_special_rarity;
                rarity_combo(
                    ui,
                    "min_special_rarity",
                    &mut settings.min_special_rarity,
                    ..=max,
                );
                ui.end_row();

                ui.label("Max special rarity");
                let min = settings.min_special_rarity;
                rarity_combo(
                    ui,
                    "max_special_rarity",
                    &mut settings.max_special_rarity,
                    min..,
                );
                ui.end_row();

                ui.label("Stock source path");
                ui.horizontal(|ui| {
                    let edit = ui.add(
                        egui::TextEdit::singleline(&mut self.stock_source_text)
                            .desired_width(360.0),
                    );
                    let submitted = edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                    load = ui.button("Load").clicked() || submitted;
                });
                ui.end_row();
            });

        if load {
            self.reload_stock();
        }
    }

    fn stock_error_modal(&mut self, ctx: &egui::Context) {
        if !self.stock_error {
            return;
        }
        let modal = egui::Modal::new(egui::Id::unique("stock_error")).show(ctx, |ui| {
            ui.set_max_width(360.0);
            ui.heading(RichText::new("Error generating shop").color(ERR_COLOR));
            ui.add_space(6.0);
            ui.label(
                "Please ensure the shop stock pools have been loaded (click Reload stock) \
                 before generating inventory.",
            );
            ui.add_space(8.0);
            ui.vertical_centered(|ui| ui.button("Dismiss").clicked())
                .inner
        });
        if modal.inner || modal.should_close() {
            self.stock_error = false;
        }
    }
}

impl eframe::App for WerlenApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        egui::Panel::top("header").show(ui, |ui| self.header(ui));
        egui::CentralPanel::default().show(ui, |ui| self.content(ui));
        self.shop.save_settings_if_changed();

        self.stock_error_modal(&ctx);
    }
}

fn stock_table(ui: &mut egui::Ui, tab: Tab, stock: &[StockItem], level_header: &str) {
    let row_height = ui.text_style_height(&egui::TextStyle::Body) + 6.0;
    TableBuilder::new(ui)
        .id_salt(tab.title())
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::remainder().at_least(160.0))
        .column(Column::initial(160.0).at_least(80.0))
        .column(Column::initial(110.0).at_least(60.0))
        .column(Column::initial(90.0).at_least(60.0))
        .header(row_height, |mut header| {
            for title in ["Name", "Category", level_header, "Price (gp)"] {
                header.col(|ui| {
                    ui.strong(title);
                });
            }
        })
        .body(|body| {
            body.rows(row_height, stock.len(), |mut row| {
                let item = &stock[row.index()];
                row.col(|ui| {
                    ui.label(&item.name);
                });
                row.col(|ui| {
                    ui.label(&item.category);
                });
                row.col(|ui| {
                    let (text, color) = match (item.rarity, item.level) {
                        (Some(rarity), _) => (rarity.to_string(), rarity_color(rarity)),
                        (None, Some(level)) => (level.to_string(), spell_color(level)),
                        (None, None) => (String::new(), Color32::GRAY),
                    };
                    ui.label(RichText::new(text).color(color).strong());
                });
                row.col(|ui| {
                    ui.label(format!("{} gp", item.price));
                });
            });
        });
}

/// rarity dropdown restricted to the given range
fn rarity_combo(
    ui: &mut egui::Ui,
    id: &str,
    value: &mut Rarity,
    allowed: impl std::ops::RangeBounds<Rarity>,
) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(RichText::new(value.to_string()).color(rarity_color(*value)))
        .show_ui(ui, |ui| {
            for rarity in Rarity::ALL.into_iter().filter(|r| allowed.contains(r)) {
                let text = RichText::new(rarity.to_string()).color(rarity_color(rarity));
                ui.selectable_value(value, rarity, text);
            }
        });
}

/// Translate level/rarity to color
fn rarity_color(rarity: Rarity) -> Color32 {
    match rarity {
        Rarity::Common => Color32::from_rgb(150, 150, 150),
        Rarity::Uncommon => Color32::from_rgb(60, 180, 75),
        Rarity::Rare => Color32::from_rgb(40, 160, 210),
        Rarity::VeryRare => Color32::from_rgb(180, 90, 220),
        Rarity::Legendary => Color32::from_rgb(230, 160, 30),
    }
}

fn spell_color(level: usize) -> Color32 {
    match level {
        0 | 1 => rarity_color(Rarity::Common),
        2 | 3 => rarity_color(Rarity::Uncommon),
        4 | 5 => rarity_color(Rarity::Rare),
        6..=8 => rarity_color(Rarity::VeryRare),
        _ => rarity_color(Rarity::Legendary),
    }
}
