//! Packets for the SteelSeries KLC controller (USB 1038:1122, interface 0).
//! Platform independent: the platform modules only deliver the bytes.
//!
//! 1. "Wake": the older msi-perkeyrgb protocol. Per-region feature reports
//!    followed by `0d 00 02` / `09` output reports. On the Titan this only
//!    switches the backlight on (with a firmware theme), its colors are
//!    ignored.
//! 2. "Color": the protocol from the Linux hid-steelseries patch for the
//!    MSI Raider A18 HX. One feature report `0c 00 66 00` followed by
//!    (key, R, G, B) entries. The Titan only reads about the first 100
//!    entries, so modifiers go first.

/// Feature report payload length, without the leading report ID byte.
pub const REPORT_LEN: usize = 524;

/// Output reports sent after each wake report, without the report ID byte.
pub const WAKE_COMMIT: [&[u8]; 2] = [&[0x0d, 0x00, 0x02], &[0x09]];

/// Old protocol regions: region id -> HID usage IDs of its keys.
fn regions() -> [(u8, Vec<u8>); 4] {
    [
        // alphanum
        (0x2a, (0x04..0x28).chain(0x3a..0x40).collect()),
        // enter
        (0x0b, vec![0x28, 0x31, 0x32, 0x64, 0x87, 0x88, 0x89, 0x8a, 0x8b, 0x90, 0x91]),
        // modifiers
        (
            0x18,
            (0x29..0x31)
                .chain(0x33..0x3a)
                .chain([0x65])
                .chain(0xe0..0xe7)
                .chain([0xf0])
                .collect(),
        ),
        // numpad
        (0x24, (0x40..0x64).collect()),
    ]
}

/// New protocol key list. Modifiers first: the Titan ignores entries past
/// ~100, and the numpad codes at the end do not exist on this keyboard.
fn color_keys() -> Vec<u8> {
    (0xe0..0xe8).chain([0xf0]).chain(0x04..0x67).collect()
}

pub fn wake_reports(rgb: [u8; 3]) -> Vec<Vec<u8>> {
    regions()
        .into_iter()
        .map(|(region, keys)| {
            let mut buf = vec![0x0e, 0x00, region, 0x00];
            for &key in &keys {
                buf.extend_from_slice(&rgb);
                buf.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0x01, 0x00, key]);
            }
            buf.resize(4 + 42 * 12, 0);
            buf.extend_from_slice(&[0; 14]);
            buf.extend_from_slice(&[0x08, 0x39]);
            debug_assert_eq!(buf.len(), REPORT_LEN);
            buf
        })
        .collect()
}

pub fn color_report(rgb: [u8; 3]) -> Vec<u8> {
    let mut buf = vec![0u8; REPORT_LEN];
    buf[0] = 0x0c;
    buf[2] = 0x66;
    let slots = (REPORT_LEN - 4) / 4;
    for i in 0..slots {
        buf[4 + i * 4] = 0xff; // unused slot marker
    }
    for (i, key) in color_keys().into_iter().take(slots).enumerate() {
        buf[4 + i * 4..8 + i * 4].copy_from_slice(&[key, rgb[0], rgb[1], rgb[2]]);
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_sizes() {
        for r in wake_reports([1, 2, 3]) {
            assert_eq!(r.len(), REPORT_LEN);
        }
        assert_eq!(color_report([1, 2, 3]).len(), REPORT_LEN);
    }

    #[test]
    fn region_sizes_fit() {
        for (_, keys) in regions() {
            assert!(keys.len() <= 42);
        }
    }

    #[test]
    fn color_report_layout() {
        let r = color_report([0xff, 0, 0]);
        assert_eq!(&r[..12], &[0x0c, 0, 0x66, 0, 0xe0, 0xff, 0, 0, 0xe1, 0xff, 0, 0]);
        let n = color_keys().len();
        assert_eq!(r[4 + n * 4], 0xff); // first unused slot
    }
}
