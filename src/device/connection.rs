/// Connection management: open, initialize, and interact with an AK680 MAX.
///
/// Supports both Lightless (feature reports) and RGB (interrupt reports)
/// variants. Device discovery iterates all Ajazz VID HID interfaces and
/// probes each vendor-specific endpoint until one responds to GetDeviceInfo.

use std::collections::HashSet;
use hidapi::{HidApi, HidDevice};

use crate::device::DeviceError;
use crate::model::key::Key;
use crate::model::keyboard::{DeviceInfo, DriverProtocol, KeyboardConfig, KeyboardState};
use crate::model::layer::Layer;
use crate::protocol::{commands, key_list, rgb_commands};

// # Known hardware configurations

pub const KNOWN_KEYBOARDS: &[KeyboardConfig] = &[
    KeyboardConfig {
        name: "AK680 MAX (No-RGB)", vendor_id: 0x3151, product_id: 0x502C,
        usage_page: 0xFFFF, min_actuation: 0.1, max_actuation: 3.2,
        rt_min_sensitivity: 0.01, rt_max_sensitivity: 2.0,
        protocol: DriverProtocol::Lightless,
    },
    KeyboardConfig {
        name: "AK680 MAX (RGB)", vendor_id: 0x0C45, product_id: 0x80B2,
        usage_page: 0xFF67, min_actuation: 0.1, max_actuation: 3.4,
        rt_min_sensitivity: 0.01, rt_max_sensitivity: 2.0,
        protocol: DriverProtocol::Rgb,
    },
    KeyboardConfig {
        name: "AK680 V2 (0x80BC)", vendor_id: 0x0C45, product_id: 0x80BC,
        usage_page: 0xFF67, min_actuation: 0.1, max_actuation: 3.3,
        rt_min_sensitivity: 0.01, rt_max_sensitivity: 2.0,
        protocol: DriverProtocol::Rgb,
    },
    KeyboardConfig {
        name: "AK680 V2 (0x80C1)", vendor_id: 0x0C45, product_id: 0x80C1,
        usage_page: 0xFF67, min_actuation: 0.1, max_actuation: 3.3,
        rt_min_sensitivity: 0.01, rt_max_sensitivity: 2.0,
        protocol: DriverProtocol::Rgb,
    },
    KeyboardConfig {
        name: "AK680 V2 (0x80C2)", vendor_id: 0x0C45, product_id: 0x80C2,
        usage_page: 0xFF67, min_actuation: 0.1, max_actuation: 3.3,
        rt_min_sensitivity: 0.01, rt_max_sensitivity: 2.0,
        protocol: DriverProtocol::Rgb,
    },
];

const AJAZZ_VIDS: [u16; 2] = [0x3151, 0x0C45];

// # Diagnostics

#[derive(Debug)]
pub struct ScannedDevice {
    pub vendor_id: u16,
    pub product_id: u16,
    pub usage_page: u16,
    pub usage: u16,
    pub interface_number: i32,
    pub product_string: String,
    pub matched_config: Option<&'static str>,
    pub matched_protocol: Option<DriverProtocol>,
}

impl std::fmt::Display for ScannedDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VID={:#06X} PID={:#06X} page={:#06X} usage={:#06X} iface={} \"{}\"",
            self.vendor_id, self.product_id, self.usage_page,
            self.usage, self.interface_number, self.product_string)?;
        if let Some(name) = self.matched_config { write!(f, " -> {name}")?; }
        Ok(())
    }
}

fn scan_ajazz_devices(api: &HidApi) -> Vec<ScannedDevice> {
    let vids: HashSet<u16> = AJAZZ_VIDS.iter().copied().collect();
    let mut results = Vec::new();
    for info in api.device_list().filter(|d| vids.contains(&d.vendor_id())) {
        let matched = KNOWN_KEYBOARDS.iter()
            .find(|k| k.vendor_id == info.vendor_id()
                    && k.product_id == info.product_id()
                    && k.usage_page == info.usage_page());
        results.push(ScannedDevice {
            vendor_id: info.vendor_id(),
            product_id: info.product_id(),
            usage_page: info.usage_page(),
            usage: info.usage(),
            interface_number: info.interface_number(),
            product_string: info.product_string().unwrap_or("(unknown)").to_string(),
            matched_config: matched.map(|k| k.name),
            matched_protocol: matched.map(|k| k.protocol),
        });
    }
    if results.is_empty() { log::warn!("No Ajazz HID interfaces found"); }
    results
}

