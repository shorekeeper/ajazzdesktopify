/// HID communication protocol for the AJAZZ AK680 MAX keyboard family.
///
/// # Two variants
///
/// | Property       | Lightless (no-RGB)       | RGB                        |
/// |----------------|--------------------------|----------------------------|
/// | VID / PID      | `0x3151` / `0x502C`      | `0x0C45` / `0x80B2`        |
/// | HID interface  | Feature reports (`0xFFFF`)| Interrupt reports (`0xFF68`)|
/// | Report size    | 64 bytes                 | 64 bytes                   |
/// | Header         | 8 bytes, last = checksum | 8 bytes, no checksum       |
/// | Read commands  | `0xE5` + sub-commands    | `0x1X` (X = table ID)      |
/// | Write commands | `0x65` + sub-commands    | `0x2X` (X = table ID)      |
///
/// # RGB wire format
///
/// Every report is 64 bytes. Magic `0xAA` outbound, `0x55` inbound:
///
/// ```text
/// [0] Magic     0xAA (TX) / 0x55 (RX)
/// [1] Command   operation ID
/// [2] Length    payload bytes this chunk (max 56)
/// [3] Offset   low byte of 16-bit offset
/// [4] Offset   high byte
/// [5] Sub-cmd  command-specific
/// [6] Flags    0x01 on last chunk of a write
/// [7] Reserved 0x00
/// [8..63] Payload (up to 56 bytes)
/// ```
///
/// Flash memory map:
///
/// ```text
/// 0x9000  DeviceInfo      (CMD 0x10)
/// 0x9200  Device config   (CMD 0x11/0x21)
/// 0x9600  Key mapping     (CMD 0x12/0x22)
/// 0x9A00  Per-key RGB     (CMD 0x14/0x24)  512 bytes
/// 0x9C00  LED animation   (CMD 0x15/0x25)  ~4608 bytes
/// 0xB000  Unknown         (CMD 0x16/0x26)
/// 0xB200  Release actuation (CMD 0x18/0x28)
/// 0xB600  Press actuation (CMD 0x17/0x27)  1024 bytes
/// ```
pub mod commands;
pub mod key_list;
pub mod layout;
pub mod packet;
pub mod rgb_commands;

#[cfg(windows)]
pub mod win_hid;