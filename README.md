# Parsec Virtual Display Plugin

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![macOS](https://img.shields.io/badge/macOS-11%2B-brightgreen.svg)]()
[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange.svg)]()
[![Platform](https://img.shields.io/badge/Platform-Apple%20Silicon%20%7C%20Intel-lightgrey.svg)]()

**[Русский](README_RU.md)**

Lightweight menu bar app for macOS that automatically manages virtual displays for Parsec.

Set your resolution once, forget about it — the app handles everything when clients connect and disconnect. Perfect for headless Mac setups (Mac Mini servers, remote workstations) where no monitor is plugged in.

## How it works

- Watches Parsec log file for connection events
- Automatically creates virtual display with your resolution/refresh rate on connect
- Automatically destroys it on disconnect
- Sits quietly in menu bar, zero maintenance after setup

## Install

Download `.dmg` from [Releases](../../releases), open it, drag to Applications.

## Build

```bash
cargo run          # dev
./build.sh         # release, outputs to dist/
```

## Requirements

- macOS 11+
- Parsec

## Stack

- Rust + Dioxus
- CoreGraphics private API for virtual displays
- objc2 for macOS integration

## FAQ

**Is it safe?**
Uses the same private API as BetterDisplay and other utilities. Works on macOS 11-15, expected to work on future versions.

**Why not BetterDisplay?**
BetterDisplay is great but paid ($18) and does way more than needed. This is free, focused, and auto-manages displays.

**ARM or Intel?**
Both. Universal binary works on Apple Silicon and Intel Macs.

## License

MIT
