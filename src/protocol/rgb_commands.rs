/// HID protocol driver for the AK680 MAX RGB variant.
///
/// # Wire format
///
/// All communication uses 64-byte interrupt reports on HID interface 2
/// (usage page `0xFF68`). With the 1-byte report ID prepended by the HID
/// layer, each USB transfer is 65 bytes.
///
/// ```text
/// Byte [0]    Magic       0xAA (host -> device) / 0x55 (device -> host)
/// Byte [1]    Command     operation ID
/// Byte [2]    Length      payload bytes this chunk (max 0x38 = 56)
/// Byte [3]    Offset_lo   low byte of 16-bit byte offset into flash region
/// Byte [4]    Offset_hi   high byte
/// Byte [5]    Sub-cmd     operation-specific parameter
/// Byte [6]    Flags       0x01 on the last chunk of a write sequence
/// Byte [7]    Reserved    always 0x00
/// Byte [8..63] Payload    up to 56 bytes of data
/// ```
///
/// # Command table
///
/// | Read | Write | Flash    | Description        | Size    |
/// |------|-------|----------|--------------------|---------|
/// | 0x10 | --    | `0x9000` | DeviceInfo         | 48B     |
/// | 0x11 | 0x21  | `0x9200` | Device config      | ~512B   |
/// | 0x12 | 0x22  | `0x9600` | Key mapping        | ~512B   |
/// | 0x13 | 0x23  | SRAM     | LED state register | 24B     |
/// | 0x14 | 0x24  | `0x9A00` | Per-key RGB colors | 512B    |
/// | 0x15 | 0x25  | `0x9C00` | LED animation cfg  | ~4608B  |
/// | 0x17 | 0x27  | `0xB600` | Press actuation    | 1024B   |
/// | 0x18 | 0x28  | `0xB200` | Release actuation  | 1024B   |
///
/// # Chunked transfer
///
/// Tables larger than 56 bytes are split into chunks with incrementing
/// byte offsets:
///
/// ```text
/// Chunk 0: offset=0x0000  length=0x38 (56 bytes)
/// Chunk 1: offset=0x0038  length=0x38
/// Chunk 2: offset=0x0070  length=0x38
/// ...
/// Chunk N: offset=N*56    length=remaining
/// ```
///
/// For writes, the last chunk sets `header[6] = 0x01`. The firmware echoes
/// each written chunk in its response as confirmation.
///
/// # Example: reading DeviceInfo
///
/// ```text
/// TX: AA 10 30 00 00 00 01 00  [48 zero bytes]
/// RX: 55 10 30 00 00 00 01 00  00 00 00 92 45 0C B2 80  06 01 00 00 66 01 04 15
///                               ^^^^^^^^ ^^^^^ ^^^^^
///                               version  VID   PID
/// ```
///
/// # Vulnerability
///
/// Read commands do not validate offset against table boundaries.
/// CMD `0x17` with offset > 0x400 reads arbitrary flash, allowing a
/// full 64KB firmware dump.

use std::fmt;
use hidapi::HidDevice;
use crate::device::DeviceError;

// # Constants

const REPORT_SIZE: usize = 64;
const HEADER_SIZE: usize = 8;
const DATA_SIZE: usize = REPORT_SIZE - HEADER_SIZE; // 56

const MAGIC_OUT: u8 = 0xAA;
const MAGIC_IN: u8 = 0x55;

const MAX_RETRIES: u32 = 3;
const READ_TIMEOUT_MS: i32 = 1000;

// # Transport

/// Describes which HID API calls to use for send and receive.
///
/// The correct strategy depends on the OS and which HID interface
/// is open. For the RGB model on Windows, `OutputReport` works on
/// interface 2 (`0xFF68`). Interface 3 (`0xFF67`) requires feature
/// reports but does not process commands (firmware update endpoint).
///
/// Discovery is automatic via `probe_transport()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    /// `write()` + `read_timeout()` via interrupt endpoints.
    OutputReport,
    /// `send_feature_report()` + `get_feature_report()` via control pipe.
    FeatureReport,
    /// `send_feature_report()` for TX, `read_timeout()` for RX (hybrid).
    MixedFeatureWrite,
}

impl fmt::Display for Transport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutputReport => write!(f, "OutputReport (write + read)"),
            Self::FeatureReport => write!(f, "FeatureReport (feature send + feature get)"),
            Self::MixedFeatureWrite => write!(f, "Mixed (feature send + interrupt read)"),
        }
    }
}

