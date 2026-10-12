```
.
├── Cargo.lock
├── Cargo.toml
├── LICENSE
├── README.md
├── scripts/
│   ├── build-deb.sh
│   └── generate-changelog.sh
├── assets/
│   ├── background.jpg
│   ├── retropie-rom-manager.svg
│   └── icons/
│       ├── psx.svg
│       ├── arcade.svg
│       ├── dreamcast.svg
│       ├── gba.svg
│       ├── megadrive.svg
│       ├── n64.svg
│       └── device-gamepad.svg   <- generic fallback (matches SystemDef::icon)
├── packaging/
│   ├── DEBIAN/
│   │   ├── postinst
│   │   └── postrm
│   ├── changelog
│   ├── copyright
│   └── retropie-rom-manager.desktop
└── src
    ├── assets.rs                <- embedded UI icons + on-disk system icons
    ├── library.rs
    ├── main.rs
    ├── scraper.rs               <- Skyscraper runs + gamelist.xml reading
    ├── skyscraper_setup.rs      <- installing Skyscraper via RetroPie-Setup
    ├── systems.rs
    ├── theme.rs                 <- system accent on top of GPUI Kit's themes
    └── ui
        ├── mod.rs
        ├── root.rs              <- app state + actions (install, scrape, delete)
        ├── sidebar.rs
        ├── library.rs           <- header, notices, toolbar, grid / list
        └── details.rs           <- game details sheet + dialogs
```





