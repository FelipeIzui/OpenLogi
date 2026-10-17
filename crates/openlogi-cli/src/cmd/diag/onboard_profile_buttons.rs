//! Read-only 0x8100 G903 HERO profile button-table diagnostics.
//!
//! Sends GetProfilesDescription (function 0) and MemoryRead (function 5)
//! only. No mode/profile switching, settings changes, flash writes, full
//! profile dumps or macro payload decoding. Entire sectors are held in
//! memory only long enough to validate their CRC and extract 12 bindings.

use anyhow::{Context as _, Result, anyhow, ensure};
use clap::Args;
use hidpp::{
    channel::HidppChannel,
    nibble::U4,
    protocol::v20::{Message, MessageHeader},
};

use super::{
    onboard_profile_info::{ONBOARD_PROFILES_FEATURE, ProfileDescription, read_description},
    select_device,
};

const G903_SECTOR_SIZE: usize = 255;
const BUTTON_OFFSET: usize = 32;
const ALTERNATE_BUTTON_OFFSET: usize = 96;
const BUTTON_BINDING_SIZE: usize = 4;
const G903_BUTTON_COUNT: usize = 12;

#[derive(Debug, Args)]
pub struct ProfileButtonsArgs {
    /// Match one online mouse (for example, G903).
    #[arg(long, value_name = "NAME")]
    pub device: Option<String>,
}

fn ensure_g903_format(description: ProfileDescription) -> Result<()> {
    ensure!(
        description.memory_model == 1
            && description.profile_format == 4
            && description.macro_format == 1
            && description.sector_size == 255
            && description.button_count == 12
            && description.profile_count > 0
            && description.profile_count <= 5
            && description.sector_count >= description.profile_count.saturating_add(1),
        "unsupported profile descriptor; refusing to interpret profile memory"
    );
    Ok(())
}

