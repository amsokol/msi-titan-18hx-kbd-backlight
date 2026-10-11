#!/usr/bin/env python3
"""Turn on the keyboard backlight of an MSI laptop with a SteelSeries KLC
controller (USB 1038:1122), e.g. MSI Titan 18 HX A14VIG.

Protocol taken from the Linux hid-steelseries patch for MSI Raider A18 HX:
one 524-byte feature report (report ID 0) on USB interface 0:
  byte 0: 0x0c, byte 2: 0x66 (keyboard), then 4-byte entries (key, R, G, B).

A second, older KLC protocol (msi-perkeyrgb, MSI GE/GS/GT/Raider models) is
also supported: per-region 524-byte feature reports
  0e 00 REGION 00, 42 x (R G B 00*6 01 00 KEY), 00*14 08 39
followed by 64-byte output reports "0d 00 02" and "09" that apply the colors.

Usage (needs root, /dev/hidraw* is root-only):
  sudo python3 tools/klc-probe.py              # white, both protocols
  sudo python3 tools/klc-probe.py ff8000       # any color as RRGGBB
  sudo python3 tools/klc-probe.py 000000       # off
  sudo python3 tools/klc-probe.py ffffff old   # only the older protocol
  sudo python3 tools/klc-probe.py ffffff new   # only the Raider A18 protocol
  sudo python3 tools/klc-probe.py ff0000 sweep [first] [last] [delay]
      # old protocol, then light every unknown code green one by one
      # (hex range, default 01..ff, 1 s each) to find a key's code
  sudo python3 tools/klc-probe.py ff0000 newsweep [first] [last] [delay]
      # same with the Raider A18 protocol: all keys r/g/b + one code green
"""
import fcntl
import glob
import os
import sys
import time

VID, PID = 0x1038, 0x1122
REPORT_LEN = 524

# HID keyboard usage IDs, from the kernel patch, plus a few extra
# (0x32, 0x48, 0x4a, 0x4d, 0x65, 0xe7) in case the Titan layout uses them.
# The Titan seems to ignore entries past ~100, so modifiers go first and the
# numpad codes, which this keyboard does not have, last.
KEYS = [*range(0xe0, 0xe8), 0xf0, *range(0x04, 0x67)]


# Older protocol: region id -> HID usage IDs of the keys in that region.
REGIONS = {
    0x2a: [*range(0x04, 0x28), *range(0x3a, 0x40)],  # alphanum
    0x0b: [0x28, 0x31, 0x32, 0x64, 0x87, 0x88, 0x89, 0x8a, 0x8b, 0x90, 0x91],
    0x18: [*range(0x29, 0x31), *range(0x33, 0x3a), 0x65,
           *range(0xe0, 0xe7), 0xf0],  # modifiers
    0x24: list(range(0x40, 0x64)),  # numpad
}


def hidiocsfeature(length):
    # _IOC(_IOC_WRITE | _IOC_READ, 'H', 0x06, length)
    return (3 << 30) | (length << 16) | (ord("H") << 8) | 0x06


def find_hidraw():
    for node in sorted(glob.glob("/sys/class/hidraw/hidraw*")):
        dev = os.path.join(node, "device")
        try:
            with open(os.path.join(dev, "uevent")) as f:
                uevent = f.read()
            with open(os.path.join(dev, "..", "bInterfaceNumber")) as f:
                iface = int(f.read().strip(), 16)
        except OSError:
            continue
        if f"HID_ID=0003:{VID:08X}:{PID:08X}" in uevent and iface == 0:
            return "/dev/" + os.path.basename(node)
    return None


def build_report(r, g, b, extra=()):
    """extra: (key, r, g, b) entries placed before KEYS."""
    buf = bytearray(REPORT_LEN)
    buf[0] = 0x0c
    buf[2] = 0x66
    slots = (REPORT_LEN - 4) // 4
    for i in range(slots):
        buf[4 + i * 4] = 0xff  # unused slot marker
    entries = list(extra) + [(key, r, g, b) for key in KEYS]
    for i, entry in enumerate(entries[:slots]):
        buf[4 + i * 4:8 + i * 4] = bytes(entry)
    return buf


