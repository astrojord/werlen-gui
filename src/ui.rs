use std::path::Path;

use egui::{Color32, RichText};
use egui_extras::{Column, TableBuilder};

use crate::shop::{MAX_SCROLL_LEVEL, POOL_MAX, Rarity, Shop, StockItem, StockStatus};

const OK_COLOR: Color32 = Color32::from_rgb(90, 190, 110);
const ERR_COLOR: Color32 = Color32::from_rgb(230, 80, 80);

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
                    .button("📥 Import CSV…")
                    .on_hover_text("Add or update catalogue items from a stock CSV")
                    .clicked()
                {
                    self.import_csv();
                }
            });
        });

        let (scrolls, items) = self.shop.pool_sizes();
        let (status_text, status_color) = match &self.shop.status {
            StockStatus::Ready if scrolls + items == 0 => (
                "● Database empty — import a CSV or add items on the Catalogue tab".to_string(),
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
            StockStatus::Error(msg) => (format!("✕ {msg}"), ERR_COLOR),
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
            Tab::Catalogue => return self.catalogue(ui),
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

    fn catalogue(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("➕ Add item").clicked() {
                self.editing = Some(ItemForm::new());
            }
            ui.separator();
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Search name or category")
                    .desired_width(240.0),
            );
            ui.selectable_value(&mut self.kind_filter, None, "All");
            ui.selectable_value(&mut self.kind_filter, Some(Kind::Scroll), "Scrolls");
            ui.selectable_value(&mut self.kind_filter, Some(Kind::Item), "Items");
        });
        ui.add_space(4.0);

        let search = self.search.trim().to_lowercase();
        let shown: Vec<&StockItem> = self
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
            .collect();

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
            .column(Column::initial(110.0).at_least(60.0))
            .column(Column::initial(90.0).at_least(60.0))
            .column(Column::auto())
            .header(row_height, |mut header| {
                for title in ["Name", "Category", "Level/Rarity", "Price (gp)", ""] {
                    header.col(|ui| {
                        ui.strong(title);
                    });
                }
            })
            .body(|body| {
                body.rows(row_height, shown.len(), |mut row| {
                    let item = shown[row.index()];
                    item_cells(&mut row, item);
                    row.col(|ui| {
                        if ui.small_button("✏").on_hover_text("Edit").clicked() {
                            edit = Some(item);
                        }
                        if ui.small_button("🗑").on_hover_text("Delete").clicked() {
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
        let settings = &mut self.shop.settings;
        let mut open_folder = false;
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

                ui.label("Database");
                ui.horizontal(|ui| match &self.shop.db_path {
                    Some(path) => {
                        ui.monospace(path.display().to_string());
                        open_folder = ui.button("Open folder").clicked();
                    }
                    None => {
                        ui.colored_label(ERR_COLOR, "in memory — changes won't be saved");
                    }
                });
                ui.end_row();
            });

        if open_folder
            && let Some(dir) = self.shop.db_path.as_deref().and_then(Path::parent)
            && let Err(err) = open_in_file_manager(dir)
        {
            self.shop.status = StockStatus::Error(format!("couldn't open folder: {err}"));
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
                self.shop.status = StockStatus::Error(format!("couldn't delete item: {err}"));
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
        egui::CentralPanel::default().show(ui, |ui| self.content(ui));
        self.shop.save_settings_if_changed();

        self.stock_error_modal(&ctx);
        self.item_form_modal(&ctx);
        self.delete_modal(&ctx);
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

/// name, category, level/rarity, and price cells for one stock row
fn item_cells(row: &mut egui_extras::TableRow, item: &StockItem) {
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
                item_cells(&mut row, item);
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
