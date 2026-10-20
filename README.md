# werlen-gui

An [egui] port/fork of [werlen](../werlen), the TUI that generates stock lists for the magic item shops run by the not-so-humble retired adventurer Werlen in my D&D games.
He stocks a variety of spell scrolls, low-rarity items, and a special stock of the good stuff for those that pay for his subscription service.

Users are responsible for providing their own shop stock, obtained with purchase(s) from WoTC or limited to the SRD. Stock is kept in a SQLite database (`werlen.db` in the platform data directory, e.g. `~/.local/share/werlen-gui/` on Linux), so it only needs to be added once. There are two ways to add it:

- **Import CSV…** reads a CSV file with columns `name`, `price`, `category`, `rarity` for items (leave empty for scrolls), and `level` for scrolls (leave empty for items; `C` for cantrips). Entries are matched on name (ignoring case), rarity, and level: matches have their price and category updated, anything new is added, and nothing is removed. Importing the same file again is safe.
- The **Catalogue** tab lists everything in the database, with search and filters, and lets you add, edit, or delete entries by hand.

The size of the three stock pools and the spell level/rarity range of items in the stock pools are configurable on the Settings tab. Generation is slightly weighted towards lower level/rarity items. Settings and the last generated shop are saved too, so they're still there when you reopen the app.

[egui]: https://github.com/emilk/egui

### AI disclosure:
Code in this repo is written with the assistance of AI agents, as a personal exercise to expand my expertise with AI-enabled coding practices. I am a full-time software engineer and review all code before publishing.
