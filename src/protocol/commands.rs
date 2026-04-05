/// High-level HID transactions for the lightless (no-RGB) variant.
///
/// Communication uses HID **feature reports** (not interrupt).
/// Every outgoing report is 64 bytes with an 8-byte checksummed header.
///
/// # Actuation read protocol
///
/// Actuation data is read via CMD `0xE5` with direction sub-commands:
///
/// ```text
/// Sub 0x00 = press actuation depths     (4 chunks x 32 keys = 128 u16 LE values)
/// Sub 0x01 = release actuation depths
/// Sub 0x02 = RT press sensitivity
/// Sub 0x03 = RT release sensitivity
/// Sub 0x07 = rapid trigger on/off flags (2 chunks x 64 bytes, 0x80 = on)
/// ```
///
/// # Actuation write protocol
///
/// Written via CMD `0x65` with matching sub-commands. After all chunks for
/// a direction, an end-marker packet is sent:
///
/// ```text
/// TX: 65 [dir] 01 04 [dir] 00 00 [checksum]  [56 zero bytes]
/// ```

use hidapi::HidDevice;

use crate::model::key::Key;
use crate::model::layer::Layer;
use crate::protocol::packet::{build_header, build_packet, REPORT_SIZE};

const ACTUATION_CHUNK_COUNT: usize = 4;

#[derive(Clone, Copy)]
enum Direction {
    Press,
    Release,
}

impl Direction {
    const fn actuation_sub(self) -> u8 {
        match self { Self::Press => 0x00, Self::Release => 0x01 }
    }

    const fn rt_sensitivity_sub(self) -> u8 {
        match self { Self::Press => 0x02, Self::Release => 0x03 }
    }
}

/// Send a feature report (prepend report ID 0x00 for Windows HID).
fn send(device: &HidDevice, payload: &[u8; REPORT_SIZE]) -> Result<(), hidapi::HidError> {
    let mut buf = [0u8; REPORT_SIZE + 1];
    buf[0] = 0x00;
    buf[1..].copy_from_slice(payload);
    device.send_feature_report(&buf)?;
    log::trace!("TX feature: {:02X?}", &payload[..8]);
    Ok(())
}

/// Receive a feature report and return the 64-byte payload (no report ID).
fn receive(device: &HidDevice) -> Result<[u8; REPORT_SIZE], hidapi::HidError> {
    let mut buf = [0u8; REPORT_SIZE + 1];
    buf[0] = 0x00;
    device.get_feature_report(&mut buf)?;
    let mut out = [0u8; REPORT_SIZE];
    out.copy_from_slice(&buf[1..=REPORT_SIZE]);
    log::trace!("RX feature: {:02X?}", &out[..8]);
    Ok(out)
}

/// Query the active layer index.
///
/// ```text
/// TX: 84 00 00 00 00 00 00 7B  [56 zero bytes]
/// RX: [layer_byte] ...
/// ```
pub fn get_active_layer(device: &HidDevice) -> Result<Layer, crate::device::DeviceError> {
    let pkt = build_packet(&build_header(0x84, [0; 6]), &[]);
    send(device, &pkt)?;
    let resp = receive(device)?;
    Layer::from_byte(resp[1]).ok_or_else(|| {
        crate::device::DeviceError::Protocol(format!("Invalid layer byte: {}", resp[1]))
    })
}

/// Switch to a different layer.
///
/// ```text
/// TX: 04 [layer] 00 00 00 00 00 [checksum]  [56 zero bytes]
/// ```
pub fn set_active_layer(device: &HidDevice, layer: Layer) -> Result<(), hidapi::HidError> {
    let pkt = build_packet(&build_header(0x04, [layer as u8, 0, 0, 0, 0, 0]), &[]);
    send(device, &pkt)
}

