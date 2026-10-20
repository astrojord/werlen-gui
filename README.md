# werlen-gui

An [egui] port/fork of [werlen](../werlen), the TUI that generates stock lists for the magic item shops run by the not-so-humble retired adventurer Werlen in my D&D games.
He stocks a variety of spell scrolls, low-rarity items, and a special stock of the good stuff for those that pay for his subscription service.

Users are responsible for providing their own shop stock, obtained with purchase(s) from WoTC or limited to the SRD, via the `Stock source path` setting. The application expects a CSV file with columns `name`, `price`, `category`, `rarity` for items (leave empty for scrolls), and `level` for scrolls (leave empty for items).

The size of the three stock pools and the spell level/rarity range of items in the stock pools are configurable on the Settings tab. Generation is slightly weighted towards lower level/rarity items.

[egui]: https://github.com/emilk/egui

### AI disclosure:
Code in this repo is written with the assistance of AI agents, as a personal exercise to expand my expertise with AI-enabled coding practices. I am a full-time software engineer and review all code before publishing.
