MOTD
====

[![Build status](https://img.shields.io/travis/desbma/motd/master.svg?style=flat)](https://travis-ci.org/desbma/motd)
[![AUR version](https://img.shields.io/aur/version/motd.svg?style=flat)](https://aur.archlinux.org/packages/motd/)
[![License](https://img.shields.io/github/license/desbma/motd.svg?style=flat)](https://github.com/desbma/motd/blob/master/LICENSE)

Dynamically generate Linux MOTD SSH banner


## Goals

* Should be very fast (no perceived visual latency, even under high load)
* Display relevant system information, and colorize anormal measures in orange if something is suspicious, red if it requires immediate action
* Be reasonably portable across Linux boxes (rsync'ing the binary should work)
* Learn Rust :)

## Information displayed

* system load (orange/red if close/above CPU count)
* memory/swap usage
* filesystem usage (orange/red if almost full)
* hardware temperatures (CPU, HDD...) (orange/red if too hot)
* network interface bandwidth
* Systemd units in failed state (red)


## Screenshot

[![Imgur](https://i.imgur.com/OPrRqKzl.png)](https://i.imgur.com/OPrRqKz.png)


## License

[GPLv3](https://www.gnu.org/licenses/gpl-3.0-standalone.html)
