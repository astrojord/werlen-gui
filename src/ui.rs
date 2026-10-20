use std::cmp::Ordering;
use std::path::Path;
use std::time::Duration;

use chrono::{Local, TimeZone};
use egui::epaint::text::{FontInsert, FontPriority, InsertFontFamily};
use egui::{Color32, FontData, FontFamily, RichText, ThemePreference};
use egui_extras::{Column, TableBuilder};
use egui_material_icons::MaterialIcon;
use egui_material_icons::icons::{
    ICON_ADD, ICON_ARROW_DROP_DOWN, ICON_ARROW_DROP_UP, ICON_BRIGHTNESS_AUTO, ICON_CHECK_CIRCLE,
    ICON_DARK_MODE, ICON_DELETE, ICON_EDIT, ICON_ERROR, ICON_FOLDER_OPEN, ICON_LIGHT_MODE,
    ICON_SEARCH, ICON_UPLOAD_FILE, ICON_WAND_STARS,
};

use crate::shop::{MAX_SCROLL_LEVEL, Notice, POOL_MAX, Rarity, Shop, StockItem};

const ERR_COLOR: Color32 = Color32::from_rgb(230, 80, 80);
/// fill for the primary Generate button
const ACCENT: Color32 = Color32::from_rgb(255, 200, 100);

/// how long info toasts stay up, and how long of that they spend fading out
const TOAST_SECS: f64 = 4.0;
const TOAST_FADE_SECS: f64 = 0.6;

/// room left at the right edge of tables so the floating scrollbar doesn't cover prices
const SCROLLBAR_GUTTER: f32 = 10.0;

/// text sizes for W95FA, which runs small for its nominal size
const TEXT_SIZE: f32 = 12.5;
const SMALL_SIZE: f32 = 11.0;
const HEADING_SIZE: f32 = 20.0;
const SECTION_SIZE: f32 = 17.0;
const TAB_SIZE: f32 = 15.0;
const GENERATE_SIZE: f32 = 18.0;
const FONT_W95FA: &[u8] = include_bytes!("../assets/fonts/w95fa.otf");

/// most expensive thing the item form will let you price something at
const PRICE_MAX: usize = 1_000_000;

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

    /// the shop tabs show generated stock; the rest manage it
    fn is_shop(self) -> bool {
        matches!(self, Tab::Scrolls | Tab::Items | Tab::Specials)
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

/// scroll or item, for the catalogue filter and the item form
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Scroll,
    Item,
}

/// a short-lived message in the bottom-right corner; errors stay until dismissed
struct Toast {
    text: String,
    error: bool,
    /// egui time it was created, in seconds
    created: f64,
}

/// the add/edit item dialog
struct ItemForm {
    /// None when adding a new item
    id: Option<i64>,
    name: String,
    category: String,
    price: usize,
    kind: Kind,
    level: usize,
    rarity: Rarity,
    error: Option<String>,
}

impl ItemForm {
    fn new() -> Self {
        Self {
            id: None,
            name: String::new(),
            category: String::new(),
            price: 0,
            kind: Kind::Item,
            level: 1,
            rarity: Rarity::Common,
            error: None,
        }
    }

    fn edit(item: &StockItem) -> Self {
        Self {
            id: item.id,
            name: item.name.clone(),
            category: item.category.clone(),
            price: item.price,
            kind: if item.level.is_some() {
                Kind::Scroll
            } else {
                Kind::Item
            },
            level: item.level.unwrap_or(1),
            rarity: item.rarity.unwrap_or_default(),
            error: None,
        }
    }

