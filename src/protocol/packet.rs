/// Low-level packet construction for the lightless (no-RGB) variant.
///
/// Every report is 64 bytes. The first 8 bytes form a header where
/// byte 7 is a checksum: `0xFF - sum(header[0..7])`.
///
/// Example header for reading actuation (CMD `0xE5`, sub=`0x00`):
///
/// ```text
/// E5 00 01 00 00 00 00 1A
/// |  |  |  |           |
/// |  |  |  chunk=0     checksum = 0xFF - (0xE5+0x00+0x01) = 0x1A
/// |  |  direction=read
/// |  sub=press
/// cmd
/// ```

/// Size of every HID report exchanged with the keyboard.
pub const REPORT_SIZE: usize = 64;

/// Header occupies the first 8 bytes of each report.
pub const HEADER_SIZE: usize = 8;

/// Build the 8-byte header with automatic checksum.
///
/// `packet_type` is the command ID. `data` provides bytes 1-6 of the header.
/// Byte 7 is computed as `0xFF - (packet_type + sum(data))`.
///
/// ```text
/// [0] packet_type
/// [1] data[0]     (sub-command / direction)
/// [2] data[1]     (read=1 / write)
/// [3] data[2]     (chunk index)
/// [4] data[3]
/// [5] data[4]
/// [6] data[5]
/// [7] checksum
/// ```
pub fn build_header(packet_type: u8, data: [u8; 6]) -> [u8; HEADER_SIZE] {
    let sum: u16 = packet_type as u16 + data.iter().map(|&b| b as u16).sum::<u16>();
    let checksum = (0xFFu16.wrapping_sub(sum)) as u8;
    [
        packet_type,
        data[0], data[1], data[2], data[3], data[4], data[5],
        checksum,
    ]
}

/// Assemble a full 64-byte report from an 8-byte header and body payload.
///
/// Body is copied into bytes `[8..]`; excess is truncated, shortage is zero-filled.
pub fn build_packet(header: &[u8; HEADER_SIZE], body: &[u8]) -> [u8; REPORT_SIZE] {
    let mut buf = [0u8; REPORT_SIZE];
    buf[..HEADER_SIZE].copy_from_slice(header);
    let n = body.len().min(REPORT_SIZE - HEADER_SIZE);
    buf[HEADER_SIZE..HEADER_SIZE + n].copy_from_slice(&body[..n]);
    buf
}