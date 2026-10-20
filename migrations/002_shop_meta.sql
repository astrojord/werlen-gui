-- when the shop saved in shop_stock was generated
CREATE TABLE shop_meta (
    id           INTEGER PRIMARY KEY CHECK (id = 1),
    generated_at INTEGER NOT NULL -- unix seconds
);