const TRANSPORT_PROBE_ORDER: [Transport; 3] = [
    Transport::OutputReport,
    Transport::MixedFeatureWrite,
    Transport::FeatureReport,
];

// # Data types

/// Information returned by CMD `0x10` (GetDeviceInfo).
///
/// Parsed from the 56-byte payload:
/// ```text
/// [+3]   BCD firmware version (lo nibble + hi nibble * 10 + byte[4] * 100)
/// [+5,6] Manufacturer ID (u16 LE, e.g. 0x0C45)
/// [+7,8] Product ID (u16 LE, e.g. 0x80B2)
/// [+10]  Battery level (0-100, 0 if wired)
/// [+11]  Charge status
/// [+22]  RT precision
/// ```
#[derive(Debug, Clone)]
pub struct RgbDeviceInfo {
    pub firmware_version: f64,
    pub manufacturer_id: u16,
    pub product_id: u16,
    pub battery_level: u8,
    pub charge_status: u8,
    pub rt_precision: u8,
}

/// LED effect identifiers.
///
/// The keyboard supports 20 effects (0x01-0x14) plus OFF (0x00).
/// Effects 0x01-0x13 cycle with Fn+\\; effect 0x14 ("Custom Per-Key")
/// is only reachable via HID command `0x23` and shows per-key colors
/// from the CMD `0x14` color table.
///
/// The state register (CMD `0x13`) reports the current effect:
///
/// ```text
/// RX: 55 13 18 00 00 00 00 00
///     [id] FF [af] [af] 00 00 00 00  [engine] [bright] [speed] 00 ...
///      |       |                      |        |        |
///      |       anim flags (FF=anim)   0x01=on  1-5      1-5
///      effect ID (0x00-0x14)
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EffectId {
    Off             = 0x00,
    SolidColor      = 0x01,
    KeypressLight   = 0x02,
    Breathing       = 0x03,
    Starfall        = 0x04,
    Rain            = 0x05,
    RainbowShimmer  = 0x06,
    Fade            = 0x07,
    RainbowWave     = 0x08,
    CenterWaves     = 0x09,
    TopDownWave     = 0x0A,
    ColorPulseWave  = 0x0B,
    RainbowRotation = 0x0C,
    RowFlash        = 0x0D,
    RippleH         = 0x0E,
    RippleRadial    = 0x0F,
    Scanner         = 0x10,
    CenterPulse     = 0x11,
    ShoreWaves      = 0x12,
    RowDiverge      = 0x13,
    CustomColors    = 0x14,
}

impl EffectId {
    /// All 21 effects including Off and CustomColors.
    pub const ALL: [Self; 21] = [
        Self::Off, Self::SolidColor, Self::KeypressLight, Self::Breathing,
        Self::Starfall, Self::Rain, Self::RainbowShimmer, Self::Fade,
        Self::RainbowWave, Self::CenterWaves, Self::TopDownWave,
        Self::ColorPulseWave, Self::RainbowRotation, Self::RowFlash,
        Self::RippleH, Self::RippleRadial, Self::Scanner, Self::CenterPulse,
        Self::ShoreWaves, Self::RowDiverge, Self::CustomColors,
    ];

