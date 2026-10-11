//! Turn on the keyboard backlight of an MSI Titan 18 HX A14VIG.
//!
//! The keyboard lighting is driven by a SteelSeries KLC controller
//! (USB 1038:1122, interface 0). See protocol.rs and README.md for how it
//! works. Linux is tested; Windows is written but not tested yet.

mod protocol;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as platform;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as platform;

#[cfg(not(any(target_os = "linux", windows)))]
compile_error!("only Linux and Windows are supported");

use std::io;
use std::process::ExitCode;
use std::thread::sleep;
use std::time::Duration;

use platform::Device;

const DEFAULT_COLOR: [u8; 3] = [0xff, 0x00, 0x00];

/// Attempts to find and program the device, e.g. right after resume when the
/// USB device may not be back yet.
const ATTEMPTS: u32 = 20;
const RETRY_DELAY: Duration = Duration::from_millis(500);

/// The firmware theme switched on by the wake step is applied with a delay
/// and overwrites colors sent right after it, so wait before coloring and
/// send the colors once more after that.
const WAKE_SETTLE: Duration = Duration::from_secs(1);

fn apply(rgb: [u8; 3]) -> io::Result<String> {
    let dev = Device::open()?;
    for report in protocol::wake_reports(rgb) {
        dev.send_feature(&report)?;
        for commit in protocol::WAKE_COMMIT {
            dev.send_output(commit)?;
        }
    }
    let color = protocol::color_report(rgb);
    for _ in 0..2 {
        sleep(WAKE_SETTLE);
        dev.send_feature(&color)?;
    }
    Ok(dev.name())
}

fn parse_color(arg: &str) -> Option<[u8; 3]> {
    if arg == "off" {
        return Some([0, 0, 0]);
    }
    let hex = arg.strip_prefix('#').unwrap_or(arg);
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

fn usage() {
    println!(
        "Usage: titan-kbd-light [RRGGBB | off]\n\
         \n\
         Turns on the keyboard backlight of MSI Titan 18 HX and sets one color\n\
         for all keys (default ff0000). Needs root on Linux.\n\
         \n\
         Examples:\n  \
           titan-kbd-light           # red\n  \
           titan-kbd-light ffffff    # white\n  \
           titan-kbd-light off       # all keys black"
    );
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rgb = match args.as_slice() {
        [] => DEFAULT_COLOR,
        [a] if a == "-h" || a == "--help" => {
            usage();
            return ExitCode::SUCCESS;
        }
        [a] if a == "-V" || a == "--version" => {
            println!("titan-kbd-light {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        [a] => match parse_color(a) {
            Some(rgb) => rgb,
            None => {
                eprintln!("Bad color {a:?}, expected RRGGBB (e.g. ff0000) or off");
                return ExitCode::from(2);
            }
        },
        _ => {
            usage();
            return ExitCode::from(2);
        }
    };

    let mut last_err = None;
    for attempt in 1..=ATTEMPTS {
        match apply(rgb) {
            Ok(name) => {
                println!(
                    "{name}: keyboard color set to #{:02x}{:02x}{:02x}",
                    rgb[0], rgb[1], rgb[2]
                );
                return ExitCode::SUCCESS;
            }
            // Permission errors will not go away by retrying.
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                eprintln!("{e} ({})", platform::PERMISSION_HINT);
                return ExitCode::FAILURE;
            }
            Err(e) => {
                last_err = Some(e);
                if attempt < ATTEMPTS {
                    sleep(RETRY_DELAY);
                }
            }
        }
    }
    eprintln!("{}", last_err.expect("at least one attempt"));
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        assert_eq!(parse_color("ff8000"), Some([0xff, 0x80, 0x00]));
        assert_eq!(parse_color("#00FF00"), Some([0, 0xff, 0]));
        assert_eq!(parse_color("off"), Some([0, 0, 0]));
        assert_eq!(parse_color("fff"), None);
        assert_eq!(parse_color("zzzzzz"), None);
    }
}
