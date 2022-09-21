# MOTD

[![Build status](https://github.com/desbma/motd/actions/workflows/ci.yml/badge.svg)](https://github.com/desbma/motd/actions)
[![AUR version](https://img.shields.io/aur/version/motd.svg?style=flat)](https://aur.archlinux.org/packages/motd/)
[![License](https://img.shields.io/github/license/desbma/motd.svg?style=flat)](https://github.com/desbma/motd/blob/master/LICENSE)

Dynamically generate Linux MOTD SSH banner

## Goals

- Should be very fast (no perceived visual latency, even under high load)
- Display relevant system information, and colorize anormal measures in orange if something is suspicious, red if it requires immediate action
- Be reasonably portable across Linux boxes (rsync'ing the binary should work)
- Learn Rust :)

## Information displayed

- system load (orange/red if close/above CPU count)
- memory/swap usage
- filesystem usage (orange/red if almost full)
- hardware temperatures (CPU, HDD...) (orange/red if too hot)
- network interface bandwidth
- Systemd units in failed state (red)

## Screenshot

[![Imgur](https://i.imgur.com/OPrRqKzl.png)](https://i.imgur.com/OPrRqKz.png)

## Installation

### From source

You need a Rust build environment for example from [rustup](https://rustup.rs/).

```
cargo build --release
install -Dm 755 -t /usr/local/bin target/release/motd
```

### From the AUR

Arch Linux users can install the [motd AUR package](https://aur.archlinux.org/packages/motd/).

## Configuration

Configuration is **optional**, and allows you to exclude for example some filesystems or temperature sensors based on regular expressions.

Example of `~/.config/motd/config.toml` config file:

```
[fs]
mount_path_blacklist = ["^/dev($|/)", "^/run($|/)"]
mount_type_blacklist = ["^tmpfs$"]

[temp]
hwmon_label_blacklist = ["^CPUTIN$", "^SYSTIN$"]

```

## License

[GPLv3](https://www.gnu.org/licenses/gpl-3.0-standalone.html)
