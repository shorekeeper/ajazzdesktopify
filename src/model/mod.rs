/// Pure data types shared between protocol, device, and UI layers.
///
/// No I/O, no HID calls - only `Clone`/`Copy`/`Serialize` structs
/// that describe keyboard state in memory.
pub mod key;
pub mod keyboard;
pub mod layer;