    /// Effects available in the UI picker (excludes Off and CustomColors).
    pub const UI_LIST: [Self; 19] = [
        Self::SolidColor, Self::KeypressLight, Self::Breathing,
        Self::Starfall, Self::Rain, Self::RainbowShimmer, Self::Fade,
        Self::RainbowWave, Self::CenterWaves, Self::TopDownWave,
        Self::ColorPulseWave, Self::RainbowRotation, Self::RowFlash,
        Self::RippleH, Self::RippleRadial, Self::Scanner, Self::CenterPulse,
        Self::ShoreWaves, Self::RowDiverge,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Off             => "Off",
            Self::SolidColor      => "Solid Color",
            Self::KeypressLight   => "Keypress Light",
            Self::Breathing       => "Breathing",
            Self::Starfall        => "Starfall",
            Self::Rain            => "Rain",
            Self::RainbowShimmer  => "Rainbow Shimmer",
            Self::Fade            => "Fade",
            Self::RainbowWave     => "Rainbow Wave",
            Self::CenterWaves     => "Center Waves",
            Self::TopDownWave     => "Top-Down Wave",
            Self::ColorPulseWave  => "Color Pulse Wave",
            Self::RainbowRotation => "Rainbow Rotation",
            Self::RowFlash        => "Row Flash",
            Self::RippleH         => "Ripple Horizontal",
            Self::RippleRadial    => "Ripple Radial",
            Self::Scanner         => "Scanner",
            Self::CenterPulse     => "Center Pulse",
            Self::ShoreWaves      => "Shore Waves",
            Self::RowDiverge      => "Row Diverge",
            Self::CustomColors    => "Custom Per-Key",
        }
    }

    pub const fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x00 => Some(Self::Off),             0x01 => Some(Self::SolidColor),
            0x02 => Some(Self::KeypressLight),   0x03 => Some(Self::Breathing),
            0x04 => Some(Self::Starfall),        0x05 => Some(Self::Rain),
            0x06 => Some(Self::RainbowShimmer),  0x07 => Some(Self::Fade),
            0x08 => Some(Self::RainbowWave),     0x09 => Some(Self::CenterWaves),
            0x0A => Some(Self::TopDownWave),      0x0B => Some(Self::ColorPulseWave),
            0x0C => Some(Self::RainbowRotation), 0x0D => Some(Self::RowFlash),
            0x0E => Some(Self::RippleH),         0x0F => Some(Self::RippleRadial),
            0x10 => Some(Self::Scanner),         0x11 => Some(Self::CenterPulse),
            0x12 => Some(Self::ShoreWaves),      0x13 => Some(Self::RowDiverge),
            0x14 => Some(Self::CustomColors),
            _    => None,
        }
    }

    /// Whether the LED animation engine must be active for this effect.
    ///
    /// Effects with `needs_engine() == false` use static colors from flash
    /// (no frame-by-frame updates). CustomColors (0x14) requires the engine
    /// to read from the per-key color table.
    ///
    /// Verified empirically:
    /// ```text
    /// Static  (engine=0, anim=[00,00]): 0x01, 0x06, 0x08, 0x0B
    /// Dynamic (engine=1, anim=[FF,FF]): all others including 0x14
    /// ```
    pub const fn needs_engine(self) -> bool {
        matches!(
            self,
            Self::Off | Self::SolidColor | Self::RainbowShimmer
                | Self::RainbowWave | Self::ColorPulseWave
        ) == false
    }
}

impl fmt::Display for EffectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:02X} ({})", *self as u8, self.name())
    }
}

/// Current LED state readable from CMD `0x13` and writable via CMD `0x23`.
///
/// # Wire layout (CMD `0x23` payload at bytes `[8..]`)
///
/// ```text
/// [+0]  effect ID       0x00-0x14
/// [+1]  constant        always 0xFF
/// [+2]  anim flag 0     0xFF if animated, 0x00 if static
/// [+3]  anim flag 1     same as [+2]
/// [+4..+7] reserved     0x00
/// [+8]  engine          0x01 if animation engine active, 0x00 otherwise
/// [+9]  brightness      1-5
/// [+10] speed           1-5
/// ```
///
/// Example: set Rainbow Rotation at full brightness, speed 3:
///
/// ```text
/// TX: AA 23 18 00 00 00 00 00
///     0C FF FF FF 00 00 00 00  01 05 03 00 ...
/// ```
///
/// Example: set Custom Per-Key colors:
///
/// ```text
/// TX: AA 23 18 00 00 00 00 00
///     14 FF FF FF 00 00 00 00  01 05 03 00 ...
/// ```
///
/// Example: turn off LEDs (all zeros):
///
/// ```text
/// TX: AA 23 18 00 00 00 00 00  00 00 00 00 00 00 00 00  00 00 00 00 ...
/// ```
#[derive(Debug, Clone, Copy)]
pub struct LedState {
    pub effect: EffectId,
    /// Brightness level, 1-5. 0 when off.
    pub brightness: u8,
    /// Animation speed, 1-5. 0 when off or irrelevant.
    pub speed: u8,
}

impl LedState {
    pub fn off() -> Self {
        Self { effect: EffectId::Off, brightness: 0, speed: 0 }
    }

