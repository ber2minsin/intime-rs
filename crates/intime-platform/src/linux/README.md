# Linux platform notes

## Window events

- **Sway** (`SWAYSOCK` set): Sway IPC window focus/title events.
- **Otherwise** (GNOME, etc.): AT-SPI2 over D-Bus.

Requires `at-spi2-core` running for the AT-SPI path.

## Screenshots

- **Sway/wlroots**: `grim` region capture using window geometry from Sway.
- **Fallback**: XDG Desktop Portal (`org.freedesktop.portal.Screenshot`).

### Fedora packages

```bash
sudo dnf install wayland-devel wayland-protocols-devel \
  pipewire-devel clang-devel mesa-libEGL-devel mesa-libgbm-devel \
  grim at-spi2-core xdg-desktop-portal-wlr
```