/// CRC-16/CCITT-FALSE (poly 0x1021, initial 0xFFFF), as used by Logitech
/// profile sectors. The final two bytes of a sector are big-endian CRC.
fn crc_ccitt(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xffff;
    for byte in data {
        crc ^= u16::from(*byte) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}

fn check_sector_crc(sector: &[u8]) -> Result<()> {
    ensure!(sector.len() == G903_SECTOR_SIZE, "unexpected profile sector length");
    let payload_len = sector.len() - 2;
    let stored = u16::from_be_bytes([sector[payload_len], sector[payload_len + 1]]);
    let computed = crc_ccitt(&sector[..payload_len]);
    ensure!(stored == computed, "profile sector CRC mismatch; no bindings decoded");
    Ok(())
}

/// Each onboard memory read returns 16 bytes. Offset 239 is the last safe
/// 16-byte window of a 255-byte sector; reading from 240 is an invalid
/// argument on Logitech firmware.
fn sector_read_offsets() -> impl Iterator<Item = usize> {
    (0..G903_SECTOR_SIZE)
        .step_by(16)
        .map(|start| start.min(G903_SECTOR_SIZE - 16))
}

async fn read_sector(
    channel: &HidppChannel,
    device_index: u8,
    feature_index: u8,
    sector_index: u16,
) -> Result<[u8; G903_SECTOR_SIZE]> {
    let mut sector = [0u8; G903_SECTOR_SIZE];
    for offset in sector_read_offsets() {
        let mut request = [0u8; 16];
        request[..2].copy_from_slice(&sector_index.to_be_bytes());
        request[2..4].copy_from_slice(&u16::try_from(offset)?.to_be_bytes());
        let reply = channel
            .send_v20(Message::Long(
                MessageHeader {
                    device_index,
                    feature_index,
                    function_id: U4::from_lo(5),
                    software_id: channel.get_sw_id(),
                },
                request,
            ))
            .await
            .with_context(|| format!("read-only sector {sector_index} offset {offset}"))?;
        let Message::Long(_, payload) = reply else {
            return Err(anyhow!("unexpected short profile-memory response"));
        };
        sector[offset..offset + 16].copy_from_slice(&payload);
    }
    check_sector_crc(&sector)?;
    Ok(sector)
}

/// Identify the first *enabled* user profile in a CRC-validated dictionary.
/// A disabled or absent profile is not a safe target for interpretation.
fn first_enabled_slot(sector: &[u8], profile_count: u8, sector_count: u8) -> Result<u16> {
    check_sector_crc(sector)?;
    for slot in 0..usize::from(profile_count) {
        let base = slot * 4;
        let address = u16::from_be_bytes([sector[base], sector[base + 1]]);
        if address == 0xffff {
            break;
        }
        if sector[base + 2] != 0 {
            ensure!(
                address > 0 && address < u16::from(sector_count),
                "enabled profile points outside device memory"
            );
            return Ok(address);
        }
    }
    Err(anyhow!("no enabled profile in the onboard dictionary"))
}

fn describe_binding(bytes: &[u8]) -> String {
    match bytes {
        [0x80, 0x01, hi, lo] => {
            let mask = u16::from_be_bytes([*hi, *lo]);
            if mask.count_ones() == 1 {
                format!("mouse button {}", mask.trailing_zeros() + 1)
            } else {
                format!("mouse mask 0x{mask:04x}")
            }
        }
        [0x80, 0x02, modifiers, key] => {
            format!("keyboard usage 0x{key:02x}, modifiers 0x{modifiers:02x}")
        }
        [0x80, 0x03, hi, lo] => {
            format!("consumer usage 0x{:04x}", u16::from_be_bytes([*hi, *lo]))
        }
        [0x80, 0x00, ..] => "no action".to_owned(),
        [0x90, code, ..] => format!("special action 0x{code:02x}"),
        [0xff, ..] => "disabled".to_owned(),
        [0x00, ..] => "macro (contents omitted)".to_owned(),
        [kind, ..] => format!("other action type 0x{kind:02x}"),
        [] => "invalid binding".to_owned(),
    }
}

fn button_bindings(sector: &[u8], index: usize) -> Result<(String, String)> {
    check_sector_crc(sector)?;
    ensure!(index < G903_BUTTON_COUNT, "button index out of range");
    let primary = BUTTON_OFFSET + index * BUTTON_BINDING_SIZE;
    let shifted = ALTERNATE_BUTTON_OFFSET + index * BUTTON_BINDING_SIZE;
    Ok((
        describe_binding(&sector[primary..primary + BUTTON_BINDING_SIZE]),
        describe_binding(&sector[shifted..shifted + BUTTON_BINDING_SIZE]),
    ))
}

pub async fn run(args: ProfileButtonsArgs) -> Result<()> {
    let (route, name) = select_device(args.device.as_deref(), &[ONBOARD_PROFILES_FEATURE]).await?;
    ensure!(
        name.to_ascii_lowercase().contains("g903"),
        "this format-specific diagnostic is limited to the G903 family"
    );
    let features = openlogi_hid::dump_features(&route)
        .await
        .context("enumerate supported HID++ features")?;
    let feature_index = features
        .iter()
        .position(|entry| entry.id == ONBOARD_PROFILES_FEATURE)
        .ok_or_else(|| anyhow!("device does not expose onboard profiles"))?;
    let feature_index: u8 = feature_index
        .try_into()
        .context("profile feature index exceeds one byte")?;
    let channel = openlogi_hid::channel_pool()
        .open(&route)
        .await
        .context("open device HID++ channel")?
        .ok_or_else(|| anyhow!("device HID++ channel unavailable"))?;

    let description = read_description(&channel, route.device_index(), feature_index).await?;
    ensure_g903_format(description)?;
    let directory = read_sector(&channel, route.device_index(), feature_index, 0).await?;
    let address = first_enabled_slot(
        &directory,
        description.profile_count,
        description.sector_count,
    )?;
    let profile = read_sector(&channel, route.device_index(), feature_index, address).await?;

    println!("device: {name}");
    println!("format 4, firmware entries: {}", description.button_count);
    println!("first enabled onboard profile sector: 0x{address:04x}");
    println!("firmware control | primary mapping | alternate mapping");
    for button in 0..G903_BUTTON_COUNT {
        let (primary, alternate) = button_bindings(&profile, button)?;
        println!("{:>16} | {primary} | {alternate}", button + 1);
    }
    println!("CRC: directory and selected profile valid.");
    println!("Only read requests were sent. No profile or configuration writes.");
    println!("Macro payloads and profile names are not displayed or saved.");
    println!("Physical control labels still require device-specific verification.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_sector() -> [u8; G903_SECTOR_SIZE] {
        let mut bytes = [0xff; G903_SECTOR_SIZE];
        let crc = crc_ccitt(&bytes[..G903_SECTOR_SIZE - 2]);
        bytes[G903_SECTOR_SIZE - 2..].copy_from_slice(&crc.to_be_bytes());
        bytes
    }

    fn with_updated_crc(bytes: &mut [u8; G903_SECTOR_SIZE]) {
        let crc = crc_ccitt(&bytes[..G903_SECTOR_SIZE - 2]);
        bytes[G903_SECTOR_SIZE - 2..].copy_from_slice(&crc.to_be_bytes());
    }

    #[test]
    fn last_profile_read_window_stays_inside_the_255_byte_sector() {
        let offsets: Vec<_> = sector_read_offsets().collect();
        assert_eq!(offsets.len(), 16);
        assert_eq!(offsets[0], 0);
        assert_eq!(offsets[14], 224);
        assert_eq!(offsets[15], 239);
        assert!(offsets.iter().all(|offset| offset + 16 <= G903_SECTOR_SIZE));
    }

    #[test]
    fn crc_matches_ccitt_false_known_vector() {
        assert_eq!(crc_ccitt(b"123456789"), 0x29b1);
        let mut sector = valid_sector();
        check_sector_crc(&sector).expect("generated sector has valid CRC");
        sector[5] ^= 1;
        check_sector_crc(&sector).expect_err("corrupt sector must be rejected");
    }

    #[test]
    fn dictionary_requires_an_enabled_in_bounds_profile() {
        let mut sector = valid_sector();
        sector[0..4].copy_from_slice(&[0x00, 0x01, 0x01, 0x00]);
        with_updated_crc(&mut sector);
        assert_eq!(first_enabled_slot(&sector, 5, 16).unwrap(), 1);
        sector[1] = 0x15;
        with_updated_crc(&mut sector);
        first_enabled_slot(&sector, 5, 16).expect_err("invalid sector reference rejected");
    }

    #[test]
    fn identifies_multiple_firmware_buttons_with_same_logical_mapping() {
        let mut sector = valid_sector();
        sector[32..36].copy_from_slice(&[0x80, 0x01, 0x00, 0x08]);
        sector[40..44].copy_from_slice(&[0x80, 0x01, 0x00, 0x08]);
        sector[96..100].copy_from_slice(&[0x00, 0x01, 0x03, 0x00]);
        with_updated_crc(&mut sector);
        assert_eq!(button_bindings(&sector, 0).unwrap().0, "mouse button 4");
        assert_eq!(button_bindings(&sector, 2).unwrap().0, "mouse button 4");
        assert_eq!(
            button_bindings(&sector, 0).unwrap().1,
            "macro (contents omitted)"
        );
    }

    #[test]
    fn rejects_unsupported_descriptors() {
        let sample = ProfileDescription {
            memory_model: 1,
            profile_format: 4,
            macro_format: 1,
            profile_count: 5,
            out_of_box_profiles: 1,
            button_count: 12,
            sector_count: 16,
            sector_size: 255,
            mechanical_layout: 0x0a,
            various_info: 4,
        };
        ensure_g903_format(sample).expect("known G903 descriptor");
        ensure_g903_format(ProfileDescription { profile_format: 3, ..sample })
            .expect_err("unknown layout forbidden");
    }
}
