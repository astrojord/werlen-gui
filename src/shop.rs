use rand::Rng;
use rand::distr::Distribution;
use rand::distr::weighted::WeightedIndex;
use rand::seq::IndexedRandom;
use rusqlite::ErrorCode;
use std::path::{Path, PathBuf};
use std::{error::Error, fmt};

use crate::db::{Db, ImportSummary};

/// max spell level within 5e
pub const MAX_SCROLL_LEVEL: usize = 9;

/// upper bound on the size of any one stock list
pub const POOL_MAX: usize = 128;

pub struct Shop {
    db: Db,
    /// Where the database file lives; None if we fell back to an in-memory database
    pub db_path: Option<PathBuf>,

    /// Outcome of the last load/import, shown in the header
    pub status: StockStatus,

    pub settings: Settings,
    /// what's in the database, so we only write settings when they change
    saved_settings: Settings,

    /// Every item and scroll in the database; the pool shops are drawn from
    pub catalogue: Vec<StockItem>,

    /// Current shop stock, taken from the catalogue
    pub scroll_stock: Vec<StockItem>,
    pub item_stock: Vec<StockItem>,
    pub special_stock: Vec<StockItem>,
}

impl Shop {
    /// Open the database in the platform data dir, falling back to an
    /// in-memory database (with an error status) if that fails
    pub fn open() -> Self {
        let path = Db::default_path();
        let opened = match &path {
            Some(path) => Db::open(path).map_err(|err| err.to_string()),
            None => Err("couldn't find a data directory".to_string()),
        };
        match opened {
            Ok(db) => Self::with_db(db, path),
            Err(err) => {
                let db = Db::open_in_memory().expect("in-memory sqlite should always open");
                let mut shop = Self::with_db(db, None);
                shop.status = StockStatus::Error(format!(
                    "couldn't open the database, nothing will be saved: {err}"
                ));
                shop
            }
        }
    }

    pub fn with_db(db: Db, db_path: Option<PathBuf>) -> Self {
        let mut shop = Self {
            db,
            db_path,
            status: StockStatus::Ready,
            settings: Settings::default(),
            saved_settings: Settings::default(),
            catalogue: Vec::new(),
            scroll_stock: Vec::new(),
            item_stock: Vec::new(),
            special_stock: Vec::new(),
        };
        if let Err(err) = shop.load() {
            shop.status = StockStatus::Error(format!("couldn't load the database: {err}"));
        }
        shop
    }

    fn load(&mut self) -> rusqlite::Result<()> {
        if let Some(settings) = self.db.load_settings()? {
            self.settings = settings;
            self.saved_settings = settings;
        }
        [self.scroll_stock, self.item_stock, self.special_stock] = self.db.load_shop()?;
        self.catalogue = self.db.items()?;
        Ok(())
    }

    /// (scrolls, items) in the catalogue
    pub fn pool_sizes(&self) -> (usize, usize) {
        let scrolls = self.catalogue.iter().filter(|i| i.level.is_some()).count();
        (scrolls, self.catalogue.len() - scrolls)
    }

    /// Write settings to the database if they've changed since the last save.
    /// Cheap to call every frame.
    pub fn save_settings_if_changed(&mut self) {
        if self.settings == self.saved_settings {
            return;
        }
        // remember these even on failure so a broken database isn't retried every frame
        self.saved_settings = self.settings;
        if let Err(err) = self.db.save_settings(&self.settings) {
            self.status = StockStatus::Error(format!("couldn't save settings: {err}"));
        }
    }

    /// Add or update catalogue items from a CSV file
    pub fn import_csv(&mut self, path: &Path) {
        let result = read_csv(path).and_then(|items| Ok(self.db.upsert_items(&items)?));
        self.status = match result {
            Ok(summary) => StockStatus::Imported(summary),
            Err(err) => StockStatus::Error(format!("import failed: {err}")),
        };
        self.reload_catalogue();
    }

    pub fn add_item(&mut self, item: &StockItem) -> Result<(), String> {
        let result = self.db.insert_item(item).map(drop);
        self.after_edit(result)
    }

    pub fn update_item(&mut self, id: i64, item: &StockItem) -> Result<(), String> {
        let result = self.db.update_item(id, item);
        self.after_edit(result)
    }

    pub fn delete_item(&mut self, id: i64) -> Result<(), String> {
        let result = self.db.delete_item(id);
        self.after_edit(result)
    }

