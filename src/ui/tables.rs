//! the stock tables: repeated picks grouped into a quantity, sortable columns,
//! and level/rarity badges

use std::cmp::Ordering;

use egui::RichText;
use egui_extras::{Column, TableBuilder};
use egui_material_icons::icons::{ICON_ARROW_DROP_DOWN, ICON_ARROW_DROP_UP};

use super::Tab;
use super::style::{rarity_color, spell_color};
use super::thousands;
use crate::shop::StockItem;

/// room left at the right edge of tables so the floating scrollbar doesn't cover prices
const SCROLLBAR_GUTTER: f32 = 10.0;

/// a row of a stock table, with repeated picks merged into a quantity
pub(super) struct StockRow<'a> {
    pub(super) item: &'a StockItem,
    pub(super) qty: usize,
}

/// merge repeated picks of the same entry, keeping first-seen order
pub(super) fn group_repeats(stock: &[StockItem]) -> Vec<StockRow<'_>> {
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
pub(super) enum SortColumn {
    Qty,
    Name,
    Category,
    /// level for scrolls, rarity for items; scrolls sort before items
    Tier,
    Price,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Sort {
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
            SortColumn::Tier => a.item.tier_key().cmp(&b.item.tier_key()),
            SortColumn::Price => a.item.price.cmp(&b.item.price),
        }
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
pub(super) fn sort_rows(rows: &mut [StockRow], sort: Option<Sort>) {
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
pub(super) fn item_cells(row: &mut egui_extras::TableRow, item: &StockItem, verbose: bool) {
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

/// an item's rarity or a scroll's level, as the badge shows it
pub(super) fn tier_text(item: &StockItem, verbose: bool) -> Option<String> {
    Some(match (item.rarity, item.level) {
        (Some(rarity), _) => rarity.to_string(),
        (None, Some(0)) if verbose => "Cantrip scroll".to_string(),
        (None, Some(0)) => "Cantrip".to_string(),
        (None, Some(level)) if verbose => format!("Lvl {level} scroll"),
        (None, Some(level)) => format!("Level {level}"),
        (None, None) => return None,
    })
}

/// coloured pill showing an item's rarity or a scroll's level
fn tier_badge(ui: &mut egui::Ui, item: &StockItem, verbose: bool) {
    let Some(text) = tier_text(item, verbose) else {
        return;
    };
    let color = match (item.rarity, item.level) {
        (Some(rarity), _) => rarity_color(rarity),
        (None, level) => spell_color(level.unwrap_or_default()),
    }
    .get(ui);
    egui::Frame::new()
        .fill(color.gamma_multiply(0.18))
        .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.6)))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(6, 0))
        .show(ui, |ui| {
            ui.label(RichText::new(text).color(color).strong());
        });
}

pub(super) fn row_height(ui: &egui::Ui) -> f32 {
    ui.text_style_height(&egui::TextStyle::Body) + 8.0
}

/// a striped, resizable table; callers add their columns
pub(super) fn table<'a>(ui: &'a mut egui::Ui, id: &str) -> TableBuilder<'a> {
    TableBuilder::new(ui)
        .id_salt(id)
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
}

/// the name, category, level/rarity, and price columns that [`item_cells`] fills
pub(super) fn item_columns(table: TableBuilder<'_>) -> TableBuilder<'_> {
    table
        .column(Column::remainder().at_least(160.0))
        .column(Column::initial(160.0).at_least(80.0))
        .column(Column::initial(130.0).at_least(80.0))
        .column(Column::initial(90.0).at_least(60.0))
}

/// sortable headers for [`item_columns`]
pub(super) fn item_headers(
    header: &mut egui_extras::TableRow,
    category: &str,
    tier: &str,
    sort: &mut Option<Sort>,
) {
    sort_header(header, "Name", SortColumn::Name, sort, false);
    sort_header(header, category, SortColumn::Category, sort, false);
    sort_header(header, tier, SortColumn::Tier, sort, false);
    sort_header(header, "Price (gp)", SortColumn::Price, sort, true);
}

pub(super) fn stock_table(ui: &mut egui::Ui, tab: Tab, rows: &[StockRow], sort: &mut Option<Sort>) {
    let (category, tier) = match tab {
        Tab::Scrolls => ("School", "Level"),
        Tab::Items => ("Category", "Rarity"),
        _ => ("Category", "Level/Rarity"),
    };
    let row_height = row_height(ui);
    item_columns(table(ui, tab.title()).column(Column::auto().at_least(40.0)))
        .header(row_height, |mut header| {
            sort_header(&mut header, "Qty", SortColumn::Qty, sort, true);
            item_headers(&mut header, category, tier, sort);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shop::Rarity;

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
}
