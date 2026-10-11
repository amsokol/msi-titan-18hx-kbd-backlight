# MSI Titan 18 HX keyboard backlight for Linux (and Windows)

A small Rust program that turns on the keyboard backlight of the
**MSI Titan 18 HX A14VIG** laptop and sets one color for all keys (red by
default). It runs automatically at boot and after resume, so the backlight
key is not needed.

SteelSeries has no software for Linux, and the in-kernel RGB support
(`hid-steelseries`) is enabled only for the MSI Raider A18 HX. On Windows
the program can replace the heavyweight SteelSeries GG.

| Platform | Status |
|---|---|
| Linux | works, tested |
| Windows | written and compiles, **not tested on hardware** |

## Layout

| Path | What it is |
|---|---|
| `src/protocol.rs` | controller command bytes, shared by both OSes |
| `src/main.rs` | argument parsing, command order, delays and retries |
| `src/linux.rs` | sends commands through `/dev/hidrawN` |
| `src/windows.rs` | sends commands through `hid.dll` (no driver needed) |
| `install.sh`, `uninstall.sh`, `dist/linux/` | Linux install: systemd and udev |
| `install.ps1`, `uninstall.ps1`, `dist/windows/` | Windows install: Task Scheduler |
| `tools/klc-probe.py` | Python script for experiments and finding key codes |

The platform is selected automatically at build time (`#[cfg(target_os)]`);
there is nothing to enable.

## Hardware

| | |
|---|---|
| Laptop | MSI Titan 18 HX A14VIG (board MS-1822, BIOS E1822IMS.117) |
| Lighting controller | SteelSeries KLC, USB `1038:1122`, interface 0 |
| Tested on | Linux kernel 7.3 (Ubuntu) |

Check that the controller is present:

```bash
lsusb | grep 1038:1122
```

## Installing on Linux (also after reinstalling the OS)

1. Install Rust if you don't have it yet:

   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. Clone the repository and run the installer **as your normal user**
   (it asks for the sudo password itself):

   ```bash
   git clone git@github.com:amsokol/msi-titan-18hx-kbd-backlight.git
   cd msi-titan-18hx-kbd-backlight
   ./install.sh
   ```

The backlight turns on right away. What gets installed:

| File | Purpose |
|---|---|
| `/usr/local/bin/titan-kbd-light` | the program |
| `/etc/systemd/system/titan-kbd-light.service` | service: runs at boot and after resume |
| `/etc/udev/rules.d/99-titan-kbd-light.rules` | starts the service when the keyboard controller appears |
| `/etc/default/titan-kbd-light` | color setting |

After installation the repository is no longer needed: the installed files
do not depend on it.

## Changing the color

Set `COLOR` in `/etc/default/titan-kbd-light` (`RRGGBB` or `off`) and apply:

```bash
sudo systemctl start titan-kbd-light
```

One-off, without changing the setting:

```bash
sudo titan-kbd-light 00ff00
```

## Checking and troubleshooting

```bash
systemctl status titan-kbd-light
```

```bash
journalctl -b -u titan-kbd-light
```

If the program prints `SteelSeries KLC (1038:1122, interface 0) not found`,
the controller is missing or has a different USB ID (see `lsusb`). Before
giving up, the program waits 10 seconds for it to appear, e.g. after resume.

## Uninstalling

```bash
./uninstall.sh
```

## Windows (untested)

The Windows code is written but has not been run on a real laptop yet.
The first install is effectively a test.

1. Uninstall or disable SteelSeries GG and the lighting control in
   MSI Center, otherwise they will override the color.
2. Install Rust: <https://rustup.rs>. The MSVC toolchain is needed (rustup
   offers to install Visual Studio Build Tools).
3. In an **elevated** PowerShell, from the repository folder:

   ```powershell
   powershell -ExecutionPolicy Bypass -File .\install.ps1
   ```

   Another color: `.\install.ps1 -Color 00ff00`.

The script builds `titan-kbd-light.exe`, copies it to
`C:\Program Files\titan-kbd-light\` and creates a `titan-kbd-light` task in
Task Scheduler. The task runs as SYSTEM at boot, at any user logon (the boot
trigger does not fire when Fast Startup is enabled) and after resume.

Manual test without installing:

```powershell
cargo build --release
.\target\release\titan-kbd-light.exe ff0000
```

Uninstall: `powershell -ExecutionPolicy Bypass -File .\uninstall.ps1`.

If it does not work:
- `not found`: Windows did not find the `VID_1038&PID_1122&MI_00` interface.
  The device path is shown in Device Manager: "Human Interface Devices",
  device Properties, "Details", "Device instance path".
- `Access is denied`: another program (GG, MSI Center) holds the device.
- The backlight turns on but some keys stay blue: increase `WAKE_SETTLE` in
  `src/main.rs`.

## How it works

This was worked out by experimenting on this laptop. The controller speaks
two protocols, and the Titan needs both: the first turns the backlight on,
the second sets the colors. All commands go to interface 0 as 524-byte HID
feature reports with report ID 0 (a USB SET_REPORT with `wValue 0x0300`).

**1. Wake (old protocol, as in msi-perkeyrgb).** One feature report for each
of the four key groups:

```
0e 00 GROUP 00 | 42 × (R G B 00 00 00 00 00 00 01 00 KEY) | 00×14 08 39
```

Each is followed by two 64-byte output reports: `0d 00 02 00…` and `09 00…`.
Groups: `2a` letters/digits, `0b` Enter, `18` modifiers, `24` numpad.
On the Titan this **turns the backlight on with a built-in blue theme, and
the colors sent are ignored**. The theme is applied with a delay, so the
program waits a second before the color step and sends the colors twice.

Careful: the firmware treats the third header byte as a group number.
Sending a packet with a different value there (e.g. 40 or 22) resets some
keys (Esc, Tab, Caps, left Shift and Ctrl) to the theme color.

**2. Color (protocol from the Linux kernel patch for MSI Raider A18 HX).**
One feature report:

```
0c 00 66 00 | (KEY R G B) × N | unused entries: KEY = ff
```

`KEY` is the HID usage ID of a key: `04` = A, `e0` = left Ctrl, etc.
**The Titan reads only about the first 100 entries**, so the modifiers
(`e0`–`e7`, `f0`) go first. If they are at the end, Windows, AltGr, right
Ctrl and right Shift stay blue.

## Tools

`tools/klc-probe.py` is the original Python script all of this was worked
out with. It has key code sweep modes (`sweep` for the old protocol and
`newsweep` for the new one) that light keys green one at a time. Useful if
some keys stay uncolored on another model.

```bash
sudo python3 tools/klc-probe.py ff0000 both
```

## Credits

- Linux kernel patch `HID: steelseries: Add MSI Raider A18 HX A9WJG RGB
  support`: the `0c 00 66` protocol.
- [msi-perkeyrgb](https://github.com/Askannz/msi-perkeyrgb): the `0e`
  protocol.
- [msi-lightbar-linux](https://github.com/fenyxsomnia/msi-lightbar-linux):
  the `0d 00 02` / `09` commands and the group-number trap.

## License

[MIT](LICENSE)
