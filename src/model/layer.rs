/// Configuration layer index (0-3).
///
/// The AK680 MAX stores up to four independent configurations.
/// Layer switching is controlled by CMD `0x84` (read) / `0x04` (write)
/// on the lightless variant. The RGB variant reports the active layer
/// in the state register (CMD `0x13`, payload byte `[+8]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum Layer {
    Layer1 = 0,
    Layer2 = 1,
    Layer3 = 2,
    Layer4 = 3,
}

impl Layer {
    pub const ALL: [Layer; 4] = [Self::Layer1, Self::Layer2, Self::Layer3, Self::Layer4];

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Layer1 => "Layer 1",
            Self::Layer2 => "Layer 2",
            Self::Layer3 => "Layer 3",
            Self::Layer4 => "Layer 4",
        }
    }

    pub const fn from_byte(value: u8) -> Option<Layer> {
        match value {
            0 => Some(Self::Layer1),
            1 => Some(Self::Layer2),
            2 => Some(Self::Layer3),
            3 => Some(Self::Layer4),
            _ => None,
        }
    }
}