def new_sweep(fd, r, g, b, first, last, delay):
    """Whole keyboard r/g/b plus one unknown code green, code by code."""
    for key in range(first, last + 1):
        if key in KEYS or key == 0xff:
            continue
        print(f"code 0x{key:02x}", flush=True)
        send_feature(fd, build_report(r, g, b, [(key, 0, 255, 0)]))
        time.sleep(delay)
    send_feature(fd, build_report(r, g, b))


def build_old_reports(r, g, b):
    reports = []
    for region, keys in REGIONS.items():
        buf = bytearray([0x0e, 0x00, region, 0x00])
        for key in keys:
            buf += bytes((r, g, b, 0, 0, 0, 0, 0, 0, 0x01, 0x00, key))
        buf += bytes(12 * (42 - len(keys)))
        buf += bytes(14) + bytes((0x08, 0x39))
        assert len(buf) == REPORT_LEN
        reports.append(buf)

    return reports


def make_single(key, r, g, b):
    # One-entry packet. Entry count 1 is not a known region id.
    buf = bytearray([0x0e, 0x00, 0x01, 0x00])
    buf += bytes((r, g, b, 0, 0, 0, 0, 0, 0, 0x01, 0x00, key))
    buf += bytes(12 * 41) + bytes(14) + bytes((0x08, 0x39))
    return buf


def commit(fd):
    send_output(fd, bytes((0x0d, 0x00, 0x02)))
    send_output(fd, bytes((0x09,)))


def sweep(fd, r, g, b, first, last, delay):
    """Light each code not covered by REGIONS green, then leave it r/g/b."""
    known = {k for keys in REGIONS.values() for k in keys}
    for key in range(first, last + 1):
        if key in known:
            continue
        print(f"code 0x{key:02x}", flush=True)
        send_feature(fd, make_single(key, 0, 255, 0))
        commit(fd)
        time.sleep(delay)
        send_feature(fd, make_single(key, r, g, b))
        commit(fd)


def send_feature(fd, payload):
    # Leading 0x00 is the report ID; usbhid strips it and sends the payload
    # bytes with wValue 0x0300, same as the kernel patch.
    data = bytes([0x00]) + payload
    fcntl.ioctl(fd, hidiocsfeature(len(data)), data)


def send_output(fd, payload):
    os.write(fd, bytes([0x00]) + payload.ljust(64, b"\0"))


def main():
    color = sys.argv[1] if len(sys.argv) > 1 else "ffffff"
    try:
        r, g, b = bytes.fromhex(color.lstrip("#"))
    except ValueError:
        sys.exit(f"Bad color {color!r}, expected RRGGBB, e.g. ffffff")

    path = find_hidraw()
    if not path:
        sys.exit("SteelSeries KLC (1038:1122, interface 0) not found")

    proto = sys.argv[2] if len(sys.argv) > 2 else "both"
    if proto not in ("both", "old", "new", "sweep", "newsweep"):
        sys.exit(f"Bad mode {proto!r}, expected old, new, both, sweep "
                 "or newsweep")

    fd = os.open(path, os.O_RDWR)
    try:
        if proto in ("both", "old"):
            # Commit after every region, as msi-lightbar-linux does.
            for report in build_old_reports(r, g, b):
                send_feature(fd, report)
                commit(fd)
            print(f"{path}: old protocol sent")
        if proto == "sweep":
            first = int(sys.argv[3], 16) if len(sys.argv) > 3 else 0x01
            last = int(sys.argv[4], 16) if len(sys.argv) > 4 else 0xff
            delay = float(sys.argv[5]) if len(sys.argv) > 5 else 1.0
            for report in build_old_reports(r, g, b):
                send_feature(fd, report)
                commit(fd)
            sweep(fd, r, g, b, first, last, delay)
        if proto == "newsweep":
            first = int(sys.argv[3], 16) if len(sys.argv) > 3 else 0x00
            last = int(sys.argv[4], 16) if len(sys.argv) > 4 else 0xfe
            delay = float(sys.argv[5]) if len(sys.argv) > 5 else 1.0
            new_sweep(fd, r, g, b, first, last, delay)
        if proto in ("both", "new"):
            send_feature(fd, build_report(r, g, b))
            print(f"{path}: new protocol sent")
    finally:
        os.close(fd)
    print(f"keyboard color set to #{r:02x}{g:02x}{b:02x}")


if __name__ == "__main__":
    main()