    fn to_item(&self) -> StockItem {
        StockItem {
            id: self.id,
            name: self.name.trim().to_string(),
            category: self.category.trim().to_string(),
            price: self.price,
            rarity: (self.kind == Kind::Item).then_some(self.rarity),
            level: (self.kind == Kind::Scroll).then_some(self.level),
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

impl Default for WerlenApp {
    fn default() -> Self {
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
}

impl WerlenApp {
    fn import_csv(&mut self) {
        let picked = rfd::FileDialog::new()
            .set_title("Import stock CSV")
            .add_filter("CSV", &["csv"])
            .pick_file();
        if let Some(path) = picked {
            self.shop.import_csv(&path);
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
            for tab in Tab::ALL.into_iter().filter(|t| t.is_shop()) {
                let count = match tab {
                    Tab::Scrolls => self.shop.scroll_stock.len(),
                    Tab::Items => self.shop.item_stock.len(),
                    _ => self.shop.special_stock.len(),
                };
                let label = RichText::new(format!("{} ({count})", tab.title())).size(TAB_SIZE);
                ui.selectable_value(&mut self.tab, tab, label);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // right to left, so reversed
                for tab in Tab::ALL.into_iter().rev().filter(|t| !t.is_shop()) {
                    let label = RichText::new(tab.title()).size(TAB_SIZE);
                    ui.selectable_value(&mut self.tab, tab, label);
                }
            });
        });
        ui.add_space(4.0);
    }

    /// "Generated 19 Oct 2026, 19:42 · 35 entries · 14,350 gp total"
    fn shop_summary(&self) -> String {
        if !self.shop.has_stock() {
            return "No shop generated yet".to_string();
        }
        let stock = [
            &self.shop.scroll_stock,
            &self.shop.item_stock,
            &self.shop.special_stock,
        ];
        let entries: usize = stock.iter().map(|list| list.len()).sum();
        let value: usize = stock
            .iter()
            .flat_map(|list| list.iter())
            .map(|item| item.price)
            .sum();
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

    fn toast(&mut self, ctx: &egui::Context, text: String, error: bool) {
        let created = ctx.input(|i| i.time);
        self.toasts.push(Toast {
            text,
            error,
            created,
        });
    }

    fn show_toasts(&mut self, ctx: &egui::Context) {
        if let Some(notice) = self.shop.notice.take() {
            let (text, error) = match notice {
                Notice::Imported(summary) => (
                    format!(
                        "Imported CSV — {} added, {} updated",
                        thousands(summary.added),
                        thousands(summary.updated)
                    ),
                    false,
                ),
                Notice::Error(msg) => (msg, true),
            };
            self.toast(ctx, text, error);
        }

        let now = ctx.input(|i| i.time);
        self.toasts
            .retain(|toast| toast.error || now - toast.created < TOAST_SECS);
        if self.toasts.is_empty() {
            return;
        }

        let mut dismissed = None;
        egui::Area::new(egui::Id::unique("toasts"))
            .anchor(egui::Align2::RIGHT_BOTTOM, [-12.0, -36.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                for (i, toast) in self.toasts.iter().enumerate() {
                    ui.scope(|ui| {
                        if !toast.error {
                            let left = TOAST_SECS - (now - toast.created);
                            ui.set_opacity((left / TOAST_FADE_SECS).clamp(0.0, 1.0) as f32);
                        }
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                if toast.error {
                                    ui.colored_label(ERR_COLOR, with_icon(ICON_ERROR, &toast.text));
                                    if ui.small_button("Dismiss").clicked() {
                                        dismissed = Some(i);
                                    }
                                } else {
                                    ui.colored_label(
                                        ok_color(ui.visuals().dark_mode),
                                        with_icon(ICON_CHECK_CIRCLE, &toast.text),
                                    );
                                }
                            });
                        });
                    });
                }
            });

        if let Some(i) = dismissed {
            self.toasts.remove(i);
        }
        if self.toasts.iter().any(|toast| !toast.error) {
            // keep animating the fade
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }

