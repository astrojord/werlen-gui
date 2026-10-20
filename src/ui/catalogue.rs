//! the Catalogue tab: searching, importing, and editing the item database

use egui::RichText;
use egui_extras::Column;
use egui_material_icons::icons::{ICON_ADD, ICON_DELETE, ICON_EDIT, ICON_SEARCH, ICON_UPLOAD_FILE};

use super::style::{ERR_COLOR, rarity_combo, with_icon};
use super::tables::{
    StockRow, item_cells, item_columns, item_headers, row_height, sort_rows, table,
};
use super::{Tab, WerlenApp};
use crate::shop::{MAX_SCROLL_LEVEL, Rarity, StockItem};

/// most expensive thing the item form will let you price something at
const PRICE_MAX: usize = 1_000_000;

/// scroll or item, for the catalogue filter and the item form
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Scroll,
    Item,
}

/// the add/edit item dialog
pub(super) struct ItemForm {
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

    pub(super) fn catalogue(&mut self, ui: &mut egui::Ui) {
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

        let row_height = row_height(ui);
        let mut edit = None;
        let mut delete = None;
        item_columns(table(ui, "catalogue"))
            .column(Column::auto()) // edit/delete buttons
            .header(row_height, |mut header| {
                item_headers(&mut header, "Category", "Level/Rarity", sort);
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

    pub(super) fn item_form_modal(&mut self, ctx: &egui::Context) {
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

    pub(super) fn delete_modal(&mut self, ctx: &egui::Context) {
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
            if let Some(id) = item.id {
                self.shop.delete_item(id);
            }
        } else if !(cancel || modal.should_close()) {
            self.pending_delete = Some(item);
        }
    }
}
