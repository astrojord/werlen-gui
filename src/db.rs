use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::shop::{Rarity, Settings, Stock, StockItem, StockList};

/// schema migrations, applied in order. A database's `user_version` is the
/// number of these it has already had applied; add a new file to change the schema
const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/001_initial.sql"),
    include_str!("../migrations/002_shop_meta.sql"),
];

/// the last generated shop as saved in the database
#[derive(Debug, Default)]
pub struct SavedShop {
    pub stock: Stock,
    /// unix seconds; None if no shop has been generated yet
    pub generated_at: Option<i64>,
}

/// result of a CSV import: how many rows were new vs matched an existing item
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ImportSummary {
    pub added: usize,
    pub updated: usize,
}

pub struct Db {
    conn: Connection,
}

impl Db {
    /// where the database lives by default, e.g. ~/.local/share/werlen-gui/werlen.db
    pub fn default_path() -> Option<PathBuf> {
        ProjectDirs::from("", "", "werlen-gui").map(|dirs| dirs.data_dir().join("werlen.db"))
    }

    /// open (creating if needed) the database at `path`
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(parent) = path.parent() {
            // let Connection::open report the failure if this didn't work
            let _ = std::fs::create_dir_all(parent);
        }
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> rusqlite::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> rusqlite::Result<Self> {
        let mut db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&mut self) -> rusqlite::Result<()> {
        let applied: usize = self
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))?;
        if applied >= MIGRATIONS.len() {
            return Ok(());
        }

        let tx = self.conn.transaction()?;
        for migration in &MIGRATIONS[applied..] {
            tx.execute_batch(migration)?;
        }
        tx.pragma_update(None, "user_version", MIGRATIONS.len())?;
        tx.commit()
    }

    /// every item in the catalogue, scrolls first by level, then items by rarity
    pub fn items(&self) -> rusqlite::Result<Vec<StockItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, category, price, rarity, level, id FROM items
             ORDER BY level IS NULL, level, rarity, name COLLATE NOCASE",
        )?;
        stmt.query_map([], |row| {
            Ok(StockItem {
                id: Some(row.get(5)?),
                ..item_from_row(row)?
            })
        })?
        .collect()
    }

    /// add a new item, returning its id
    pub fn insert_item(&self, item: &StockItem) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO items (name, category, price, rarity, level) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                item.name,
                item.category,
                item.price,
                item.rarity,
                item.level
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// overwrite the item with `id`
    pub fn update_item(&self, id: i64, item: &StockItem) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE items SET name = ?2, category = ?3, price = ?4, rarity = ?5, level = ?6
             WHERE id = ?1",
            params![
                id,
                item.name,
                item.category,
                item.price,
                item.rarity,
                item.level
            ],
        )?;
        Ok(())
    }

    pub fn delete_item(&self, id: i64) -> rusqlite::Result<()> {
        self.conn
            .execute("DELETE FROM items WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// add or update items, matching on (name ignoring case, rarity, level).
    /// matched items get their name/category/price overwritten; nothing is deleted
    pub fn upsert_items(&mut self, items: &[StockItem]) -> rusqlite::Result<ImportSummary> {
        let mut summary = ImportSummary::default();
        let tx = self.conn.transaction()?;
        {
            let mut find = tx.prepare(
                "SELECT id FROM items
                 WHERE lower(name) = lower(?1) AND rarity IS ?2 AND level IS ?3",
            )?;
            let mut update =
                tx.prepare("UPDATE items SET name = ?2, category = ?3, price = ?4 WHERE id = ?1")?;
            let mut insert = tx.prepare(
                "INSERT INTO items (name, category, price, rarity, level)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;

            for item in items {
                let existing: Option<i64> = find
                    .query_row(params![item.name, item.rarity, item.level], |row| {
                        row.get(0)
                    })
                    .optional()?;
                match existing {
                    Some(id) => {
                        update.execute(params![id, item.name, item.category, item.price])?;
                        summary.updated += 1;
                    }
                    None => {
                        insert.execute(params![
                            item.name,
                            item.category,
                            item.price,
                            item.rarity,
                            item.level
                        ])?;
                        summary.added += 1;
                    }
                }
            }
        }
        tx.commit()?;
        Ok(summary)
    }

    /// saved generation settings, or None if they've never been saved
    pub fn load_settings(&self) -> rusqlite::Result<Option<Settings>> {
        self.conn
            .query_row(
                "SELECT max_scrolls, max_scroll_level, min_special_scroll_level, max_items,
                        max_item_rarity, max_specials, min_special_rarity, max_special_rarity
                 FROM settings WHERE id = 1",
                [],
                |row| {
                    Ok(Settings {
                        max_scrolls: row.get(0)?,
                        max_scroll_level: row.get(1)?,
                        min_special_scroll_level: row.get(2)?,
                        max_items: row.get(3)?,
                        max_item_rarity: row.get(4)?,
                        max_specials: row.get(5)?,
                        min_special_rarity: row.get(6)?,
                        max_special_rarity: row.get(7)?,
                    })
                },
            )
            .optional()
    }

    pub fn save_settings(&self, settings: &Settings) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO settings
                 (id, max_scrolls, max_scroll_level, min_special_scroll_level, max_items,
                  max_item_rarity, max_specials, min_special_rarity, max_special_rarity)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                settings.max_scrolls,
                settings.max_scroll_level,
                settings.min_special_scroll_level,
                settings.max_items,
                settings.max_item_rarity,
                settings.max_specials,
                settings.min_special_rarity,
                settings.max_special_rarity,
            ],
        )?;
        Ok(())
    }

    /// the last generated shop
    pub fn load_shop(&self) -> rusqlite::Result<SavedShop> {
        let mut stmt = self.conn.prepare(
            "SELECT name, category, price, rarity, level FROM shop_stock
             WHERE list = ?1 ORDER BY position",
        )?;
        let mut stock = Stock::default();
        for list in StockList::ALL {
            stock[list] = stmt
                .query_map(params![list as u8], item_from_row)?
                .collect::<rusqlite::Result<_>>()?;
        }
        let generated_at = self
            .conn
            .query_row(
                "SELECT generated_at FROM shop_meta WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(SavedShop {
            stock,
            generated_at,
        })
    }

    /// replace the saved shop with `stock`, generated at `generated_at` (unix seconds)
    pub fn save_shop(&mut self, stock: &Stock, generated_at: i64) -> rusqlite::Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM shop_stock", [])?;
        tx.execute(
            "INSERT OR REPLACE INTO shop_meta (id, generated_at) VALUES (1, ?1)",
            params![generated_at],
        )?;
        {
            let mut insert = tx.prepare(
                "INSERT INTO shop_stock (list, position, name, category, price, rarity, level)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for list in StockList::ALL {
                for (position, item) in stock[list].iter().enumerate() {
                    insert.execute(params![
                        list as u8,
                        position,
                        item.name,
                        item.category,
                        item.price,
                        item.rarity,
                        item.level
                    ])?;
                }
            }
        }
        tx.commit()
    }
}