    fn after_edit(&mut self, result: rusqlite::Result<()>) -> Result<(), String> {
        result.map_err(|err| match err.sqlite_error_code() {
            Some(ErrorCode::ConstraintViolation) => {
                "an item with that name, rarity and level already exists".to_string()
            }
            _ => err.to_string(),
        })?;
        self.reload_catalogue();
        Ok(())
    }

    fn reload_catalogue(&mut self) {
        match self.db.items() {
            Ok(items) => self.catalogue = items,
            Err(err) => self.status = StockStatus::Error(err.to_string()),
        }
    }

    /// Generate a fresh shop from the catalogue and save it.
    /// Returns false if there are no scrolls or no items to draw from.
    pub fn generate_shop(&mut self) -> bool {
        // check that we actually have shop stock pool to look at
        let (scroll_count, item_count) = self.pool_sizes();
        if scroll_count == 0 || item_count == 0 {
            return false;
        }

        // new RNG whenever we re-generate
        let mut rng = rand::rng();
        let settings = self.settings;
        // clear current stocks
        self.item_stock.clear();
        self.scroll_stock.clear();
        self.special_stock.clear();

        // bucket the pools by tier so weighted picks are a tier draw
        // followed by a uniform draw within that tier
        let item_buckets = bucket_by_rarity(&self.catalogue);
        let level_buckets = bucket_by_level(&self.catalogue);

        let item_choices: Vec<(u32, &Vec<&StockItem>)> = (0..=settings.max_item_rarity as usize)
            .map(|r| (rarity_weight(r), &item_buckets[r]))
            .collect();

        while self.item_stock.len() < settings.max_items {
            match weighted_pick(&item_choices, &mut rng) {
                Some(chosen) => self.item_stock.push(chosen.clone()),
                None => break, // nothing in the pool qualifies; stop rather than spin forever
            }
        }

        let scroll_choices: Vec<(u32, &Vec<&StockItem>)> = (0..=settings.max_scroll_level)
            .map(|l| (scroll_level_weight(l), &level_buckets[l]))
            .collect();

        while self.scroll_stock.len() < settings.max_scrolls {
            match weighted_pick(&scroll_choices, &mut rng) {
                Some(chosen) => self.scroll_stock.push(chosen.clone()),
                None => break,
            }
        }

        // specials draw from item and scroll tiers combined into a single bucket
        let special_item_choices = (settings.min_special_rarity as usize
            ..=settings.max_special_rarity as usize)
            .map(|r| (rarity_weight(r), &item_buckets[r]));
        let special_scroll_choices = (settings.min_special_scroll_level..=MAX_SCROLL_LEVEL)
            .map(|l| (scroll_level_weight(l), &level_buckets[l]));
        let special_choices: Vec<(u32, &Vec<&StockItem>)> =
            special_item_choices.chain(special_scroll_choices).collect();

        while self.special_stock.len() < settings.max_specials {
            match weighted_pick(&special_choices, &mut rng) {
                Some(chosen) => self.special_stock.push(chosen.clone()),
                None => break,
            }
        }

        // show scrolls sorted by level, then items sorted by rarity
        self.special_stock
            .sort_by_key(|item| match (item.level, item.rarity) {
                (Some(level), _) => (0u8, level, Rarity::default()),
                (_, Some(rarity)) => (1u8, 0usize, rarity),
                (None, None) => (2u8, 0usize, Rarity::default()),
            });

        self.scroll_stock.sort_by_key(|item| item.level);
        self.item_stock.sort_by_key(|item| item.rarity);

        if let Err(err) =
            self.db
                .save_shop([&self.scroll_stock, &self.item_stock, &self.special_stock])
        {
            self.status = StockStatus::Error(format!("couldn't save the shop: {err}"));
        }
        true
    }

    pub fn has_stock(&self) -> bool {
        !(self.scroll_stock.is_empty()
            && self.item_stock.is_empty()
            && self.special_stock.is_empty())
    }
}

/// parse a stock CSV with columns name, price, category, rarity, level
fn read_csv(path: &Path) -> Result<Vec<StockItem>, Box<dyn Error>> {
    let mut items = Vec::new();
    let mut reader = csv::Reader::from_path(path)?;
    for result in reader.records() {
        let record = result?;

        let name = record.get(0).unwrap_or_default().trim().to_string();
        let price: usize = record
            .get(1)
            .unwrap_or_default()
            .trim()
            .parse()
            .unwrap_or_default();
        let category = record.get(2).unwrap_or_default().trim().to_string();
        let rarity_str = record.get(3).unwrap_or_default().trim().to_lowercase();
        let level_str = record.get(4).unwrap_or_default().trim();

        let rarity = if rarity_str.is_empty() {
            None
        } else {
            Some(match rarity_str.as_str() {
                "uncommon" => Rarity::Uncommon,
                "rare" => Rarity::Rare,
                "very rare" => Rarity::VeryRare,
                "legendary" => Rarity::Legendary,
                _ => Rarity::Common,
            })
        };

        let level = if level_str.is_empty() {
            None
        } else if level_str.eq_ignore_ascii_case("c") {
            Some(0) // cantrip
        } else {
            Some(level_str.parse::<usize>().unwrap_or_default())
        };

        items.push(StockItem {
            id: None,
            name,
            price,
            category,
            rarity,
            level,
        });
    }
    Ok(items)
}

