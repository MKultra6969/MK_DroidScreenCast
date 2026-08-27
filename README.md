[🇺🇸English](https://github.com/MKultra6969/MK_DroidScreenCast/blob/main/README_ENG.md)
<div align="center">

# 📱 MK DroidScreenCast v1.0.2

**Полноценное настольное приложение для управления Android на базе ADB и Scrcpy**
<br>
*Подключайте устройство, запускайте scrcpy, записывайте экран и управляйте файлами в одном окне.*

[![Tauri](https://img.shields.io/badge/Tauri-Desktop-24C8DB?style=for-the-badge&logo=tauri&logoColor=white)](https://tauri.app/)
[![Scrcpy](https://img.shields.io/badge/Powered_by-Scrcpy-green?style=for-the-badge&logo=android)](https://github.com/Genymobile/scrcpy)
[![Version](https://img.shields.io/badge/Version-1.0.2-2ea44f?style=for-the-badge)](#)
[![License](https://img.shields.io/badge/License-WTFPL-red?style=for-the-badge)](http://www.wtfpl.net/)

</div>

---

## О проекте

**MK DroidScreenCast** — полноценное настольное приложение, которое объединяет `adb` и `scrcpy` в удобный интерфейс. Вся логика живёт внутри самого приложения: отдельный процесс не поднимается и сетевой порт не открывается, инструменты скачиваются автоматически, всё работает в одном окне.

---

## Интерфейс

> <img width="2490" height="1312" alt="showcase" src="https://github.com/user-attachments/assets/2d5e608a-a59e-494e-bb03-e8499dd1f990" />



---

## Возможности

### Устройства
*   USB, Wi-Fi pairing (Android 11+), USB -> Wi-Fi (TCP/IP).
*   Сохраненные устройства и быстрый коннект.
*   Автовыбор подключения.

### Scrcpy и запись
*   Пресеты для битрейта и максимального размера.
*   Режимы клавиатуры и общие опции (не гасить экран, показать касания, fullscreen, без звука, выключить экран).
*   HUD записи с выбором формата, источника звука и папки сохранения.

### Файлы и диагностика
*   Файловый менеджер (push/pull) и галерея скриншотов.
*   Выгрузка `logs.zip` и проверка обновлений.
*   Редактор конфига и переключение RU/EN.

---

## Установка

### Релиз
1.  Скачайте последнюю версию из GitHub Releases: https://github.com/MKultra6969/MK_DroidScreenCast/releases
2.  Установите и запустите MK DroidScreenCast.

### Сборка из исходников
См. `docs/build.md` для зависимостей (Node 18+, Rust 1.85+) и команд сборки.

---

## Использование

1.  Откройте MK DroidScreenCast.
2.  Подключите устройство по USB или через Wi-Fi pairing.
3.  Выберите пресет scrcpy и запустите трансляцию.
4.  Используйте запись, файлы и диагностику по необходимости.

---

## Конфигурация

Настройки лежат в `config.json` и доступны в приложении в разделе Настройки > Конфигурация.

---

## Структура проекта

```text
MK_DroidScreenCast/
├── frontend/          # Desktop UI (React + Tauri)
├── src-tauri/         # Rust: логика приложения и IPC
├── downloads/         # ADB/Scrcpy cache
├── logs/              # Logs and diagnostics
├── config.json        # App settings
└── docs/              # Build notes
```

---

## Подготовка телефона

1.  Настройки -> О телефоне -> 7 раз на "Номер сборки".
2.  Настройки -> Система -> Для разработчиков.
3.  Включите отладку по USB.
4.  Для Wi-Fi (Android 11+): включите Wireless debugging.

---

## Автор

**MKultra69**

*   GitHub: [@MKultra6969](https://github.com/MKultra6969)
*   Telegram Channel: [@MKplusULTRA](https://t.me/MKplusULTRA)

## P.S.
* Как всегда, все очевидно, лицензия как всегда, отношение к людям как всегда.