/// read name, category, price, rarity, level from the first five columns
fn item_from_row(row: &Row) -> rusqlite::Result<StockItem> {
    Ok(StockItem {
        id: None,
        name: row.get(0)?,
        category: row.get(1)?,
        price: row.get(2)?,
        rarity: row.get(3)?,
        level: row.get(4)?,
    })
}

// rarities are stored as their index (Common = 0 .. Legendary = 4)
impl ToSql for Rarity {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(*self as u8))
    }
}

impl FromSql for Rarity {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let index = value.as_i64()?;
        usize::try_from(index)
            .ok()
            .and_then(|i| Rarity::ALL.get(i).copied())
            .ok_or(FromSqlError::OutOfRange(index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, rarity: Option<Rarity>, level: Option<usize>, price: usize) -> StockItem {
        StockItem {
            id: None,
            name: name.to_string(),
            category: "Test".to_string(),
            price,
            rarity,
            level,
        }
    }

    #[test]
    fn new_db_is_empty() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.items().unwrap().is_empty());
        assert_eq!(db.load_settings().unwrap(), None);
        let saved = db.load_shop().unwrap();
        assert!(saved.stock.is_empty());
        assert_eq!(saved.generated_at, None);
    }

    #[test]
    fn migrations_set_user_version() {
        let db = Db::open_in_memory().unwrap();
        let version: usize = db
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len());
    }

    #[test]
    fn reopening_keeps_data() {
        let path = std::env::temp_dir().join(format!("werlen-test-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let db = Db::open(&path).unwrap();
            db.insert_item(&item("Bag of Holding", Some(Rarity::Uncommon), None, 1000))
                .unwrap();
        }
        let db = Db::open(&path).unwrap();
        assert_eq!(db.items().unwrap().len(), 1);
        drop(db);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn upsert_adds_then_updates() {
        let mut db = Db::open_in_memory().unwrap();
        let first = [
            item("Potion of Healing", Some(Rarity::Common), None, 25),
            item("Fireball", None, Some(3), 300),
            // same name, different rarity: a separate item
            item("Djinn and Tonic", Some(Rarity::Common), None, 0),
            item("Djinn and Tonic", Some(Rarity::Rare), None, 0),
        ];
        assert_eq!(
            db.upsert_items(&first).unwrap(),
            ImportSummary {
                added: 4,
                updated: 0
            }
        );

        let second = [
            // case-insensitive match updates price
            item("potion of healing", Some(Rarity::Common), None, 50),
            item("Fireball", None, Some(3), 300),
            item("Shield", None, Some(1), 60),
        ];
        assert_eq!(
            db.upsert_items(&second).unwrap(),
            ImportSummary {
                added: 1,
                updated: 2
            }
        );

        let items = db.items().unwrap();
        assert_eq!(items.len(), 5);
        let potion = items
            .iter()
            .find(|i| i.name.eq_ignore_ascii_case("potion of healing"))
            .unwrap();
        assert_eq!(potion.price, 50);
    }

    #[test]
    fn insert_update_delete() {
        let db = Db::open_in_memory().unwrap();
        let id = db.insert_item(&item("Shield", None, Some(1), 60)).unwrap();
        db.update_item(id, &item("Shield", None, Some(1), 75))
            .unwrap();
        assert_eq!(db.items().unwrap()[0].price, 75);

        // the unique key rejects an exact duplicate
        assert!(db.insert_item(&item("SHIELD", None, Some(1), 1)).is_err());

        db.delete_item(id).unwrap();
        assert!(db.items().unwrap().is_empty());
    }

    #[test]
    fn settings_round_trip() {
        let db = Db::open_in_memory().unwrap();
        let mut settings = Settings::default();
        db.save_settings(&settings).unwrap();
        assert_eq!(db.load_settings().unwrap(), Some(settings));

        settings.max_items = 42;
        settings.max_special_rarity = Rarity::Legendary;
        db.save_settings(&settings).unwrap();
        assert_eq!(db.load_settings().unwrap(), Some(settings));
    }

    #[test]
    fn shop_round_trip() {
        let mut db = Db::open_in_memory().unwrap();
        let mut stock = Stock::default();
        stock[StockList::Scrolls] = vec![
            item("Shield", None, Some(1), 60),
            item("Fireball", None, Some(3), 300),
        ];
        stock[StockList::Items] = vec![item("Bag of Holding", Some(Rarity::Uncommon), None, 1000)];
        db.save_shop(&stock, 1_000).unwrap();

        let saved = db.load_shop().unwrap();
        assert_eq!(saved.generated_at, Some(1_000));
        assert_eq!(
            saved.stock[StockList::Scrolls]
                .iter()
                .map(|x| x.name.as_str())
                .collect::<Vec<_>>(),
            ["Shield", "Fireball"]
        );
        assert_eq!(saved.stock[StockList::Items].len(), 1);
        assert!(saved.stock[StockList::Specials].is_empty());

        // saving again replaces rather than appends
        stock[StockList::Scrolls].clear();
        db.save_shop(&stock, 2_000).unwrap();
        let saved = db.load_shop().unwrap();
        assert_eq!(saved.generated_at, Some(2_000));
        assert!(saved.stock[StockList::Scrolls].is_empty());
        assert_eq!(saved.stock[StockList::Items].len(), 1);
    }
}
