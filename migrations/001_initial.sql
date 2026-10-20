CREATE TABLE items (
    id       INTEGER PRIMARY KEY,
    name     TEXT    NOT NULL,
    category TEXT    NOT NULL,
    price    INTEGER NOT NULL,
    rarity   INTEGER,          -- Rarity index, NULL for scrolls
    level    INTEGER           -- spell level, NULL for items
);
-- NULLs are distinct in a plain UNIQUE constraint, so key on IFNULL'd columns
CREATE UNIQUE INDEX items_key ON items (lower(name), ifnull(rarity, -1), ifnull(level, -1));

CREATE TABLE settings (
    id                       INTEGER PRIMARY KEY CHECK (id = 1),
    max_scrolls              INTEGER NOT NULL,
    max_scroll_level         INTEGER NOT NULL,
    min_special_scroll_level INTEGER NOT NULL,
    max_items                INTEGER NOT NULL,
    max_item_rarity          INTEGER NOT NULL,
    max_specials             INTEGER NOT NULL,
    min_special_rarity       INTEGER NOT NULL,
    max_special_rarity       INTEGER NOT NULL
);

-- a snapshot of the last generated shop, copied rather than linked to items
-- so that editing the catalogue doesn't rewrite an already generated shop
CREATE TABLE shop_stock (
    list     INTEGER NOT NULL, -- StockList index
    position INTEGER NOT NULL,
    name     TEXT    NOT NULL,
    category TEXT    NOT NULL,
    price    INTEGER NOT NULL,
    rarity   INTEGER,
    level    INTEGER,
    PRIMARY KEY (list, position)
);
