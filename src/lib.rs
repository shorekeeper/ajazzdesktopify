/// AK680 MAX keyboard driver library.
///
/// Provides HID communication, protocol encoding/decoding, and device
/// management for both the Lightless (no-RGB, VID `0x3151`) and RGB
/// (VID `0x0C45`) hardware variants.
pub mod device;
pub mod model;
pub mod protocol;