//! the egui app: header, shop tabs, and footer. The other tabs, the tables,
//! toasts, and shared styling each live in their own module

mod catalogue;
mod settings;
mod style;
mod tables;
mod toasts;

use chrono::{Local, TimeZone};
use egui::RichText;

use crate::shop::{Shop, StockItem, StockList};
use catalogue::{ItemForm, Kind};
use style::{ERR_COLOR, TAB_SIZE, banner, generate_button};
use tables::{Sort, group_repeats, sort_rows, stock_table};
use toasts::Toast;

pub use style::setup_fonts;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Tab {
    #[default]
    Scrolls,
    Items,
    Specials,
    Catalogue,
    Settings,
}

impl Tab {
    const ALL: [Tab; 5] = [
        Tab::Scrolls,
        Tab::Items,
        Tab::Specials,
        Tab::Catalogue,
        Tab::Settings,
    ];

    /// the stock list a shop tab shows; None for the tabs that manage the shop
    fn stock_list(self) -> Option<StockList> {
        match self {
            Tab::Scrolls => Some(StockList::Scrolls),
            Tab::Items => Some(StockList::Items),
            Tab::Specials => Some(StockList::Specials),
            Tab::Catalogue | Tab::Settings => None,
        }
    }

    fn title(self) -> &'static str {
        match self {
            Tab::Scrolls => "Scrolls",
            Tab::Items => "Items",
            Tab::Specials => "Specials",
            Tab::Catalogue => "Catalogue",
            Tab::Settings => "Settings",
        }
    }
}

pub struct WerlenApp {
    shop: Shop,
    tab: Tab,
    /// Show the "nothing to generate from" popup?
    stock_error: bool,

    /// Catalogue tab search text and scroll/item filter (None = show both)
    search: String,
    kind_filter: Option<Kind>,
    /// Open add/edit dialog, if any
    editing: Option<ItemForm>,
    /// Item awaiting delete confirmation, if any
    pending_delete: Option<StockItem>,

    toasts: Vec<Toast>,

    /// Column sort for each tab's table, indexed by `Tab as usize`; None keeps shop order
    sorts: [Option<Sort>; Tab::ALL.len()],
}

impl WerlenApp {
    /// opens (or creates) the database, so this touches disk
    pub fn new() -> Self {
        Self {
            shop: Shop::open(),
            tab: Tab::default(),
            stock_error: false,
            search: String::new(),
            kind_filter: None,
            editing: None,
            pending_delete: None,
            toasts: Vec::new(),
            sorts: [None; Tab::ALL.len()],
        }
    }

    fn generate(&mut self) {
        if !self.shop.generate_shop() {
            self.stock_error = true;
        }
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                banner(ui);
                ui.add_space(2.0);
                ui.weak(self.shop_summary());
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if generate_button(ui)
                    .on_hover_text("Replace the current shop with a freshly generated one")
                    .clicked()
                {
                    self.generate();
                }
            });
        });

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            for tab in Tab::ALL {
                let Some(list) = tab.stock_list() else {
                    continue;
                };
                let count = self.shop.stock[list].len();
                let label = RichText::new(format!("{} ({count})", tab.title())).size(TAB_SIZE);
                ui.selectable_value(&mut self.tab, tab, label);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // right to left, so reversed
                for tab in Tab::ALL
                    .into_iter()
                    .rev()
                    .filter(|t| t.stock_list().is_none())
                {
                    let label = RichText::new(tab.title()).size(TAB_SIZE);
                    ui.selectable_value(&mut self.tab, tab, label);
                }
            });
        });
        ui.add_space(4.0);
    }

    /// "Generated 19 Oct 2026, 19:42 · 35 entries · 14,350 gp total"
    fn shop_summary(&self) -> String {
        let stock = &self.shop.stock;
        if stock.is_empty() {
            return "No shop generated yet".to_string();
        }
        let entries = stock.iter().count();
        let value: usize = stock.iter().map(|item| item.price).sum();
        let totals = format!("{entries} entries · {} gp total", thousands(value));

        // shops saved before generation times were recorded have no date
        match self
            .shop
            .generated_at
            .and_then(|secs| Local.timestamp_opt(secs, 0).single())
        {
            Some(when) => format!("Generated {} · {totals}", when.format("%-d %b %Y, %H:%M")),
            None => totals,
        }
    }

    fn footer(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let (scrolls, items) = self.shop.pool_sizes();
            if scrolls + items == 0 {
                ui.colored_label(
                    ERR_COLOR,
                    "Database empty — import a CSV or add items on the Catalogue tab",
                );
            } else {
                ui.weak(format!(
                    "Database: {} scrolls · {} items",
                    thousands(scrolls),
                    thousands(items)
                ));
            }
            if self.shop.db_path.is_none() {
                ui.colored_label(ERR_COLOR, "· in memory, changes won't be saved");
            }
        });
    }

    fn content(&mut self, ui: &mut egui::Ui) {
        let tab = self.tab;
        let Some(list) = tab.stock_list() else {
            return match tab {
                Tab::Catalogue => self.catalogue(ui),
                _ => self.settings(ui),
            };
        };

        if self.shop.stock.is_empty() {
            let mut generate = false;
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.3);
                ui.weak(format!(
                    "Generate a shop to see its {} here.",
                    tab.title().to_lowercase()
                ));
                ui.add_space(8.0);
                generate = generate_button(ui).clicked();
            });
            if generate {
                self.generate();
            }
            return;
        }

        let mut rows = group_repeats(&self.shop.stock[list]);
        let sort = &mut self.sorts[tab as usize];
        sort_rows(&mut rows, *sort);
        stock_table(ui, tab, &rows, sort);
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
                "The shop needs at least one scroll and one item to draw from. \
                 Import a CSV or add stock on the Catalogue tab.",
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
        egui::Panel::bottom("footer").show(ui, |ui| {
            ui.add_space(2.0);
            self.footer(ui);
            ui.add_space(2.0);
        });
        egui::CentralPanel::default().show(ui, |ui| self.content(ui));
        self.shop.save_settings_if_changed();

        self.stock_error_modal(&ctx);
        self.item_form_modal(&ctx);
        self.delete_modal(&ctx);
        self.show_toasts(&ctx);
    }
}

/// 1234567 -> "1,234,567"
fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_separators() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_000), "1,000");
        assert_eq!(thousands(25_000), "25,000");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }
}