    fn content(&mut self, ui: &mut egui::Ui) {
        let tab = self.tab;
        let (stock, kind) = match tab {
            Tab::Settings => return self.settings(ui),
            Tab::Catalogue => return self.catalogue(ui),
            Tab::Scrolls => (&self.shop.scroll_stock, "scroll"),
            Tab::Items => (&self.shop.item_stock, "item"),
            Tab::Specials => (&self.shop.special_stock, "special stock"),
        };

        if !self.shop.has_stock() {
            let mut generate = false;
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.3);
                ui.weak(format!("Generate a shop to see {kind} inventory here."));
                ui.add_space(8.0);
                generate = generate_button(ui).clicked();
            });
            if generate {
                self.generate();
            }
            return;
        }

        let mut rows = group_repeats(stock);
        let sort = &mut self.sorts[tab as usize];
        sort_rows(&mut rows, *sort);
        stock_table(ui, tab, &rows, sort);
    }

    fn catalogue(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button(with_icon(ICON_ADD, "Add item")).clicked() {
                self.editing = Some(ItemForm::new());
            }
            ui.separator();
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text(with_icon(ICON_SEARCH, "Search name or category"))
                    .desired_width(240.0),
            );
            ui.selectable_value(&mut self.kind_filter, None, "All");
            ui.selectable_value(&mut self.kind_filter, Some(Kind::Scroll), "Scrolls");
            ui.selectable_value(&mut self.kind_filter, Some(Kind::Item), "Items");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(with_icon(ICON_UPLOAD_FILE, "Import CSV…"))
                    .on_hover_text("Add or update catalogue entries from a stock CSV")
                    .clicked()
                {
                    self.import_csv();
                }
            });
        });
        ui.add_space(4.0);

        let search = self.search.trim().to_lowercase();
        let mut shown: Vec<StockRow> = self
            .shop
            .catalogue
            .iter()
            .filter(|item| match self.kind_filter {
                Some(Kind::Scroll) => item.level.is_some(),
                Some(Kind::Item) => item.level.is_none(),
                None => true,
            })
            .filter(|item| {
                search.is_empty()
                    || item.name.to_lowercase().contains(&search)
                    || item.category.to_lowercase().contains(&search)
            })
            .map(|item| StockRow { item, qty: 1 })
            .collect();
        let sort = &mut self.sorts[Tab::Catalogue as usize];
        sort_rows(&mut shown, *sort);

        ui.weak(format!(
            "Showing {} of {} entries",
            shown.len(),
            self.shop.catalogue.len()
        ));

        let row_height = ui.text_style_height(&egui::TextStyle::Body) + 8.0;
        let mut edit = None;
        let mut delete = None;
        TableBuilder::new(ui)
            .id_salt("catalogue")
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::remainder().at_least(160.0))
            .column(Column::initial(160.0).at_least(80.0))
            .column(Column::initial(130.0).at_least(80.0))
            .column(Column::initial(90.0).at_least(60.0))
            .column(Column::auto())
            .header(row_height, |mut header| {
                sort_header(&mut header, "Name", SortColumn::Name, sort, false);
                sort_header(&mut header, "Category", SortColumn::Category, sort, false);
                sort_header(&mut header, "Level/Rarity", SortColumn::Tier, sort, false);
                sort_header(&mut header, "Price (gp)", SortColumn::Price, sort, true);
                header.col(|_| {});
            })
            .body(|body| {
                body.rows(row_height, shown.len(), |mut row| {
                    let item = shown[row.index()].item;
                    item_cells(&mut row, item, true);
                    row.col(|ui| {
                        if ui.small_button(ICON_EDIT).on_hover_text("Edit").clicked() {
                            edit = Some(item);
                        }
                        if ui
                            .small_button(ICON_DELETE)
                            .on_hover_text("Delete")
                            .clicked()
                        {
                            delete = Some(item);
                        }
                    });
                });
            });

        if let Some(item) = edit {
            self.editing = Some(ItemForm::edit(item));
        }
        if let Some(item) = delete {
            self.pending_delete = Some(item.clone());
        }
    }

    fn settings(&mut self, ui: &mut egui::Ui) {
        let mut open_folder = false;
        egui::ScrollArea::vertical().show(ui, |ui| {
            let settings = &mut self.shop.settings;

            settings_section(ui, "Scrolls", |ui| {
                setting_label(ui, "Number of scrolls", "How many scrolls the Scrolls tab stocks");
                ui.add(egui::Slider::new(&mut settings.max_scrolls, 0..=POOL_MAX));
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
                ui.add(egui::Slider::new(&mut settings.max_items, 0..=POOL_MAX));
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
                ui.add(egui::Slider::new(&mut settings.max_specials, 0..=POOL_MAX));
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

    fn item_form_modal(&mut self, ctx: &egui::Context) {
        let Some(mut form) = self.editing.take() else {
            return;
        };
        let title = if form.id.is_some() {
            "Edit item"
        } else {
            "Add item"
        };

        let modal = egui::Modal::new(egui::Id::unique("item_form")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.heading(title);
            ui.add_space(6.0);
            egui::Grid::new("item_form_grid")
                .num_columns(2)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    ui.label("Name");
                    ui.text_edit_singleline(&mut form.name);
                    ui.end_row();

                    ui.label("Category");
                    ui.text_edit_singleline(&mut form.category);
                    ui.end_row();

                    ui.label("Price (gp)");
                    ui.add(egui::DragValue::new(&mut form.price).range(0..=PRICE_MAX));
                    ui.end_row();

                    ui.label("Type");
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut form.kind, Kind::Item, "Item");
                        ui.radio_value(&mut form.kind, Kind::Scroll, "Scroll");
                    });
                    ui.end_row();

                    match form.kind {
                        Kind::Scroll => {
                            ui.label("Level");
                            ui.add(
                                egui::DragValue::new(&mut form.level).range(0..=MAX_SCROLL_LEVEL),
                            );
                        }
                        Kind::Item => {
                            ui.label("Rarity");
                            rarity_combo(ui, "item_form_rarity", &mut form.rarity, ..);
                        }
                    }
                    ui.end_row();
                });

            if let Some(error) = &form.error {
                ui.add_space(4.0);
                ui.colored_label(ERR_COLOR, error);
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let save = ui.button("Save").clicked();
                let cancel = ui.button("Cancel").clicked();
                (save, cancel)
            })
            .inner
        });

        let (save, cancel) = modal.inner;
        if cancel || modal.should_close() {
            return; // drop the form
        }
        if save {
            let item = form.to_item();
            let result = if item.name.is_empty() {
                Err("name can't be empty".to_string())
            } else {
                match form.id {
                    Some(id) => self.shop.update_item(id, &item),
                    None => self.shop.add_item(&item),
                }
            };
            match result {
                Ok(()) => return,
                Err(err) => form.error = Some(err),
            }
        }
        self.editing = Some(form);
    }

    fn delete_modal(&mut self, ctx: &egui::Context) {
        let Some(item) = self.pending_delete.take() else {
            return;
        };
        let modal = egui::Modal::new(egui::Id::unique("delete_item")).show(ctx, |ui| {
            ui.set_max_width(360.0);
            ui.heading("Delete item");
            ui.add_space(6.0);
            ui.label(format!(
                "Delete \"{}\" from the catalogue? Shops already generated keep their copy.",
                item.name
            ));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let delete = ui
                    .button(RichText::new("Delete").color(ERR_COLOR))
                    .clicked();
                let cancel = ui.button("Cancel").clicked();
                (delete, cancel)
            })
            .inner
        });

        let (delete, cancel) = modal.inner;
        if delete {
            if let Some(id) = item.id
                && let Err(err) = self.shop.delete_item(id)
            {
                self.shop.notice = Some(Notice::Error(format!("couldn't delete item: {err}")));
            }
        } else if !(cancel || modal.should_close()) {
            self.pending_delete = Some(item);
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
fn with_icon(icon: MaterialIcon, text: &str) -> String {
    format!("{} {text}", icon.codepoint)
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

/// the block-letter WERLEN'S banner from the TUI
const BANNER: &str = include_str!("../assets/werlen_ascii.txt");
const BANNER_SIZE: f32 = 12.0;

/// draw [`BANNER`] in purple, with lines packed tight so the block characters join up
fn banner(ui: &mut egui::Ui) {
    let color = if ui.visuals().dark_mode {
        Color32::from_rgb(185, 140, 255)
    } else {
        Color32::from_rgb(105, 55, 180)
    };
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
fn generate_button(ui: &mut egui::Ui) -> egui::Response {
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

/// a row of a stock table, with repeated picks merged into a quantity
struct StockRow<'a> {
    item: &'a StockItem,
    qty: usize,
}

/// merge repeated picks of the same entry, keeping first-seen order
fn group_repeats(stock: &[StockItem]) -> Vec<StockRow<'_>> {
    let mut rows: Vec<StockRow> = Vec::new();
    for item in stock {
        let existing = rows.iter_mut().find(|row| {
            row.item.name.eq_ignore_ascii_case(&item.name)
                && row.item.rarity == item.rarity
                && row.item.level == item.level
        });
        match existing {
            Some(row) => row.qty += 1,
            None => rows.push(StockRow { item, qty: 1 }),
        }
    }
    rows
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortColumn {
    Qty,
    Name,
    Category,
    /// level for scrolls, rarity for items; scrolls sort before items
    Tier,
    Price,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Sort {
    column: SortColumn,
    descending: bool,
}

impl SortColumn {
    fn compare(self, a: &StockRow, b: &StockRow) -> Ordering {
        match self {
            SortColumn::Qty => a.qty.cmp(&b.qty),
            SortColumn::Name => a.item.name.to_lowercase().cmp(&b.item.name.to_lowercase()),
            SortColumn::Category => a
                .item
                .category
                .to_lowercase()
                .cmp(&b.item.category.to_lowercase()),
            SortColumn::Tier => tier_key(a.item).cmp(&tier_key(b.item)),
            SortColumn::Price => a.item.price.cmp(&b.item.price),
        }
    }
}

fn tier_key(item: &StockItem) -> (u8, usize) {
    match (item.level, item.rarity) {
        (Some(level), _) => (0, level),
        (None, Some(rarity)) => (1, rarity as usize),
        (None, None) => (2, 0),
    }
}

/// clicking a header cycles ascending -> descending -> back to shop order
fn next_sort(current: Option<Sort>, column: SortColumn) -> Option<Sort> {
    match current {
        Some(sort) if sort.column == column && !sort.descending => Some(Sort {
            column,
            descending: true,
        }),
        Some(sort) if sort.column == column => None,
        _ => Some(Sort {
            column,
            descending: false,
        }),
    }
}

/// stable sort, so ties keep shop order
fn sort_rows(rows: &mut [StockRow], sort: Option<Sort>) {
    let Some(sort) = sort else {
        return;
    };
    rows.sort_by(|a, b| {
        let ordering = sort.column.compare(a, b);
        if sort.descending {
            ordering.reverse()
        } else {
            ordering
        }
    });
}

/// a clickable header cell that sorts by `column`
fn sort_header(
    header: &mut egui_extras::TableRow,
    title: &str,
    column: SortColumn,
    sort: &mut Option<Sort>,
    align_right: bool,
) {
    header.col(|ui| {
        let arrow = match sort {
            Some(s) if s.column == column && s.descending => ICON_ARROW_DROP_DOWN.codepoint,
            Some(s) if s.column == column => ICON_ARROW_DROP_UP.codepoint,
            _ => "",
        };
        let layout = if align_right {
            egui::Layout::right_to_left(egui::Align::Center)
        } else {
            egui::Layout::left_to_right(egui::Align::Center)
        };
        ui.with_layout(layout, |ui| {
            if align_right && column == SortColumn::Price {
                ui.add_space(SCROLLBAR_GUTTER);
            }
            let button =
                egui::Button::new(RichText::new(format!("{title} {arrow}")).strong()).frame(false);
            if ui
                .add(button)
                .on_hover_text("Sort by this column")
                .clicked()
            {
                *sort = next_sort(*sort, column);
            }
        });
    });
}

/// name, category, level/rarity badge, and price cells for one stock row.
/// `verbose` badges say "Lvl 3 scroll" rather than "Level 3", for mixed lists
fn item_cells(row: &mut egui_extras::TableRow, item: &StockItem, verbose: bool) {
    row.col(|ui| {
        ui.label(&item.name);
    });
    row.col(|ui| {
        ui.label(&item.category);
    });
    row.col(|ui| {
        tier_badge(ui, item, verbose);
    });
    row.col(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(SCROLLBAR_GUTTER);
            // a 0 price in the stock data means nobody set one, not that it's free
            if item.price == 0 {
                ui.weak("—").on_hover_text("No price set");
            } else {
                ui.label(thousands(item.price));
            }
        });
    });
}

/// coloured pill showing an item's rarity or a scroll's level
fn tier_badge(ui: &mut egui::Ui, item: &StockItem, verbose: bool) {
    let dark = ui.visuals().dark_mode;
    let (text, color) = match (item.rarity, item.level) {
        (Some(rarity), _) => (rarity.to_string(), rarity_color(rarity, dark)),
        (None, Some(0)) if verbose => ("Cantrip scroll".to_string(), spell_color(0, dark)),
        (None, Some(0)) => ("Cantrip".to_string(), spell_color(0, dark)),
        (None, Some(level)) if verbose => (format!("Lvl {level} scroll"), spell_color(level, dark)),
        (None, Some(level)) => (format!("Level {level}"), spell_color(level, dark)),
        (None, None) => return,
    };
    egui::Frame::new()
        .fill(color.gamma_multiply(0.18))
        .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.6)))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(6, 0))
        .show(ui, |ui| {
            ui.label(RichText::new(text).color(color).strong());
        });
}

