//! What a connected device is: its name and whether it is a headset

use crate::adb::{Adb, Error};

/// System features which only headsets declare
///
/// The Android XR features come first, then the vendor ones of the headsets which predate them.
const HEADSET_FEATURES: &[&str] = &[
    "android.software.xr.api.openxr",
    "android.software.xr.api.spatial",
    "android.hardware.vr.headtracking",
    "oculus.hardware.standalone_vr",
];

/// One shell command printing the name chosen by the user, the model, then the system features
///
/// Each value is printed on its own line even when empty, so that the features always start on
/// the third line. `echo` is built in every Android shell, unlike `printf` (missing on Android 5).
const QUERY: &str = r#"echo "$(settings get global device_name 2>/dev/null)"; echo "$(getprop ro.product.model)"; pm list features"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    /// The name shown by the device in its settings, or its model
    pub name: Option<String>,
    pub headset: bool,
}

/// Ask the device for its name and kind
pub fn device_info(adb: &Adb) -> Result<DeviceInfo, Error> {
    adb.output(QUERY).map(|output| parse(&output))
}

fn parse(output: &str) -> DeviceInfo {
    let mut lines = output.lines().map(str::trim);
    let device_name = lines.next();
    let model = lines.next();
    let name = [device_name, model]
        .into_iter()
        .flatten()
        // `settings` prints "null" for a name never set
        .find(|name| !name.is_empty() && *name != "null")
        .map(str::to_owned);
    let headset = lines
        .filter_map(|line| line.strip_prefix("feature:"))
        // versioned features look like `name=version`
        .map(|feature| feature.split('=').next().unwrap_or(feature))
        .any(|feature| HEADSET_FEATURES.contains(&feature));
    DeviceInfo { name, headset }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_a_quest() {
        let output = "Quest 3S\r\nQuest 3S\r\nfeature:reqGlEsVersion=0x30002\r\n\
                      feature:android.hardware.wifi\r\nfeature:oculus.hardware.standalone_vr\r\n";
        assert_eq!(
            DeviceInfo {
                name: Some("Quest 3S".to_owned()),
                headset: true,
            },
            parse(output)
        );
    }

    #[test]
    fn detect_an_android_xr_headset_by_a_versioned_feature() {
        let output = "\nGalaxy XR\nfeature:android.software.xr.api.openxr=65537\n";
        assert!(parse(output).headset);
    }

    #[test]
    fn detect_a_phone() {
        let output = "Alice's Pixel\nPixel 8\nfeature:android.hardware.telephony\n\
                      feature:android.software.vr.mode\n";
        assert_eq!(
            DeviceInfo {
                name: Some("Alice's Pixel".to_owned()),
                headset: false,
            },
            parse(output)
        );
    }

    #[test]
    fn fall_back_to_the_model() {
        assert_eq!(Some("Pixel 8"), parse("null\nPixel 8\n").name.as_deref());
        assert_eq!(None, parse("\n\n").name);
    }
}
