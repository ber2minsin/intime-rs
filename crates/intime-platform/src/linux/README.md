# Linux platform notes

## Desktop detection

`detect_desktop()` picks a path from env (no compositor round-trips):

| Detection | Event source | Screenshots |
| --- | --- | --- |
| `SWAYSOCK` | Sway IPC + AT-SPI enrich + **document URL loop** | `grim` window region |
| `HYPRLAND_INSTANCE_SIGNATURE` / Hyprland desktop | AT-SPI primary | XDG portal |
| GNOME / Unity (`XDG_CURRENT_DESKTOP`) | AT-SPI primary | portal (`xdg-desktop-portal-gnome`) |
| KDE / Plasma | AT-SPI primary | portal (`xdg-desktop-portal-kde`) |
| Other Wayland / X11 | AT-SPI primary | portal |

MPRIS (media play/pause) always runs on the session bus.

Requires `at-spi2-core` for AT-SPI paths.

## Screenshots

- **Sway**: `grim` region capture using window geometry from Sway.
- **GNOME / KDE / Hyprland / generic**: XDG Desktop Portal Screenshot API.

### Fedora packages

```bash
# Core (all desktops)
sudo dnf install at-spi2-core dbus-devel

# Sway / wlroots
sudo dnf install grim xdg-desktop-portal-wlr

# GNOME
sudo dnf install xdg-desktop-portal-gnome

# KDE
sudo dnf install xdg-desktop-portal-kde
```