fn stock_table(ui: &mut egui::Ui, tab: Tab, rows: &[StockRow], sort: &mut Option<Sort>) {
    let (category, tier) = match tab {
        Tab::Scrolls => ("School", "Level"),
        Tab::Items => ("Category", "Rarity"),
        _ => ("Category", "Level/Rarity"),
    };
    let row_height = ui.text_style_height(&egui::TextStyle::Body) + 8.0;
    TableBuilder::new(ui)
        .id_salt(tab.title())
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::auto().at_least(40.0))
        .column(Column::remainder().at_least(160.0))
        .column(Column::initial(160.0).at_least(80.0))
        .column(Column::initial(130.0).at_least(80.0))
        .column(Column::initial(90.0).at_least(60.0))
        .header(row_height, |mut header| {
            sort_header(&mut header, "Qty", SortColumn::Qty, sort, true);
            sort_header(&mut header, "Name", SortColumn::Name, sort, false);
            sort_header(&mut header, category, SortColumn::Category, sort, false);
            sort_header(&mut header, tier, SortColumn::Tier, sort, false);
            sort_header(&mut header, "Price (gp)", SortColumn::Price, sort, true);
        })
        .body(|body| {
            body.rows(row_height, rows.len(), |mut row| {
                let StockRow { item, qty } = rows[row.index()];
                row.col(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if qty > 1 {
                            ui.strong(format!("×{qty}"));
                        } else {
                            ui.weak("1");
                        }
                    });
                });
                item_cells(&mut row, item, tab == Tab::Specials);
            });
        });
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