    pub fn new(effect: EffectId, brightness: u8, speed: u8) -> Self {
        if effect == EffectId::Off {
            return Self::off();
        }
        Self {
            effect,
            brightness: brightness.clamp(1, 5),
            speed: speed.clamp(1, 5),
        }
    }

    /// Convenience constructor for Custom Per-Key mode.
    pub fn custom_colors(brightness: u8) -> Self {
        Self::new(EffectId::CustomColors, brightness, 3)
    }
}

impl fmt::Display for LedState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "effect={} bright={} speed={}", self.effect, self.brightness, self.speed)
    }
}

/// Per-key color (R, G, B).
///
/// Stored in the color table (CMD `0x14`/`0x24`) at `key_code * 4`:
/// ```text
/// [+0] u8  LED index (= key_code)
/// [+1] u8  Red
/// [+2] u8  Green
/// [+3] u8  Blue
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct KeyColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl KeyColor {
    pub const fn new(r: u8, g: u8, b: u8) -> Self { Self { r, g, b } }
    pub const fn is_zero(self) -> bool { self.r == 0 && self.g == 0 && self.b == 0 }
}

// # Low-level I/O (private)

fn prepend_report_id(report: &[u8; REPORT_SIZE]) -> [u8; REPORT_SIZE + 1] {
    let mut buf = [0u8; REPORT_SIZE + 1];
    buf[0] = 0x00;
    buf[1..].copy_from_slice(report);
    buf
}

fn send_raw(device: &HidDevice, report: &[u8; REPORT_SIZE], transport: Transport) -> Result<(), DeviceError> {
    let buf = prepend_report_id(report);
    match transport {
        Transport::OutputReport => { device.write(&buf)?; }
        Transport::FeatureReport | Transport::MixedFeatureWrite => {
            device.send_feature_report(&buf)?;
        }
    }
    log::trace!("TX [{transport}]: {:02X?}", &report[..HEADER_SIZE]);
    Ok(())
}

fn recv_raw(device: &HidDevice, transport: Transport) -> Result<[u8; REPORT_SIZE], DeviceError> {
    let mut buf = [0u8; REPORT_SIZE + 1];
    buf[0] = 0x00;

    let n = match transport {
        Transport::OutputReport | Transport::MixedFeatureWrite => {
            device.read_timeout(&mut buf, READ_TIMEOUT_MS)?
        }
        Transport::FeatureReport => device.get_feature_report(&mut buf)?,
    };

    if n == 0 {
        return Err(DeviceError::Protocol(format!("Read returned 0 bytes ({transport})")));
    }

    // Locate the 0x55 magic byte (may be at [0] or [1] depending on report ID handling)
    let start = match (buf[0], buf.get(1)) {
        (MAGIC_IN, _) => 0,
        (_, Some(&MAGIC_IN)) if n >= 2 => 1,
        _ => return Err(DeviceError::Protocol(format!(
            "No magic 0x{MAGIC_IN:02X} in {n} bytes (first: [{:#04X}, {:#04X}])",
            buf[0], buf.get(1).copied().unwrap_or(0),
        ))),
    };

    if start + REPORT_SIZE > n {
        return Err(DeviceError::Protocol(format!(
            "Short read: {n} bytes from offset {start} (need {REPORT_SIZE})"
        )));
    }

    let mut out = [0u8; REPORT_SIZE];
    out.copy_from_slice(&buf[start..start + REPORT_SIZE]);
    log::trace!("RX [{transport}]: {:02X?}", &out[..HEADER_SIZE]);
    Ok(out)
}

/// Send a report and wait for a matching response, with retries.
fn exchange(
    device: &HidDevice,
    report: &[u8; REPORT_SIZE],
    expected_cmd: u8,
    transport: Transport,
) -> Result<[u8; REPORT_SIZE], DeviceError> {
    for attempt in 0..=MAX_RETRIES {
        send_raw(device, report, transport)?;
        match recv_raw(device, transport) {
            Ok(resp) if resp[1] == expected_cmd => return Ok(resp),
            Ok(resp) => log::warn!(
                "CMD mismatch: expected 0x{expected_cmd:02X}, got 0x{:02X} (attempt {})",
                resp[1], attempt + 1,
            ),
            Err(e) if attempt < MAX_RETRIES => {
                log::warn!("Read error (attempt {}): {e}", attempt + 1);
            }
            Err(e) => return Err(e),
        }
    }
    Err(DeviceError::Protocol("All retries exhausted".into()))
}