// # Connection entry point

/// Discover and connect to the first available AK680 MAX keyboard.
pub fn connect() -> Result<(HidDevice, KeyboardState), DeviceError> {
    let api = HidApi::new()?;
    let scanned = scan_ajazz_devices(&api);

    // Try lightless first (full support), then RGB
    for protocol in [DriverProtocol::Lightless, DriverProtocol::Rgb] {
        if let Some(result) = try_connect_protocol(&api, protocol) {
            return result;
        }
    }
    Err(build_not_found_error(&scanned))
}

fn try_connect_protocol(
    api: &HidApi,
    protocol: DriverProtocol,
) -> Option<Result<(HidDevice, KeyboardState), DeviceError>> {
    for config in KNOWN_KEYBOARDS.iter().filter(|k| k.protocol == protocol) {
        match protocol {
            DriverProtocol::Lightless => {
                for info in api.device_list().filter(|d|
                    d.vendor_id() == config.vendor_id
                    && d.product_id() == config.product_id
                    && d.usage_page() == config.usage_page
                ) {
                    if let Ok(device) = info.open_device(api) {
                        return Some(connect_lightless(device, config));
                    }
                }
            }
            DriverProtocol::Rgb => {
                if let Some(result) = try_rgb_interfaces(api, config) {
                    return Some(result);
                }
            }
        }
    }
    None
}

/// Probe every vendor-specific HID interface for an RGB keyboard.
fn try_rgb_interfaces(
    api: &HidApi,
    config: &KeyboardConfig,
) -> Option<Result<(HidDevice, KeyboardState), DeviceError>> {
    let mut candidates: Vec<_> = api.device_list()
        .filter(|d| d.vendor_id() == config.vendor_id
                  && d.product_id() == config.product_id
                  && d.usage_page() >= 0xFF00)
        .collect();
    if candidates.is_empty() { return None; }
    candidates.sort_by_key(|d| d.interface_number());

    let mut any_opened = false;
    for info in &candidates {
        let Ok(device) = info.open_device(api) else { continue };
        any_opened = true;
        match connect_rgb(device, config) {
            Ok(result) => return Some(Ok(result)),
            Err(e) => { log::warn!("Probe failed on iface {}: {e}", info.interface_number()); }
        }
    }
    any_opened.then(|| Err(DeviceError::Protocol(format!(
        "Tried {} vendor interfaces for {}, none responded", candidates.len(), config.name
    ))))
}

// # Lightless connection

fn connect_lightless(
    device: HidDevice,
    config: &KeyboardConfig,
) -> Result<(HidDevice, KeyboardState), DeviceError> {
    let kl = key_list::ak680_max_lightless_key_list();
    let refs: Vec<Option<&'static str>> = kl.to_vec();
    let active_layer = commands::get_active_layer(&device)?;
    let keys = commands::get_keys(&device, &refs)?;
    let state = KeyboardState::new(config.clone(), active_layer, keys);
    Ok((device, state))
}

// # RGB connection

/// Default actuation in mm when firmware returns 0x0000 ("use default").
const DEFAULT_ACTUATION_MM: f64 = 1.20;

