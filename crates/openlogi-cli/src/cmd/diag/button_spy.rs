//! Read-only HID++ 0x8110 (Mouse Button Spy) live-event diagnostic.
//!
//! The G903 HERO has no 0x1bxx ReprogControls, but advertises 0x8110. Reports
//! seen on a G502 HERO suggest a bitmap of *physical* button presses. The
//! report layout and spontaneous delivery have NOT been verified on a G903.
//! The command only enumerates features, opens a channel, and observes input:
//! no HID++ feature write, no profile toggle, and no button-filter change.

use anyhow::{Context as _, Result, anyhow};
use clap::Args;
use hidpp::channel::HidppMessage;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant, sleep_until};

use super::select_device;

const MOUSE_BUTTON_SPY_ID: u16 = 0x8110;

#[derive(Debug, Args)]
pub struct ButtonSpyArgs {
    /// Match an online device by name (e.g. G903).
    #[arg(long, value_name = "NAME")]
    pub device: Option<String>,

    /// Observation duration, 5 to 120 seconds. No data is saved.
    #[arg(long, default_value_t = 45, value_parser = clap::value_parser!(u64).range(5..=120))]
    pub seconds: u64,
}

/// Decode only unsolicited 0x8110 long HID++ reports addressed to the target.
///
/// Long payload bytes: [device slot, feature index, function|software ID,
/// bitmap high, bitmap low, ..]. We require function=0 and software ID=0.
/// Other-device reports and replies are ignored (do not print raw HID data).
fn decode_spy_bitmap(
    message: HidppMessage,
    matched_request: bool,
    device_index: u8,
    feature_index: u8,
) -> Option<u16> {
    if matched_request {
        return None;
    }
    match message {
        HidppMessage::Long(bytes)
            if bytes[0] == device_index && bytes[1] == feature_index && bytes[2] == 0 =>
        {
            Some(u16::from_be_bytes([bytes[3], bytes[4]]))
        }
        _ => None,
    }
}

fn bitmap_changes(previous: u16, current: u16) -> (u16, u16) {
    (current & !previous, previous & !current)
}

pub async fn run(args: ButtonSpyArgs) -> Result<()> {
    let (route, name) = select_device(args.device.as_deref(), &[MOUSE_BUTTON_SPY_ID]).await?;
    let features = openlogi_hid::dump_features(&route)
        .await
        .context("read the device feature table")?;
    let index = features
        .iter()
        .position(|feature| feature.id == MOUSE_BUTTON_SPY_ID)
        .ok_or_else(|| anyhow!("device does not expose HID++ 0x8110 (Mouse Button Spy)"))?;
    let index: u8 = index
        .try_into()
        .context("HID++ feature index exceeds one byte")?;
    let channel = openlogi_hid::channel_pool()
        .open(&route)
        .await
        .context("open the device's read-only HID++ channel")?
        .ok_or_else(|| anyhow!("selected device could not be opened"))?;

    // Never block the HID report-reader thread with console I/O.
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let device_index = route.device_index();
    let _listener = channel.add_msg_listener_guarded(move |message, matched| {
        if let Some(bitmap) = decode_spy_bitmap(message, matched, device_index, index) {
            let _ = sender.send(bitmap);
        }
    });

    println!("device: {name}");
    println!(
        "0x8110 feature index: {index}; passive observation for {} seconds",
        args.seconds
    );
    println!("Press and release each of the FOUR side buttons separately.");
    println!("Note which button bit (if any) appears for each physical press.");
    println!("Read-only: no receiver, onboard-profile, filter, or binding changes.");
    let deadline = Instant::now() + Duration::from_secs(args.seconds);
    let mut previous = 0u16;
    let mut reports_seen = 0u64;
    loop {
        tokio::select! {
            () = sleep_until(deadline) => break,
            message = receiver.recv() => {
                let Some(current) = message else { break };
                reports_seen += 1;
                let (pressed, released) = bitmap_changes(previous, current);
                for bit in 0..16 {
                    let flag = 1u16 << bit;
                    if pressed & flag != 0 {
                        println!("DOWN bit {bit:02}  (bitmap=0x{current:04x})");
                    }
                    if released & flag != 0 {
                        println!("UP   bit {bit:02}  (bitmap=0x{current:04x})");
                    }
                }
                previous = current;
            }
        }
    }

    if reports_seen == 0 {
        println!("No 0x8110 notifications were observed; this does NOT prove that");
        println!("the physical buttons are indistinguishable on this device.");
    } else {
        println!("Observed {reports_seen} 0x8110 reports. The bits are not yet mapped to G903 names.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spy_report(slot: u8, index: u8, header: u8, bitmap: u16) -> HidppMessage {
        let mut bytes = [0u8; 19];
        bytes[0] = slot;
        bytes[1] = index;
        bytes[2] = header;
        [bytes[3], bytes[4]] = bitmap.to_be_bytes();
        HidppMessage::Long(bytes)
    }

    #[test]
    fn decodes_g502_style_button_bitmap_from_the_selected_device() {
        let report = spy_report(1, 9, 0, 0x0201);
        assert_eq!(decode_spy_bitmap(report, false, 1, 9), Some(0x0201));
        assert_eq!(bitmap_changes(0, 0x0201), (0x0201, 0));
        assert_eq!(bitmap_changes(0x0201, 0x0200), (0, 1));
        assert_eq!(bitmap_changes(0x0001, 0x0002), (0x0002, 0x0001));
    }

    #[test]
    fn ignores_other_devices_features_requests_and_report_types() {
        let valid = spy_report(1, 9, 0, 0x0020);
        assert_eq!(decode_spy_bitmap(valid, true, 1, 9), None);
        assert_eq!(decode_spy_bitmap(valid, false, 7, 9), None);
        assert_eq!(decode_spy_bitmap(valid, false, 1, 8), None);
        assert_eq!(
            decode_spy_bitmap(spy_report(1, 9, 1, 0x0020), false, 1, 9),
            None
        );
        assert_eq!(
            decode_spy_bitmap(HidppMessage::Short([0; 6]), false, 1, 9),
            None
        );
    }
}