// # Transport probing

/// Try each transport strategy with a GetDeviceInfo command.
/// Returns the first strategy that completes a round-trip.
pub fn probe_transport(device: &HidDevice) -> Result<(Transport, [u8; REPORT_SIZE]), DeviceError> {
    let mut request = [0u8; REPORT_SIZE];
    request[0] = MAGIC_OUT;
    request[1] = 0x10; // GetDeviceInfo
    request[2] = 0x30; // length = 48
    request[5] = 0x01; // sub

    let mut last_err = String::new();
    for &tr in &TRANSPORT_PROBE_ORDER {
        log::info!("Probing transport: {tr} ...");
        if let Err(e) = send_raw(device, &request, tr) {
            log::warn!("  send failed: {e}");
            last_err = format!("{tr} send: {e}");
            continue;
        }
        match recv_raw(device, tr) {
            Ok(resp) if resp[1] == 0x10 => {
                log::info!("  Transport {tr} works!");
                return Ok((tr, resp));
            }
            Ok(resp) => {
                log::warn!("  CMD mismatch: 0x{:02X}", resp[1]);
                last_err = format!("{tr}: cmd mismatch");
            }
            Err(e) => {
                log::warn!("  recv failed: {e}");
                last_err = format!("{tr} recv: {e}");
            }
        }
    }
    Err(DeviceError::Protocol(format!("All transports failed. Last: {last_err}")))
}

// # DeviceInfo (CMD 0x10)

/// Query device info, also discovering the working transport.
///
/// ```text
/// TX: AA 10 30 00 00 00 01 00  [48 zeros]
/// RX: 55 10 30 00 00 00 01 00  [device info payload]
/// ```
pub fn get_device_info(device: &HidDevice) -> Result<(RgbDeviceInfo, Transport), DeviceError> {
    let (transport, response) = probe_transport(device)?;
    let d = &response[HEADER_SIZE..];
    if d.len() < 30 {
        return Err(DeviceError::Protocol(format!("DeviceInfo too short: {} bytes", d.len())));
    }

    let firmware_version = {
        let lo = (d[3] & 0x0F) as f64;
        let hi = ((d[3] & 0xF0) >> 4) as f64 * 10.0;
        let major = d[4] as f64 * 100.0;
        (lo + hi + major) / 100.0
    };

    let info = RgbDeviceInfo {
        firmware_version,
        manufacturer_id: u16::from_le_bytes([d[5], d[6]]),
        product_id: u16::from_le_bytes([d[7], d[8]]),
        battery_level: d[10],
        charge_status: d[11],
        rt_precision: d[22],
    };

    log::info!("RGB DeviceInfo: {info:?}");
    Ok((info, transport))
}

// # LED state (CMD 0x13 / 0x23)

/// Read the current LED state register.
///
/// ```text
/// TX: AA 13 18 00 00 00 00 00  [24 zeros]
/// RX: 55 13 18 00 00 00 00 00
///     [id] FF [af] [af] 00 00 00 00  [engine] [bright] [speed] 00 ...
/// ```
pub fn get_led_state(device: &HidDevice, transport: Transport) -> Result<LedState, DeviceError> {
    let mut report = [0u8; REPORT_SIZE];
    report[0] = MAGIC_OUT;
    report[1] = 0x13;
    report[2] = 0x18;

    let resp = exchange(device, &report, 0x13, transport)?;
    let d = &resp[HEADER_SIZE..];
    Ok(LedState {
        effect: EffectId::from_byte(d[0]).unwrap_or(EffectId::Off),
        brightness: d[9],
        speed: d[10],
    })
}

/// Write LED state (takes effect immediately, no Apply needed).
///
/// See [`LedState`] docs for the wire layout.
pub fn set_led_state(
    device: &HidDevice,
    transport: Transport,
    state: &LedState,
) -> Result<(), DeviceError> {
    let mut report = [0u8; REPORT_SIZE];
    report[0] = MAGIC_OUT;
    report[1] = 0x23;
    report[2] = 0x18;

    if state.effect != EffectId::Off {
        let engine = state.effect.needs_engine();
        report[8]  = state.effect as u8;
        report[9]  = 0xFF;
        report[10] = if engine { 0xFF } else { 0x00 };
        report[11] = if engine { 0xFF } else { 0x00 };
        report[16] = if engine { 0x01 } else { 0x00 };
        report[17] = state.brightness;
        report[18] = state.speed;
    }
    // Off: all zeros (already default)

    exchange(device, &report, 0x23, transport)?;
    Ok(())
}

