/// Device layer: HID discovery, connection lifecycle, error types.

pub mod connection;

/// Errors from keyboard communication.
#[derive(Debug, thiserror::Error)]
pub enum DeviceError {
    #[error("HID error: {0}")]
    Hid(#[from] hidapi::HidError),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("{0}")]
    NotFound(String),
}