/// sort items into buckets by rarity (index 0 = Common .. 4 = Legendary)
fn bucket_by_rarity(pool: &[StockItem]) -> [Vec<&StockItem>; 5] {
    let mut buckets: [Vec<&StockItem>; 5] = std::array::from_fn(|_| Vec::new());
    for item in pool {
        if let Some(rarity) = item.rarity {
            buckets[rarity as usize].push(item);
        }
    }
    buckets
}

/// sort scrolls into buckets by level (index 0..=MAX_SCROLL_LEVEL).
fn bucket_by_level(pool: &[StockItem]) -> Vec<Vec<&StockItem>> {
    let mut buckets: Vec<Vec<&StockItem>> = (0..=MAX_SCROLL_LEVEL).map(|_| Vec::new()).collect();
    for item in pool {
        if let Some(bucket) = item.level.and_then(|level| buckets.get_mut(level)) {
            bucket.push(item);
        }
    }
    buckets
}

/// assign weights to rarity levels out of 100
fn rarity_weight(rarity_index: usize) -> u32 {
    const WEIGHTS: [u32; 5] = [30, 25, 20, 15, 10];
    WEIGHTS.get(rarity_index).copied().unwrap_or(1)
}

/// assign weights to scroll levels (10 - level)
fn scroll_level_weight(level: usize) -> u32 {
    ((MAX_SCROLL_LEVEL + 1).saturating_sub(level) as u32).max(1)
}

/// pick an item from weighted tiers by choosing a tier first
/// then pick uniformly inside the tier
/// return none if every bucket is empty
fn weighted_pick<'a>(
    choices: &[(u32, &Vec<&'a StockItem>)],
    rng: &mut impl Rng,
) -> Option<&'a StockItem> {
    let nonempty: Vec<&(u32, &Vec<&StockItem>)> = choices
        .iter()
        .filter(|(_, bucket)| !bucket.is_empty())
        .collect();
    if nonempty.is_empty() {
        return None;
    }

    let weights: Vec<u32> = nonempty.iter().map(|(weight, _)| *weight).collect();
    let dist = WeightedIndex::new(&weights).ok()?;
    let (_, bucket) = nonempty[dist.sample(rng)];
    bucket.choose(rng).copied()
}

#[derive(Debug, Default, Clone)]
pub enum StockStatus {
    /// Database opened fine; nothing to report
    #[default]
    Ready,
    Imported(ImportSummary),
    Error(String),
}

#[derive(Debug, Default, PartialEq, PartialOrd, Eq, Ord, Clone, Copy)]
pub enum Rarity {
    #[default]
    Common,
    Uncommon,
    Rare,
    VeryRare,
    Legendary,
}

impl Rarity {
    pub const ALL: [Rarity; 5] = [
        Rarity::Common,
        Rarity::Uncommon,
        Rarity::Rare,
        Rarity::VeryRare,
        Rarity::Legendary,
    ];
}

impl fmt::Display for Rarity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// how many of what to put in the shop
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub max_scrolls: usize,
    pub max_scroll_level: usize,
    pub min_special_scroll_level: usize,
    pub max_items: usize,
    pub max_item_rarity: Rarity,
    pub max_specials: usize,
    pub min_special_rarity: Rarity,
    pub max_special_rarity: Rarity,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_scrolls: 20,
            max_scroll_level: 3,
            min_special_scroll_level: 4,
            max_items: 10,
            max_item_rarity: Rarity::Uncommon,
            max_specials: 5,
            min_special_rarity: Rarity::Rare,
            max_special_rarity: Rarity::VeryRare,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StockItem {
    /// row id in the items table; None for items not (yet) in the catalogue
    pub id: Option<i64>,
    pub level: Option<usize>,
    pub rarity: Option<Rarity>,
    pub name: String,
    pub category: String,
    pub price: usize,
}
