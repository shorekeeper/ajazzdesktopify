/// Per-key RGB color (0-255 per channel).
///
/// Stored in flash at `0x9A00` as part of the per-key color table
/// (CMD `0x14`/`0x24`). Each key occupies 4 bytes:
///
/// ```text
/// [+0] u8  LED hardware index (= key_code, preserve on write)
/// [+1] u8  Red
/// [+2] u8  Green
/// [+3] u8  Blue
/// ```
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct KeyColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Configuration for a single physical key.
///
/// Combines actuation parameters (from CMD `0x17`/`0x18` tables)
/// and per-key color (from CMD `0x14` table). Actuation values are
/// in millimeters (converted from the wire format of hundredths-of-mm).
///
/// Wire format for actuation (8 bytes per key at `key_code * 8`):
///
/// ```text
/// [+0] u16 LE  unknown (always 0x0000 observed)
/// [+2] u16 LE  actuation depth (hundredths of mm, e.g. 0x004B = 0.75mm)
/// [+4] u16 LE  unknown (RT-related candidate)
/// [+6] u16 LE  unknown (RT-related candidate)
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Key {
    /// Firmware key index (sparse: 0, 17-28, 32-44, 48-60, 64-76, 80-92, 104-108).
    pub code: usize,

    /// Press actuation depth in mm (e.g. 0.75). Firmware default: ~1.20mm.
    pub down_actuation: f64,

    /// Release actuation depth in mm. Usually equal to `down_actuation`.
    pub up_actuation: f64,

    /// Whether rapid trigger is enabled for this key.
    pub rapid_trigger: bool,

    /// RT press sensitivity in mm (lightless only, fields +4/+6 on RGB).
    pub rt_press_sensitivity: f64,

    /// RT release sensitivity in mm.
    pub rt_release_sensitivity: f64,

    /// Per-key backlight color (RGB model only).
    pub color: KeyColor,
}

impl Key {
    pub fn new(code: usize) -> Self {
        Self {
            code,
            down_actuation: 0.0,
            up_actuation: 0.0,
            rapid_trigger: false,
            rt_press_sensitivity: 0.0,
            rt_release_sensitivity: 0.0,
            color: KeyColor::default(),
        }
    }
}