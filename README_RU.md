# Parsec Virtual Display Plugin

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![macOS](https://img.shields.io/badge/macOS-11%2B-brightgreen.svg)]()
[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange.svg)]()
[![Platform](https://img.shields.io/badge/Platform-Apple%20Silicon%20%7C%20Intel-lightgrey.svg)]()

**[English](README.md)**

Лёгкое приложение в меню-баре macOS для автоматического управления виртуальными дисплеями для Parsec.

Настрой разрешение один раз и забудь — приложение само всё делает когда клиенты подключаются и отключаются. Идеально для headless Mac (Mac Mini серверы, удалённые рабочие станции) без подключённого монитора.

## Как работает

- Следит за лог-файлом Parsec
- Автоматически создаёт виртуальный дисплей с нужным разрешением/fps при подключении
- Автоматически удаляет его при отключении
- Тихо сидит в меню-баре, никакого обслуживания после настройки

## Установка

Скачай `.dmg` из [Releases](../../releases), открой, перетащи в Applications.

## Сборка

```bash
cargo run          # dev
./build.sh         # release, результат в dist/
```

## Требования

- macOS 11+
- Parsec

## Стек

- Rust + Dioxus
- CoreGraphics private API для виртуальных дисплеев
- objc2 для интеграции с macOS

## FAQ

**Это безопасно?**
Использует тот же private API что BetterDisplay и другие утилиты. Работает на macOS 11-15, ожидается совместимость с будущими версиями.

**Почему не BetterDisplay?**
BetterDisplay отличный, но платный ($18) и делает намного больше чем нужно. Это бесплатно, сфокусировано и автоматически управляет дисплеями.

**ARM или Intel?**
Оба. Универсальный бинарник работает на Apple Silicon и Intel Mac.

## Лицензия

MIT