fn connect_rgb(
    device: HidDevice,
    config: &KeyboardConfig,
) -> Result<(HidDevice, KeyboardState), DeviceError> {
    let (raw_info, transport) = rgb_commands::get_device_info(&device)?;

    let device_info = DeviceInfo {
        firmware_version: raw_info.firmware_version,
        battery_level: raw_info.battery_level,
        charge_status: raw_info.charge_status,
        rt_precision: raw_info.rt_precision,
    };
    log::info!("RGB: fw={:.2} bat={}% transport={transport}", device_info.firmware_version, device_info.battery_level);

    let press_table = rgb_commands::read_actuation_table(&device, transport)?;
    let release_table = rgb_commands::read_release_table(&device, transport)?;
    let rgb_table = rgb_commands::read_rgb_table(&device, transport)?;
    let led_state = rgb_commands::get_led_state(&device, transport)?;
    log::info!("LED state: {led_state}");

    let kl = key_list::ak680_max_key_list();
    let keys: Vec<Option<Key>> = kl.iter().enumerate().map(|(idx, entry)| {
        entry.map(|_| {
            let press_raw = rgb_commands::get_key_actuation(&press_table, idx);
            let release_raw = rgb_commands::get_key_actuation(&release_table, idx);
            let color = rgb_commands::get_key_color(&rgb_table, idx);
            let press_mm = if press_raw == 0 { DEFAULT_ACTUATION_MM } else { press_raw as f64 / 100.0 };
            let release_mm = if release_raw == 0 { DEFAULT_ACTUATION_MM } else { release_raw as f64 / 100.0 };
            Key {
                code: idx,
                down_actuation: press_mm,
                up_actuation: release_mm,
                rapid_trigger: false,
                rt_press_sensitivity: 0.0,
                rt_release_sensitivity: 0.0,
                color: crate::model::key::KeyColor { r: color.r, g: color.g, b: color.b },
            }
        })
    }).collect();

    log::info!("Loaded {} keys", keys.iter().flatten().count());

    let mut state = KeyboardState::new(config.clone(), Layer::Layer1, keys);
    state.device_info = Some(device_info);
    state.transport = Some(transport);
    state.raw_actuation_table = Some(press_table);
    state.raw_release_table = Some(release_table);
    state.raw_rgb_table = Some(rgb_table);
    state.led_state = Some(led_state);
    state.fully_supported = true;
    Ok((device, state))
}

// # Post-connection operations

/// Apply all RGB key settings and switch to Custom Per-Key mode (0x14).
pub fn apply_rgb_keys(
    device: &HidDevice,
    keys: &[Option<Key>],
    transport: rgb_commands::Transport,
    press_table: &[u8],
    release_table: &[u8],
    rgb_table: &[u8],
    current_led: Option<rgb_commands::LedState>,
) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>, rgb_commands::LedState), DeviceError> {
    let mut new_press = press_table.to_vec();
    let mut new_release = release_table.to_vec();
    let mut new_rgb = rgb_table.to_vec();
    new_press.resize(new_press.len().max(1024), 0);
    new_release.resize(new_release.len().max(1024), 0);
    new_rgb.resize(new_rgb.len().max(512), 0);

    for key in keys.iter().flatten() {
        rgb_commands::set_key_actuation(&mut new_press, key.code, (key.down_actuation * 100.0).round() as u16);
        rgb_commands::set_key_actuation(&mut new_release, key.code, (key.up_actuation * 100.0).round() as u16);
        rgb_commands::set_key_color(&mut new_rgb, key.code,
            rgb_commands::KeyColor::new(key.color.r, key.color.g, key.color.b));
    }

    rgb_commands::write_actuation_table(device, transport, &new_press)?;
    rgb_commands::write_release_table(device, transport, &new_release)?;
    rgb_commands::write_rgb_table(device, transport, &new_rgb)?;

    let brightness = current_led.map(|l| l.brightness.max(1)).unwrap_or(5);
    let led = rgb_commands::LedState::custom_colors(brightness);
    rgb_commands::set_led_state(device, transport, &led)?;

    Ok((new_press, new_release, new_rgb, led))
}

pub fn refresh_keys(device: &HidDevice) -> Result<Vec<Option<Key>>, DeviceError> {
    let kl = key_list::ak680_max_lightless_key_list();
    let refs: Vec<Option<&'static str>> = kl.to_vec();
    Ok(commands::get_keys(device, &refs)?)
}

pub fn apply_all_keys(device: &HidDevice, keys: &[Option<Key>]) -> Result<(), DeviceError> {
    commands::apply_keys(device, keys)?;
    Ok(())
}

pub fn switch_layer(device: &HidDevice, layer: Layer) -> Result<Vec<Option<Key>>, DeviceError> {
    commands::set_active_layer(device, layer)?;
    std::thread::sleep(std::time::Duration::from_millis(250));
    refresh_keys(device)
}

fn build_not_found_error(scanned: &[ScannedDevice]) -> DeviceError {
    if scanned.is_empty() {
        return DeviceError::NotFound(
            "No Ajazz HID devices detected.\n\n\
             - Is the keyboard plugged in via USB?\n\
             - Is another config tool running?\n\
             - Try a different USB port.".into());
    }
    let mut msg = format!("Found {} Ajazz interfaces but none responded:\n\n", scanned.len());
    for d in scanned { msg.push_str(&format!("  {d}\n")); }
    DeviceError::NotFound(msg)
}