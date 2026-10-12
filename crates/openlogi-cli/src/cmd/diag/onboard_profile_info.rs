//! HID++ 0x8100 GetProfilesDescription diagnostic (read-only request).
//!
//! The G903 HERO has an onboard profile feature even without 0x1B04.
//! Do not attempt to parse or write the device's profile sectors until the
//! actual format advertised by its firmware has been validated.

use anyhow::{Context as _, Result, anyhow};
use clap::Args;
use hidpp::{
    nibble::U4,
    protocol::v20::{Message, MessageHeader},
};

use super::select_device;

const ONBOARD_PROFILES_FEATURE: u16 = 0x8100;

#[derive(Debug, Args)]
pub struct ProfileInfoArgs {
    /// Online device name filter (for example, G903).
    #[arg(long, value_name = "NAME")]
    pub device: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProfileDescription {
    memory_model: u8,
    profile_format: u8,
    macro_format: u8,
    profile_count: u8,
    out_of_box_profiles: u8,
    button_count: u8,
    sector_count: u8,
    sector_size: u16,
    mechanical_layout: u8,
    various_info: u8,
}

/// The immutable first 16 bytes of GetProfilesDescription, *not* a profile
/// sector or a remapping table. Device-specific formats must not be guessed.
fn parse_description(data: [u8; 16]) -> ProfileDescription {
    ProfileDescription {
        memory_model: data[0],
        profile_format: data[1],
        macro_format: data[2],
        profile_count: data[3],
        out_of_box_profiles: data[4],
        button_count: data[5],
        sector_count: data[6],
        sector_size: u16::from_be_bytes([data[7], data[8]]),
        mechanical_layout: data[9],
        various_info: data[10],
    }
}

pub async fn run(args: ProfileInfoArgs) -> Result<()> {
    let (route, name) = select_device(args.device.as_deref(), &[ONBOARD_PROFILES_FEATURE]).await?;
    let features = openlogi_hid::dump_features(&route)
        .await
        .context("enumerate supported HID++ features")?;
    let index = features
        .iter()
        .position(|entry| entry.id == ONBOARD_PROFILES_FEATURE)
        .ok_or_else(|| anyhow!("device does not expose HID++ feature 0x8100"))?;
    let feature_index: u8 = index
        .try_into()
        .context("0x8100 feature index does not fit into one byte")?;
    let channel = openlogi_hid::channel_pool()
        .open(&route)
        .await
        .context("open HID++ device channel")?
        .ok_or_else(|| anyhow!("device HID++ channel unavailable"))?;
    let query = Message::Short(
        MessageHeader {
            device_index: route.device_index(),
            feature_index,
            function_id: U4::from_lo(0),
            software_id: channel.get_sw_id(),
        },
        [0; 3],
    );
    // 0x8100 function 0 = GetProfilesDescription; do not call any other
    // function, read a sector, set onboard mode, or write any profile.
    let reply = channel
        .send_v20(query)
        .await
        .context("read-only 0x8100 GetProfilesDescription request")?;
    let Message::Long(_, payload) = reply else {
        return Err(anyhow!("expected a full 16-byte profile description"));
    };
    let description = parse_description(payload);

    println!("device: {name}");
    println!("0x8100 feature index: {feature_index}");
    println!("memory model: {}", description.memory_model);
    println!("profile format: {}", description.profile_format);
    println!("macro format: {}", description.macro_format);
    println!("profile count: {}", description.profile_count);
    println!("factory profile count: {}", description.out_of_box_profiles);
    println!("firmware button entries: {}", description.button_count);
    println!(
        "sectors: {} x {} bytes",
        description.sector_count, description.sector_size
    );
    println!("mechanical layout: 0x{:02x}", description.mechanical_layout);
    println!("various info: 0x{:02x}", description.various_info);
    println!("Only the descriptor was read. No profile sectors, mappings, or settings changed.");
    println!("Firmware button entries are NOT proof of distinct Windows button events.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_g903_hero_descriptor_as_metadata_only() {
        // Observed in old libratbag G903 HERO logs. The data layout is fixed
        // for the GetProfilesDescription function, regardless of the profile
        // sector layout version.
        let bytes = [
            0x01, 0x04, 0x01, 0x05, 0x01, 0x0c, 0x10, 0x00, 0xff, 0x0a, 0x04, 0, 0, 0, 0, 0,
        ];
        let description = parse_description(bytes);
        assert_eq!(description.memory_model, 1);
        assert_eq!(description.profile_format, 4);
        assert_eq!(description.macro_format, 1);
        assert_eq!(description.profile_count, 5);
        assert_eq!(description.out_of_box_profiles, 1);
        assert_eq!(description.button_count, 12);
        assert_eq!(description.sector_count, 16);
        assert_eq!(description.sector_size, 255);
        assert_eq!(description.mechanical_layout, 10);
        assert_eq!(description.various_info, 4);
    }
}
