use super::key::Key;
use super::layer::Layer;
use crate::protocol::rgb_commands::{LedState, Transport};

/// Which wire protocol the keyboard firmware speaks.
///
/// - `Lightless`: VID `0x3151`, PID `0x502C`. 64-byte HID feature reports
///   with checksum. Commands `0xE5`/`0x65` for actuation.
/// - `Rgb`: VID `0x0C45`, PID `0x80B2`. 64-byte interrupt reports on
///   interface `0xFF68`. Commands `0x1X`/`0x2X` for read/write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverProtocol {
    Lightless,
    Rgb,
}

/// Hardware limits and identifiers for a specific keyboard model.
#[derive(Debug, Clone)]
pub struct KeyboardConfig {
    pub name: &'static str,
    pub vendor_id: u16,
    pub product_id: u16,
    pub usage_page: u16,
    pub min_actuation: f64,
    pub max_actuation: f64,
    pub rt_min_sensitivity: f64,
    pub rt_max_sensitivity: f64,
    pub protocol: DriverProtocol,
}

/// Metadata retrieved from the keyboard at connection time.
///
/// Populated from CMD `0x10` (GetDeviceInfo) response:
/// ```text
/// RX: 55 10 30 00 00 00 00 00
///     00 00 00 [ver] [vid_lo] [vid_hi] [pid_lo] [pid_hi]
///     ...  [battery] [charge] ...  [rt_precision]
/// ```
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub firmware_version: f64,
    pub battery_level: u8,
    pub charge_status: u8,
    pub rt_precision: u8,
}

/// Full mutable state for a connected keyboard.
pub struct KeyboardState {
    pub config: KeyboardConfig,
    pub active_layer: Layer,
    pub keys: Vec<Option<Key>>,
    pub busy: bool,
    pub has_unsaved_changes: bool,
    pub device_info: Option<DeviceInfo>,
    pub fully_supported: bool,
    pub transport: Option<Transport>,
    pub raw_actuation_table: Option<Vec<u8>>,
    pub raw_release_table: Option<Vec<u8>>,
    pub raw_rgb_table: Option<Vec<u8>>,
    pub led_state: Option<LedState>,
}

impl std::fmt::Debug for KeyboardState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyboardState")
            .field("config", &self.config.name)
            .field("layer", &self.active_layer)
            .field("keys", &self.keys.iter().flatten().count())
            .field("led", &self.led_state)
            .finish()
    }
}

impl KeyboardState {
    pub fn new(config: KeyboardConfig, active_layer: Layer, keys: Vec<Option<Key>>) -> Self {
        Self {
            config,
            active_layer,
            keys,
            busy: false,
            has_unsaved_changes: false,
            device_info: None,
            fully_supported: true,
            transport: None,
            raw_actuation_table: None,
            raw_release_table: None,
            raw_rgb_table: None,
            led_state: None,
        }
    }

    pub fn new_limited(
        config: KeyboardConfig,
        device_info: DeviceInfo,
        transport: Transport,
    ) -> Self {
        Self {
            config,
            active_layer: Layer::Layer1,
            keys: Vec::new(),
            busy: false,
            has_unsaved_changes: false,
            device_info: Some(device_info),
            fully_supported: false,
            transport: Some(transport),
            raw_actuation_table: None,
            raw_release_table: None,
            raw_rgb_table: None,
            led_state: None,
        }
    }
}