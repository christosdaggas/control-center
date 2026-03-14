# Control Center

**Linux System Change & Activity Timeline Viewer**

A GTK4/libadwaita application that presents a unified timeline of meaningful system changes, turning logs and events into an understandable narrative.

## Features

- 📊 **Unified Timeline**: Aggregate events from journald, package managers, kernel, and more
- 🔍 **Smart Filtering**: Preset filters for "Since Last Reboot", "Warnings & Errors", "Changes Only"
- 🔗 **Correlation Engine**: Rule-based grouping of related events with transparent reasoning
- 📝 **Human-Readable Summaries**: Clear explanations with expandable raw evidence
- 🎨 **Cross-Desktop**: Works on GNOME, KDE Plasma, and COSMIC
- 🔒 **Read-Only**: Never modifies system state, only reads

## Building

### Requirements

- Rust 1.75+
- GTK4 4.12+
- libadwaita 1.4+
- systemd (for journald access)

### Fedora

```bash
sudo dnf install gtk4-devel libadwaita-devel
cargo build --release
```

### Ubuntu

```bash
sudo apt install libgtk-4-dev libadwaita-1-dev
cargo build --release
```

### Running

```bash
cargo run
```

## Architecture

```
src/
├── domain/         # Pure business logic (events, correlation, filtering)
├── infrastructure/ # System adapters (journald, package managers, desktop)
├── application/    # State management and use cases
└── ui/             # GTK4/libadwaita widgets and pages
```


## Correlation Rules

The engine uses deterministic, rule-based correlation:

1. **Package → Service Restart**: Package update followed by service restart
2. **Service Cascade**: One service failure causing others to fail
3. **Disk → Write Failures**: Disk space issues causing service errors
4. **Permission Denial**: SELinux/AppArmor blocks causing failures

Each correlation includes:
- Confidence score (0-100)
- Explanation of why events are grouped
- Links to raw evidence

## Author

**Christos A. Daggas**
- Website: [chrisdaggas.com](https://chrisdaggas.com)
- Email: info@chrisdaggas.com

## License

Copyright © 2026 Christos A. Daggas

MIT License