/// rarity dropdown restricted to the given range
fn rarity_combo(
    ui: &mut egui::Ui,
    id: &str,
    value: &mut Rarity,
    allowed: impl std::ops::RangeBounds<Rarity>,
) {
    let dark = ui.visuals().dark_mode;
    egui::ComboBox::from_id_salt(id)
        .selected_text(RichText::new(value.to_string()).color(rarity_color(*value, dark)))
        .show_ui(ui, |ui| {
            for rarity in Rarity::ALL.into_iter().filter(|r| allowed.contains(r)) {
                let text = RichText::new(rarity.to_string()).color(rarity_color(rarity, dark));
                ui.selectable_value(value, rarity, text);
            }
        });
}

/// Translate rarity to color, picked to stay legible on the current theme
fn rarity_color(rarity: Rarity, dark_mode: bool) -> Color32 {
    let (dark, light) = match rarity {
        Rarity::Common => ((185, 190, 200), (95, 100, 110)),
        Rarity::Uncommon => ((80, 200, 100), (25, 125, 45)),
        Rarity::Rare => ((70, 170, 240), (15, 100, 175)),
        Rarity::VeryRare => ((195, 120, 240), (125, 45, 175)),
        Rarity::Legendary => ((240, 175, 50), (170, 100, 0)),
    };
    let (r, g, b) = if dark_mode { dark } else { light };
    Color32::from_rgb(r, g, b)
}

