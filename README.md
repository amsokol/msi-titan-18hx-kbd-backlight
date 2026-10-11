# Подсветка клавиатуры MSI Titan 18 HX на Linux (и Windows)

Небольшая программа на Rust: включает подсветку клавиатуры ноутбука
**MSI Titan 18 HX A14VIG** и задаёт всем клавишам один цвет
(по умолчанию красный). Она запускается сама при загрузке и после выхода
из сна, так что кнопка подсветки не нужна.

Для Linux у SteelSeries программы нет, а поддержка RGB в ядре
(`hid-steelseries`) включена только для MSI Raider A18 HX. На Windows
программа может заменить тяжёлый SteelSeries GG.

| Платформа | Состояние |
|---|---|
| Linux | работает, проверено |
| Windows | код написан и компилируется, **на железе не проверялся** |

## Структура

| Путь | Что это |
|---|---|
| `src/protocol.rs` | байты команд контроллера, общие для обеих ОС |
| `src/main.rs` | разбор аргументов, порядок команд, паузы и повторы |
| `src/linux.rs` | отправка команд через `/dev/hidrawN` |
| `src/windows.rs` | отправка команд через `hid.dll` (без драйвера) |
| `install.sh`, `uninstall.sh`, `dist/linux/` | установка на Linux: systemd и udev |
| `install.ps1`, `uninstall.ps1`, `dist/windows/` | установка на Windows: Планировщик заданий |
| `tools/klc-probe.py` | Python-скрипт для экспериментов и поиска кодов клавиш |

Нужная платформа выбирается при сборке автоматически (`#[cfg(target_os)]`),
ничего включать не нужно.

## Оборудование

| | |
|---|---|
| Ноутбук | MSI Titan 18 HX A14VIG (плата MS-1822, BIOS E1822IMS.117) |
| Контроллер подсветки | SteelSeries KLC, USB `1038:1122`, интерфейс 0 |
| Проверено на | ядре Linux 7.3 (Ubuntu) |

Проверить, что контроллер на месте:

```bash
lsusb | grep 1038:1122
```

## Установка на Linux (в том числе после переустановки системы)

1. Установите Rust, если его ещё нет:

   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. Склонируйте репозиторий и запустите установку **от обычного пользователя**
   (скрипт сам спросит пароль sudo):

   ```bash
   git clone git@github.com:amsokol/msi-titan-18hx-kbd-backlight-linux.git
   cd msi-titan-18hx-kbd-backlight-linux
   ./install.sh
   ```

Подсветка включится сразу. Что ставится в систему:

| Файл | Зачем |
|---|---|
| `/usr/local/bin/titan-kbd-light` | сама программа |
| `/etc/systemd/system/titan-kbd-light.service` | служба: запускается при загрузке и после выхода из сна |
| `/etc/udev/rules.d/99-titan-kbd-light.rules` | запускает службу, когда появляется контроллер клавиатуры |
| `/etc/default/titan-kbd-light` | настройка цвета |

## Смена цвета

Поменяйте `COLOR` в `/etc/default/titan-kbd-light` (формат `RRGGBB` или `off`)
и примените:

```bash
sudo systemctl start titan-kbd-light
```

Разово, без изменения настройки:

```bash
sudo titan-kbd-light 00ff00
```

## Проверка и диагностика

```bash
systemctl status titan-kbd-light
```

```bash
journalctl -b -u titan-kbd-light
```

Если программа пишет `SteelSeries KLC (1038:1122, interface 0) not found`,
значит, контроллера нет или у него другой USB ID (см. `lsusb`). Программа
перед этим 10 секунд ждёт его появления, например после выхода из сна.

## Удаление

```bash
./uninstall.sh
```

## Windows (не проверено)

Код для Windows написан, но ещё не запускался на настоящем ноутбуке.
Первая установка — по сути тест.

1. Удалите или отключите SteelSeries GG и управление подсветкой в
   MSI Center, иначе они будут перебивать цвет.