/// Read all key settings from the active layer.
///
/// Returns a sparse vector indexed by firmware key code. Slots for
/// non-existent keys are `None`.
pub fn get_keys(
    device: &HidDevice,
    key_list: &[Option<&'static str>],
) -> Result<Vec<Option<Key>>, crate::device::DeviceError> {
    let mut keys: Vec<Option<Key>> = key_list
        .iter()
        .enumerate()
        .map(|(idx, entry)| entry.map(|_| Key::new(idx)))
        .collect();

    // Rapid trigger on/off: 2 chunks of 64 bytes each, 0x80 = enabled
    for chunk in 0..2u8 {
        let pkt = build_packet(&build_header(0xE5, [0x07, 1, chunk, 0, 0, 0]), &[]);
        send(device, &pkt)?;
        let resp = receive(device)?;
        for i in 0..REPORT_SIZE {
            let code = chunk as usize * 64 + i;
            if let Some(Some(key)) = keys.get_mut(code) {
                key.rapid_trigger = resp[i] == 0x80;
            }
        }
    }

    // Actuation depths and RT sensitivities
    for dir in [Direction::Press, Direction::Release] {
        for chunk in 0..ACTUATION_CHUNK_COUNT as u8 {
            // Actuation depth (u16 LE per key, 32 keys per chunk)
            {
                let pkt = build_packet(
                    &build_header(0xE5, [dir.actuation_sub(), 1, chunk, 0, 0, 0]),
                    &[],
                );
                send(device, &pkt)?;
                let resp = receive(device)?;
                for i in 0..(REPORT_SIZE / 2) {
                    let code = chunk as usize * 32 + i;
                    let raw = u16::from_le_bytes([resp[i * 2], resp[i * 2 + 1]]);
                    if let Some(Some(key)) = keys.get_mut(code) {
                        let mm = raw as f64 / 100.0;
                        match dir {
                            Direction::Press => key.down_actuation = mm,
                            Direction::Release => key.up_actuation = mm,
                        }
                    }
                }
            }

            // RT sensitivity (same u16 LE format)
            {
                let pkt = build_packet(
                    &build_header(0xE5, [dir.rt_sensitivity_sub(), 1, chunk, 0, 0, 0]),
                    &[],
                );
                send(device, &pkt)?;
                let resp = receive(device)?;
                for i in 0..(REPORT_SIZE / 2) {
                    let code = chunk as usize * 32 + i;
                    let raw = u16::from_le_bytes([resp[i * 2], resp[i * 2 + 1]]);
                    if let Some(Some(key)) = keys.get_mut(code) {
                        let mm = raw as f64 / 100.0;
                        match dir {
                            Direction::Press => key.rt_press_sensitivity = mm,
                            Direction::Release => key.rt_release_sensitivity = mm,
                        }
                    }
                }
            }
        }
    }

    Ok(keys)
}

/// Write all key settings to the active layer.
///
/// Sends RT on/off flags, actuation depths, and RT sensitivities
/// in 56-key chunks with end markers.
pub fn apply_keys(
    device: &HidDevice,
    keys: &[Option<Key>],
) -> Result<(), crate::device::DeviceError> {
    // RT on/off: 2 chunks of 56 keys
    for chunk in 0..2u8 {
        let start = chunk as usize * 56;
        let end = (start + 56).min(keys.len());
        let body: Vec<u8> = keys[start..end]
            .iter()
            .map(|k| if k.as_ref().map_or(false, |k| k.rapid_trigger) { 0x80 } else { 0x00 })
            .collect();
        let pkt = build_packet(&build_header(0x65, [0x07, 1, chunk, 0, 0, 0]), &body);
        send(device, &pkt)?;
    }

    // Actuation and RT sensitivity: 4 chunks of 28 keys per direction
    for dir in [Direction::Press, Direction::Release] {
        for chunk in 0..ACTUATION_CHUNK_COUNT as u8 {
            let start = chunk as usize * 28;
            let end = (start + 28).min(keys.len());
            let slice = &keys[start..end];

            // Actuation depth
            let body: Vec<u8> = slice
                .iter()
                .flat_map(|k| {
                    let val = k.as_ref().map_or(0u16, |k| match dir {
                        Direction::Press => (k.down_actuation * 100.0).round() as u16,
                        Direction::Release => (k.up_actuation * 100.0).round() as u16,
                    });
                    val.to_le_bytes()
                })
                .collect();
            let pkt = build_packet(
                &build_header(0x65, [dir.actuation_sub(), 1, chunk, 0, 0, 0]),
                &body,
            );
            send(device, &pkt)?;

            // End marker
            let d = dir.actuation_sub();
            let end_pkt = build_packet(&build_header(0x65, [d, 0x01, 4, d, 0, 0]), &[]);
            send(device, &end_pkt)?;

            // RT sensitivity
            let rt_body: Vec<u8> = slice
                .iter()
                .flat_map(|k| {
                    let val = k.as_ref().map_or(0u16, |k| match dir {
                        Direction::Press => (k.rt_press_sensitivity * 100.0).round() as u16,
                        Direction::Release => (k.rt_release_sensitivity * 100.0).round() as u16,
                    });
                    val.to_le_bytes()
                })
                .collect();
            let rt_pkt = build_packet(
                &build_header(0x65, [dir.rt_sensitivity_sub(), 1, chunk, 0, 0, 0]),
                &rt_body,
            );
            send(device, &rt_pkt)?;
        }
    }

    Ok(())
}