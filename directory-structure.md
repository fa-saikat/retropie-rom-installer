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
    ├── assets.rs                <- on-disk asset lookup + fallback logic
    ├── library.rs
    ├── main.rs
    ├── systems.rs
    ├── theme.rs
    └── ui
        ├── mod.rs
        └── root.rs
```





