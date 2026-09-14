```
.
├── Cargo.lock
├── Cargo.toml
├── README.md
├── assets/                     (optional, create if you want custom art)
│   ├── background.jpg          (or background.png / bg.jpg / bg.png)
│   └── icons/
│       ├── psx.svg              <- emulator-specific, checked first
│       ├── arcade.svg
│       ├── dreamcast.svg
│       ├── gba.svg
│       ├── megadrive.svg
│       ├── n64.svg
│       └── device-gamepad.svg   <- generic fallback (matches SystemDef::icon)
└── src
    ├── assets.rs                <- NEW: on-disk asset lookup + fallback logic
    ├── library.rs
    ├── main.rs
    ├── systems.rs
    ├── theme.rs
    └── ui
        ├── mod.rs
        └── root.rs
```