/// scroll levels share the rarity palette, in bands
fn spell_color(level: usize, dark_mode: bool) -> Color32 {
    let rarity = match level {
        0 | 1 => Rarity::Common,
        2 | 3 => Rarity::Uncommon,
        4 | 5 => Rarity::Rare,
        6..=8 => Rarity::VeryRare,
        _ => Rarity::Legendary,
    };
    rarity_color(rarity, dark_mode)
}

/// success green, darker on light backgrounds
fn ok_color(dark_mode: bool) -> Color32 {
    if dark_mode {
        Color32::from_rgb(90, 190, 110)
    } else {
        Color32::from_rgb(30, 130, 55)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, level: Option<usize>, rarity: Option<Rarity>, price: usize) -> StockItem {
        StockItem {
            id: None,
            name: name.to_string(),
            category: String::new(),
            price,
            rarity,
            level,
        }
    }

    #[test]
    fn repeats_are_grouped_in_first_seen_order() {
        let stock = [
            item("Shield", Some(1), None, 60),
            item("Fireball", Some(3), None, 300),
            item("shield", Some(1), None, 60),
            item("Shield", Some(2), None, 60), // different level, different entry
        ];
        let rows = group_repeats(&stock);
        let summary: Vec<_> = rows
            .iter()
            .map(|r| (r.item.name.as_str(), r.item.level, r.qty))
            .collect();
        assert_eq!(
            summary,
            [
                ("Shield", Some(1), 2),
                ("Fireball", Some(3), 1),
                ("Shield", Some(2), 1)
            ]
        );
    }

    #[test]
    fn sort_cycles_and_orders() {
        let name = Some(Sort {
            column: SortColumn::Name,
            descending: false,
        });
        assert_eq!(next_sort(None, SortColumn::Name), name);
        let desc = next_sort(name, SortColumn::Name);
        assert_eq!(desc.map(|s| s.descending), Some(true));
        assert_eq!(next_sort(desc, SortColumn::Name), None);
        assert_eq!(
            next_sort(desc, SortColumn::Price).map(|s| s.column),
            Some(SortColumn::Price)
        );

        let stock = [
            item("Bag of Holding", None, Some(Rarity::Uncommon), 1000),
            item("Fireball", Some(3), None, 300),
            item("Potion of Healing", None, Some(Rarity::Common), 25),
            item("Shield", Some(1), None, 60),
        ];
        let names = |sort| {
            let mut rows = group_repeats(&stock);
            sort_rows(&mut rows, sort);
            rows.iter().map(|r| r.item.name.clone()).collect::<Vec<_>>()
        };
        // scrolls by level, then items by rarity
        assert_eq!(
            names(Some(Sort {
                column: SortColumn::Tier,
                descending: false
            })),
            ["Shield", "Fireball", "Potion of Healing", "Bag of Holding"]
        );
        assert_eq!(
            names(Some(Sort {
                column: SortColumn::Price,
                descending: true
            })),
            ["Bag of Holding", "Fireball", "Shield", "Potion of Healing"]
        );
        assert_eq!(
            names(None),
            ["Bag of Holding", "Fireball", "Potion of Healing", "Shield"]
        );
    }

    #[test]
    fn thousands_separators() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_000), "1,000");
        assert_eq!(thousands(25_000), "25,000");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }
}