// # Actuation tables (CMD 0x17/0x27 press, CMD 0x18/0x28 release)

/// Bytes per key record in the actuation table.
const ACT_BYTES_PER_KEY: usize = 8;
/// Offset of the u16 actuation value within each 8-byte record.
const ACT_FIELD_OFFSET: usize = 2;
/// Total press actuation table: 128 slots * 8 bytes = 1024.
const ACT_TABLE_SIZE: usize = 1024;
const ACT_CHUNKS: usize = (ACT_TABLE_SIZE + DATA_SIZE - 1) / DATA_SIZE;

/// Release data starts 1024 bytes into the CMD 0x18 response.
const REL_DATA_OFFSET: usize = 0x400;
const REL_READ_SIZE: usize = REL_DATA_OFFSET + ACT_TABLE_SIZE;
const REL_CHUNKS: usize = (REL_READ_SIZE + DATA_SIZE - 1) / DATA_SIZE;

/// Read a table via chunked reads.
fn read_table(
    device: &HidDevice,
    transport: Transport,
    cmd: u8,
    total_bytes: usize,
) -> Result<Vec<u8>, DeviceError> {
    let chunks = (total_bytes + DATA_SIZE - 1) / DATA_SIZE;
    let mut data = Vec::with_capacity(total_bytes);

    for chunk in 0..chunks {
        let offset = chunk * DATA_SIZE;
        let len = (total_bytes - offset).min(DATA_SIZE);

        let mut report = [0u8; REPORT_SIZE];
        report[0] = MAGIC_OUT;
        report[1] = cmd;
        report[2] = len as u8;
        report[3] = (offset & 0xFF) as u8;
        report[4] = ((offset >> 8) & 0xFF) as u8;

        let resp = exchange(device, &report, cmd, transport)?;
        data.extend_from_slice(&resp[HEADER_SIZE..HEADER_SIZE + len]);
    }

    data.truncate(total_bytes);
    Ok(data)
}

/// Write a table via chunked writes.
fn write_table(
    device: &HidDevice,
    transport: Transport,
    cmd: u8,
    data: &[u8],
) -> Result<(), DeviceError> {
    let total = data.len();
    let chunks = (total + DATA_SIZE - 1) / DATA_SIZE;

    for chunk in 0..chunks {
        let offset = chunk * DATA_SIZE;
        let len = (total - offset).min(DATA_SIZE);
        let is_last = chunk == chunks - 1;

        let mut report = [0u8; REPORT_SIZE];
        report[0] = MAGIC_OUT;
        report[1] = cmd;
        report[2] = len as u8;
        report[3] = (offset & 0xFF) as u8;
        report[4] = ((offset >> 8) & 0xFF) as u8;
        if is_last { report[6] = 0x01; }
        report[HEADER_SIZE..HEADER_SIZE + len].copy_from_slice(&data[offset..offset + len]);

        exchange(device, &report, cmd, transport)?;
    }

    Ok(())
}

/// Read press actuation table (1024 bytes from flash `0xB600`).
///
/// ```text
/// TX: AA 17 38 00 00 00 00 00  [56 zeros]         (chunk 0)
/// RX: 55 17 38 00 00 00 00 00  [56 bytes of data]
/// TX: AA 17 38 38 00 00 00 00  ...                 (chunk 1, offset=56)
/// ...
/// ```
///
/// Total: 19 chunks covering 1024 bytes.
pub fn read_actuation_table(device: &HidDevice, transport: Transport) -> Result<Vec<u8>, DeviceError> {
    read_table(device, transport, 0x17, ACT_TABLE_SIZE)
}

/// Write press actuation table.
///
/// ```text
/// TX: AA 27 38 00 00 00 00 00  [56 bytes data]    (chunk 0)
/// RX: 55 27 38 00 00 00 00 00  [echo]
/// ...
/// TX: AA 27 10 F0 03 00 01 00  [16 bytes data]    (last chunk, flag=0x01)
/// ```
pub fn write_actuation_table(device: &HidDevice, transport: Transport, table: &[u8]) -> Result<(), DeviceError> {
    if table.len() < ACT_TABLE_SIZE {
        return Err(DeviceError::Protocol(format!("Press table too short: {} < {ACT_TABLE_SIZE}", table.len())));
    }
    write_table(device, transport, 0x27, &table[..ACT_TABLE_SIZE])
}