2. Установите Rust: <https://rustup.rs>. Нужен MSVC toolchain (rustup
   предложит Visual Studio Build Tools).
3. В PowerShell **от администратора** из папки репозитория:

   ```powershell
   powershell -ExecutionPolicy Bypass -File .\install.ps1
   ```

   Другой цвет: `.\install.ps1 -Color 00ff00`.

Скрипт собирает `titan-kbd-light.exe`, копирует его в
`C:\Program Files\titan-kbd-light\` и создаёт задачу `titan-kbd-light` в
Планировщике заданий. Задача запускается от SYSTEM при загрузке, при входе
любого пользователя (при включённом «Быстром запуске» событие загрузки не
срабатывает) и после выхода из сна.

Проверка вручную, без установки:

```powershell
cargo build --release
.\target\release\titan-kbd-light.exe ff0000
```

Удаление: `powershell -ExecutionPolicy Bypass -File .\uninstall.ps1`.

Если не заработает, полезно знать:
- `not found` — Windows не нашла интерфейс `VID_1038&PID_1122&MI_00`.
  Путь устройства можно посмотреть в Диспетчере устройств: «Устройства HID»,
  свойства, «Сведения», «Путь к экземпляру устройства».
- `Access is denied` — устройство занято другой программой (GG, MSI Center).
- Подсветка включилась, но клавиши частично голубые — увеличьте
  `WAKE_SETTLE` в `src/main.rs`.

## Как это работает

Пришлось выяснять опытным путём на этом ноутбуке. Контроллер понимает два
протокола, и для Titan нужны оба: первый включает подсветку, второй задаёт
цвета. Все команды отправляются в `/dev/hidrawN` интерфейса 0 как HID feature
report 524 байта с report ID 0 (то есть USB SET_REPORT с `wValue 0x0300`).

**1. Включение (старый протокол, как в msi-perkeyrgb).** Для каждой из
четырёх групп клавиш отправляется feature report:

```
0e 00 GROUP 00 | 42 × (R G B 00 00 00 00 00 00 01 00 KEY) | 00×14 08 39
```

После каждого — два output report по 64 байта: `0d 00 02 00…` и `09 00…`.
Группы: `2a` буквы/цифры, `0b` Enter, `18` модификаторы, `24` цифровой блок.
На Titan после этого подсветка **включается со встроенной голубой темой, а
переданные цвета игнорируются**.

Осторожно: третий байт заголовка прошивка понимает как номер группы. Если
отправить пакет с другим числом в этом байте (например, 40 или 22), часть
клавиш (Esc, Tab, Caps, левые Shift и Ctrl) сбрасывается в цвет темы.

**2. Цвет (протокол из патча ядра для MSI Raider A18 HX).** Один feature report:

```
0c 00 66 00 | (KEY R G B) × N | неиспользуемые записи: KEY = ff
```

`KEY` — коды клавиш HID (usage ID): `04` = A, `e0` = левый Ctrl и т. д.
**Titan читает только примерно первые 100 записей**, поэтому модификаторы
(`e0`–`e7`, `f0`) идут первыми. Если поставить их в конец, Windows, AltGr,
правые Ctrl и Shift останутся голубыми.

## Инструменты

`tools/klc-probe.py` — исходный Python-скрипт, на котором всё это
выяснялось. В нём есть режимы перебора кодов клавиш (`sweep` для старого
протокола и `newsweep` для нового): клавиши по одной подсвечиваются
зелёным. Пригодится, если на другой модели часть клавиш не окрасится.

```bash
sudo python3 tools/klc-probe.py ff0000 both
```

## Благодарности

- Патч `HID: steelseries: Add MSI Raider A18 HX A9WJG RGB support` для ядра
  Linux — протокол `0c 00 66`.
- [msi-perkeyrgb](https://github.com/Askannz/msi-perkeyrgb) — протокол `0e`.
- [msi-lightbar-linux](https://github.com/fenyxsomnia/msi-lightbar-linux) —
  команды `0d 00 02` / `09` и ловушка с номером группы.

## Лицензия

[MIT](LICENSE)
