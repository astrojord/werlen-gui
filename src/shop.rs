use rand::Rng;
use rand::distr::Distribution;
use rand::distr::weighted::WeightedIndex;
use rand::seq::IndexedRandom;
use std::path::PathBuf;
use std::{error::Error, fmt};

/// max spell level within 5e
pub const MAX_SCROLL_LEVEL: usize = 9;

/// upper bound on the size of any one stock list
pub const POOL_MAX: usize = 128;

#[derive(Debug)]
pub struct Shop {
    /// Whether the stock source CSV has been loaded, and what happened last time we tried
    pub stock_status: StockStatus,

    /// Settings
    pub max_scrolls: usize,
    pub max_scroll_level: usize,
    pub min_special_scroll_level: usize,
    pub max_items: usize,
    pub max_item_rarity: Rarity,
    pub max_specials: usize,
    pub max_special_rarity: Rarity,
    pub min_special_rarity: Rarity,
    pub stock_source: PathBuf,

    // Stock pools, populated from stock source
    scroll_stock_pool: Vec<StockItem>,
    item_stock_pool: Vec<StockItem>,

    /// Current shop stock, taken from pools
    pub scroll_stock: Vec<StockItem>,
    pub item_stock: Vec<StockItem>,
    pub special_stock: Vec<StockItem>,
}

impl Default for Shop {
    fn default() -> Self {
        Self {
            stock_status: StockStatus::default(),
            max_scrolls: 20,
            max_scroll_level: 3,
            min_special_scroll_level: 4,
            max_items: 10,
            max_item_rarity: Rarity::Uncommon,
            max_specials: 5,
            max_special_rarity: Rarity::VeryRare,
            min_special_rarity: Rarity::Rare,
            stock_source: PathBuf::from(r"/home/jordan/code/werlen/stock_source.csv"),
            scroll_stock_pool: Vec::new(),
            item_stock_pool: Vec::new(),
            scroll_stock: Vec::new(),
            item_stock: Vec::new(),
            special_stock: Vec::new(),
        }
    }
}

impl Shop {
    pub fn update_stock_pools(&mut self) {
        self.item_stock_pool.clear();
        self.scroll_stock_pool.clear();

        match self.read_csv() {
            Ok(()) => {
                self.stock_status = StockStatus::Loaded {
                    scrolls: self.scroll_stock_pool.len(),
                    items: self.item_stock_pool.len(),
                };
            }
            Err(err) => {
                self.stock_status = StockStatus::Error(err.to_string());
            }
        }
    }

    fn read_csv(&mut self) -> Result<(), Box<dyn Error>> {
        // read csv and populate the stock pools
        let mut reader = csv::Reader::from_path(self.stock_source.as_path())?;
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
            } else {
                Some(level_str.parse::<usize>().unwrap_or_default())
            };

            let new_stock_item = StockItem {
                id: None,
                name,
                price,
                category,
                rarity,
                level,
            };

            if level.is_some() {
                self.scroll_stock_pool.push(new_stock_item);
            } else {
                self.item_stock_pool.push(new_stock_item);
            }
        }
        Ok(())
    }

    /// Generate a fresh shop from the stock pools.
    /// Returns false if the pools haven't been loaded.
    pub fn generate_shop(&mut self) -> bool {
        // check that we actually have shop stock pool to look at
        if self.item_stock_pool.is_empty() || self.scroll_stock_pool.is_empty() {
            return false;
        }

        // new RNG whenever we re-generate
        let mut rng = rand::rng();
        // clear current stocks
        self.item_stock.clear();
        self.scroll_stock.clear();
        self.special_stock.clear();

        // bucket the pools by tier so weighted picks are a tier draw
        // followed by a uniform draw within that tier
        let item_buckets = bucket_by_rarity(&self.item_stock_pool);
        let level_buckets = bucket_by_level(&self.scroll_stock_pool);

        let item_choices: Vec<(u32, &Vec<&StockItem>)> = (0..=self.max_item_rarity as usize)
            .map(|r| (rarity_weight(r), &item_buckets[r]))
            .collect();

        while self.item_stock.len() < self.max_items {
            match weighted_pick(&item_choices, &mut rng) {
                Some(chosen) => self.item_stock.push(chosen.clone()),
                None => break, // nothing in the pool qualifies; stop rather than spin forever
            }
        }

        let scroll_choices: Vec<(u32, &Vec<&StockItem>)> = (0..=self.max_scroll_level)
            .map(|l| (scroll_level_weight(l), &level_buckets[l]))
            .collect();

        while self.scroll_stock.len() < self.max_scrolls {
            match weighted_pick(&scroll_choices, &mut rng) {
                Some(chosen) => self.scroll_stock.push(chosen.clone()),
                None => break,
            }
        }

        // specials draw from item and scroll tiers combined into a single bucket
        let special_item_choices = (self.min_special_rarity as usize
            ..=self.max_special_rarity as usize)
            .map(|r| (rarity_weight(r), &item_buckets[r]));
        let special_scroll_choices = (self.min_special_scroll_level..=MAX_SCROLL_LEVEL)
            .map(|l| (scroll_level_weight(l), &level_buckets[l]));
        let special_choices: Vec<(u32, &Vec<&StockItem>)> =
            special_item_choices.chain(special_scroll_choices).collect();

        while self.special_stock.len() < self.max_specials {
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
        true
    }

    pub fn has_stock(&self) -> bool {
        !(self.scroll_stock.is_empty()
            && self.item_stock.is_empty()
            && self.special_stock.is_empty())
    }
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
    #[default]
    NotLoaded,
    Loaded {
        scrolls: usize,
        items: usize,
    },
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