/// Read release actuation table.
///
/// Release data starts at byte offset `0x0400` in the CMD `0x18` response.
/// We read the full `0x0400 + 1024 = 2048` bytes and return only the last 1024.
///
/// ```text
/// TX: AA 18 38 00 00 00 00 00  ...  (offset 0x0000, zeros returned)
/// ...
/// TX: AA 18 38 00 04 00 00 00  ...  (offset 0x0400, actual data starts)
/// ```
pub fn read_release_table(device: &HidDevice, transport: Transport) -> Result<Vec<u8>, DeviceError> {
    let raw = read_table(device, transport, 0x18, REL_READ_SIZE)?;
    if raw.len() < REL_DATA_OFFSET + ACT_TABLE_SIZE {
        let mut padded = raw;
        padded.resize(REL_DATA_OFFSET + ACT_TABLE_SIZE, 0);
        return Ok(padded[REL_DATA_OFFSET..].to_vec());
    }
    Ok(raw[REL_DATA_OFFSET..REL_DATA_OFFSET + ACT_TABLE_SIZE].to_vec())
}

/// Write release actuation table.
///
/// Must prepend 1024 zero bytes (the `0x0400` prefix region) before the
/// actual table data, totaling 2048 bytes / 37 chunks.
pub fn write_release_table(device: &HidDevice, transport: Transport, table: &[u8]) -> Result<(), DeviceError> {
    if table.len() < ACT_TABLE_SIZE {
        return Err(DeviceError::Protocol(format!("Release table too short: {} < {ACT_TABLE_SIZE}", table.len())));
    }
    let mut full = vec![0u8; REL_DATA_OFFSET];
    full.extend_from_slice(&table[..ACT_TABLE_SIZE]);
    write_table(device, transport, 0x28, &full)
}

/// Extract actuation value (hundredths of mm) for a key from the raw table.
///
/// ```text
/// offset = key_code * 8 + 2
/// value  = u16 LE at that offset
/// ```
///
/// Example: key 0 (Esc) at offset 2, value `0x004B` = 75 = 0.75mm.
pub fn get_key_actuation(table: &[u8], key_code: usize) -> u16 {
    let off = key_code * ACT_BYTES_PER_KEY + ACT_FIELD_OFFSET;
    table.get(off..off + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .unwrap_or(0)
}

/// Set actuation value (hundredths of mm) for a key in the raw table.
pub fn set_key_actuation(table: &mut [u8], key_code: usize, hundredths: u16) {
    let off = key_code * ACT_BYTES_PER_KEY + ACT_FIELD_OFFSET;
    if off + 2 <= table.len() {
        let bytes = hundredths.to_le_bytes();
        table[off] = bytes[0];
        table[off + 1] = bytes[1];
    }
}

// # Per-key RGB color table (CMD 0x14 / 0x24)

const RGB_BYTES_PER_KEY: usize = 4;
const RGB_TABLE_SIZE: usize = 512; // 128 slots * 4 bytes

/// Read the per-key RGB color table (512 bytes from flash `0x9A00`).
///
/// Each key occupies 4 bytes: `[led_index, R, G, B]`.
///
/// ```text
/// TX: AA 14 38 00 00 00 00 00  ...
/// RX: 55 14 38 00 00 00 00 00
///     00 FF 00 00  01 00 00 00  02 00 00 00 ...
///     ^^ ^^^^^^^^
///     |  RGB for key 0 (Esc = red in this example)
///     LED index
/// ```
pub fn read_rgb_table(device: &HidDevice, transport: Transport) -> Result<Vec<u8>, DeviceError> {
    read_table(device, transport, 0x14, RGB_TABLE_SIZE)
}

/// Write the per-key RGB color table.
pub fn write_rgb_table(device: &HidDevice, transport: Transport, table: &[u8]) -> Result<(), DeviceError> {
    if table.len() < RGB_TABLE_SIZE {
        return Err(DeviceError::Protocol(format!("RGB table too short: {} < {RGB_TABLE_SIZE}", table.len())));
    }
    write_table(device, transport, 0x24, &table[..RGB_TABLE_SIZE])
}

/// Get color for a key from the raw RGB table.
pub fn get_key_color(table: &[u8], key_code: usize) -> KeyColor {
    let off = key_code * RGB_BYTES_PER_KEY;
    if off + 4 > table.len() { return KeyColor::default(); }
    KeyColor { r: table[off + 1], g: table[off + 2], b: table[off + 3] }
}

/// Set color for a key in the raw RGB table (preserves LED index at `[+0]`).
pub fn set_key_color(table: &mut [u8], key_code: usize, color: KeyColor) {
    let off = key_code * RGB_BYTES_PER_KEY;
    if off + 4 <= table.len() {
        table[off + 1] = color.r;
        table[off + 2] = color.g;
        table[off + 3] = color.b;
    }
}

// # General-purpose chunked command

/// Send a multi-chunk command and collect concatenated response data.
///
/// `header_data` fills bytes `[5..]` of the header (max 3 bytes).
/// For simple reads, pass an empty `data` slice with the desired `header_data`.
pub fn send_command(
    device: &HidDevice,
    command_type: u8,
    data: &[u8],
    header_data: &[u8],
    transport: Transport,
) -> Result<Vec<u8>, DeviceError> {
    if header_data.len() > 3 {
        return Err(DeviceError::Protocol("Header data may not exceed 3 bytes".into()));
    }

    let chunk_count = if data.is_empty() { 1 } else { (data.len() + DATA_SIZE - 1) / DATA_SIZE };
    let mut result = Vec::with_capacity(chunk_count * DATA_SIZE);

    for chunk in 0..chunk_count {
        let offset = chunk * DATA_SIZE;
        let len = if data.is_empty() { 0 } else { (data.len() - offset).min(DATA_SIZE) };

        let mut report = [0u8; REPORT_SIZE];
        report[0] = MAGIC_OUT;
        report[1] = command_type;
        report[2] = len as u8;
        report[3] = (offset & 0xFF) as u8;
        report[4] = ((offset >> 8) & 0xFF) as u8;
        for (i, &b) in header_data.iter().enumerate() {
            report[5 + i] = b;
        }
        if len > 0 {
            report[HEADER_SIZE..HEADER_SIZE + len].copy_from_slice(&data[offset..offset + len]);
        }

        let resp = exchange(device, &report, command_type, transport)?;
        result.extend_from_slice(&resp[HEADER_SIZE..]);
    }

    Ok(result)
}

// # Probe API (for ak680-probe binary)

/// Report size constant for external tools.
pub const WIRE_REPORT_SIZE: usize = REPORT_SIZE;
/// Outbound magic byte for external tools.
pub const WIRE_MAGIC_OUT: u8 = MAGIC_OUT;
/// Inbound magic byte for external tools.
pub const WIRE_MAGIC_IN: u8 = MAGIC_IN;

/// Send a raw report and wait for any response (no command validation).
///
/// Returns `None` on timeout. Intended for protocol exploration.
pub fn probe_command(
    device: &HidDevice,
    report: &[u8; REPORT_SIZE],
    transport: Transport,
    timeout_ms: i32,
) -> Result<Option<[u8; REPORT_SIZE]>, DeviceError> {
    send_raw(device, report, transport)?;

    let mut buf = [0u8; 512];
    buf[0] = 0x00;

    let n = match transport {
        Transport::OutputReport | Transport::MixedFeatureWrite => {
            device.read_timeout(&mut buf, timeout_ms)?
        }
        Transport::FeatureReport => device.get_feature_report(&mut buf)?,
    };

    if n == 0 { return Ok(None); }

    let start = match (buf[0], buf.get(1)) {
        (MAGIC_IN, _) => 0,
        (_, Some(&MAGIC_IN)) if n >= 2 => 1,
        _ => return Ok(None),
    };

    if start + REPORT_SIZE > n { return Ok(None); }

    let mut out = [0u8; REPORT_SIZE];
    out.copy_from_slice(&buf[start..start + REPORT_SIZE]);
    Ok(Some(out))
}

/// Drain pending input data from the device.
pub fn drain(device: &HidDevice, transport: Transport) {
    let mut buf = [0u8; 512];
    while matches!(transport,
        Transport::OutputReport | Transport::MixedFeatureWrite
    ) && device.read_timeout(&mut buf, 10).unwrap_or(0) > 0 {}
}