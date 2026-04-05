//! Protocol exploration and debugging tool for the AJAZZ AK680 MAX.
//!
//! Provides subcommands for scanning, probing, dumping firmware, and
//! testing per-key colors and LED effects without the full GUI.
//!
//! # Examples
//!
//! ```text
//! ak680-probe info                          # device info
//! ak680-probe scan 00 30                    # probe commands 0x00..0x30
//! ak680-probe read 17 00 19                 # read press actuation (19 chunks)
//! ak680-probe send AA10300000000100...      # send raw 64-byte report
//! ak680-probe dump 64 firmware.bin 17       # dump 64KB via CMD 0x17
//! ak680-probe fill-color FF0000             # all keys red, activate 0x14
//! ak680-probe effect 0x0C 5 3              # rainbow rotation
//! ak680-probe led-state                     # read current LED state
//! ```

use std::io::Write;
use std::time::{Duration, Instant};

use ak680max_driver::device::connection;
use ak680max_driver::protocol::rgb_commands::{
    self, Transport, EffectId, LedState,
    WIRE_MAGIC_OUT, WIRE_MAGIC_IN, WIRE_REPORT_SIZE,
};

const SCAN_TIMEOUT_MS: i32 = 150;
const SCAN_DELAY: Duration = Duration::from_millis(30);
const SEND_TIMEOUT_MS: i32 = 1000;

// # Utility functions

fn hex_dump(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len() * 3);
    for (i, b) in data.iter().enumerate() {
        if i > 0 && i % 8 == 0 {
            s.push_str("  ");
        } else if i > 0 {
            s.push(' ');
        }
        s.push_str(&format!("{b:02X}"));
    }
    s
}

fn parse_hex(hex: &str) -> Result<Vec<u8>, String> {
    let clean: String = hex.chars().filter(|c| !c.is_whitespace()).collect();
    if clean.len() % 2 != 0 {
        return Err("Hex string must have even length".into());
    }
    (0..clean.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&clean[i..i + 2], 16).map_err(|e| format!("Bad hex at {i}: {e}")))
        .collect()
}

fn parse_hex_byte(s: &str) -> Result<u8, String> {
    let clean = s.trim_start_matches("0x").trim_start_matches("0X");
    u8::from_str_radix(clean, 16).map_err(|e| format!("Bad hex byte '{s}': {e}"))
}

fn parse_int_or_hex(s: &str) -> Result<u8, String> {
    if s.starts_with("0x") || s.starts_with("0X") {
        parse_hex_byte(s)
    } else {
        s.parse::<u8>().map_err(|e| format!("Bad number '{s}': {e}"))
    }
}

fn build_report(cmd: u8, data_size: u8) -> [u8; WIRE_REPORT_SIZE] {
    let mut r = [0u8; WIRE_REPORT_SIZE];
    r[0] = WIRE_MAGIC_OUT;
    r[1] = cmd;
    r[2] = data_size;
    r
}

fn has_data(response: &[u8; WIRE_REPORT_SIZE]) -> bool {
    response[8..].iter().any(|&b| b != 0)
}

fn is_disconnect(e: &ak680max_driver::device::DeviceError) -> bool {
    let msg = format!("{e}");
    msg.contains("0x000001B1") || msg.contains("0x0000048F") || msg.contains("not connected")
}

fn open_device() -> Result<(hidapi::HidDevice, Transport, String), Box<dyn std::error::Error>> {
    let (device, state) = connection::connect()?;
    let transport = state.transport.unwrap_or(Transport::OutputReport);
    let name = state.config.name.to_string();
    Ok((device, transport, name))
}

fn print_response(r: &[u8]) {
    if r.len() < 8 {
        println!("(too short: {} bytes)", r.len());
        return;
    }
    println!("Breakdown:");
    println!("  [0]    Magic:   {:#04X}", r[0]);
    println!("  [1]    Command: {:#04X}", r[1]);
    println!("  [2]    Length:  {} ({:#04X})", r[2], r[2]);
    println!("  [3..4] Offset:  {:#06X}", r[3] as u16 | ((r[4] as u16) << 8));
    println!("  [5..7] Header:  {} {} {}", r[5], r[6], r[7]);
    println!("  [8..]  Data:");
    for (i, chunk) in r[8..].chunks(8).enumerate() {
        let off = 8 + i * 8;
        print!("    [{off:2}] ");
        for b in chunk { print!("{b:02X} "); }
        print!("  ");
        for b in chunk {
            print!("{}", if b.is_ascii_graphic() { *b as char } else { '.' });
        }
        println!();
    }
}

// # Subcommands

fn cmd_info() -> Result<(), Box<dyn std::error::Error>> {
    let (_, transport, name) = open_device()?;
    println!("Device:    {name}");
    println!("Transport: {transport}");
    let (_, state) = connection::connect()?;
    if let Some(ref info) = state.device_info {
        println!("Firmware:  {:.2}", info.firmware_version);
        println!("Battery:   {}%", info.battery_level);
        println!("Charge:    {}", info.charge_status);
        println!("RT Prec:   {}", info.rt_precision);
    }
    if let Some(led) = state.led_state {
        println!("LED:       {led}");
    }
    Ok(())
}

/// Probe a range of command IDs and report which respond.
///
/// ```text
/// ak680-probe scan 00 30
/// ```
///
/// Sends each command as a 64-byte report with magic `0xAA`, the command
/// byte, and 56 bytes of payload zeros. Reports that respond with non-zero
/// data are flagged.
///
/// WARNING: commands in the `0x50-0x64` range can freeze the keyboard.
fn cmd_scan(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (device, transport, name) = open_device()?;
    let start = args.first().map(|s| parse_hex_byte(s)).transpose()?.unwrap_or(0x00);
    let end = args.get(1).map(|s| parse_hex_byte(s)).transpose()?.unwrap_or(0x64);

    println!("Scan {name} [{transport}] range {start:#04X}..={end:#04X}");
    println!();

    let mut responsive = Vec::new();
    let mut data_cmds = Vec::new();
    let t0 = Instant::now();

    for cmd in start..=end {
        rgb_commands::drain(&device, transport);
        let report = build_report(cmd, 24);

        match rgb_commands::probe_command(&device, &report, transport, SCAN_TIMEOUT_MS) {
            Ok(Some(resp)) => {
                if has_data(&resp) {
                    println!("[{cmd:#04X}] << {} ***DATA***", hex_dump(&resp));
                    data_cmds.push((cmd, resp));
                } else {
                    print!(".");
                    std::io::stdout().flush().ok();
                }
                responsive.push((cmd, resp));
            }
            Ok(None) => { print!("_"); std::io::stdout().flush().ok(); }
            Err(e) => {
                if is_disconnect(&e) {
                    println!("\n!!! Disconnected at {cmd:#04X}");
                    break;
                }
                println!("[{cmd:#04X}] !! {e}");
            }
        }
        std::thread::sleep(SCAN_DELAY);
    }

    println!("\n\nDone ({:.1}s): {} responded, {} with data",
        t0.elapsed().as_secs_f64(), responsive.len(), data_cmds.len());

    if !data_cmds.is_empty() {
        println!("\nCommands with data:");
        for (cmd, resp) in &data_cmds {
            println!("  {cmd:#04X}: {}", hex_dump(resp));
        }
    }

    // Show silent ranges
    let resp_set: std::collections::HashSet<u8> = responsive.iter().map(|(c, _)| *c).collect();
    let silent: Vec<u8> = (start..=end).filter(|c| !resp_set.contains(c)).collect();
    if !silent.is_empty() {
        print!("Silent: ");
        let mut i = 0;
        while i < silent.len() {
            let rs = silent[i];
            while i + 1 < silent.len() && silent[i + 1] == silent[i] + 1 { i += 1; }
            let re = silent[i];
            if rs == re { print!("{rs:#04X} "); } else { print!("{rs:#04X}-{re:#04X} "); }
            i += 1;
        }
        println!();
    }

    Ok(())
}

/// Send a raw hex-encoded report and display the response.
///
/// ```text
/// ak680-probe send AA10300000000100
/// ```
fn cmd_send(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        println!("Usage: ak680-probe send <hex bytes>");
        println!("  Up to 64 hex bytes, zero-padded to 64.");
        return Ok(());
    }

    let (device, transport, name) = open_device()?;
    println!("Device: {name} [{transport}]");

    let bytes = parse_hex(&args.join(""))?;
    let mut report = [0u8; WIRE_REPORT_SIZE];
    let n = bytes.len().min(WIRE_REPORT_SIZE);
    report[..n].copy_from_slice(&bytes[..n]);

    println!("TX >> {}", hex_dump(&report));
    rgb_commands::drain(&device, transport);

    match rgb_commands::probe_command(&device, &report, transport, SEND_TIMEOUT_MS) {
        Ok(Some(resp)) => { println!("RX << {}", hex_dump(&resp)); println!(); print_response(&resp); }
        Ok(None)       => println!("RX << (timeout)"),
        Err(e)         => println!("RX !! ERROR: {e}"),
    }
    Ok(())
}

/// Sweep a single header byte across all 256 values for a given command.
///
/// ```text
/// ak680-probe deep 13 5    # sweep byte [5] (sub-command) for CMD 0x13
/// ```
fn cmd_deep(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        println!("Usage: ak680-probe deep <CMD> [header_pos=5]");
        return Ok(());
    }

    let cmd = parse_hex_byte(&args[0])?;
    let pos: usize = args.get(1).map(|s| s.parse()).transpose()?.unwrap_or(5);
    if !(3..=7).contains(&pos) {
        return Err(format!("Position must be 3-7, got {pos}").into());
    }

    let (device, transport, _) = open_device()?;
    println!("Deep probe cmd={cmd:#04X}, sweep byte [{pos}]");

    let mut found = 0u32;
    for val in 0..=0xFFu8 {
        let mut report = build_report(cmd, 24);
        report[pos] = val;
        rgb_commands::drain(&device, transport);

        match rgb_commands::probe_command(&device, &report, transport, SCAN_TIMEOUT_MS) {
            Ok(Some(resp)) if has_data(&resp) => {
                println!("[{val:#04X}] << {}", hex_dump(&resp));
                found += 1;
            }
            Err(e) if is_disconnect(&e) => {
                println!("!!! Disconnected at val={val:#04X}");
                break;
            }
            _ => {}
        }
        std::thread::sleep(SCAN_DELAY);
    }
    println!("\n{found} values returned data");
    Ok(())
}

/// Read multiple chunks from a command with incrementing offsets.
///
/// ```text
/// ak680-probe read 17 00 19    # 19 chunks of CMD 0x17, sub=0x00
/// ```
fn cmd_multi_read(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 2 {
        println!("Usage: ak680-probe read <CMD> <sub> [chunks=4]");
        return Ok(());
    }

    let cmd = parse_hex_byte(&args[0])?;
    let sub = parse_hex_byte(&args[1])?;
    let chunks: usize = args.get(2).map(|s| s.parse()).transpose()?.unwrap_or(4);

    let (device, transport, _) = open_device()?;
    println!("Reading cmd={cmd:#04X} sub={sub:#04X} chunks={chunks}");

    let mut all_data = Vec::new();
    for chunk in 0..chunks {
        let offset = chunk * 56;
        let mut report = build_report(cmd, 0x38);
        report[3] = (offset & 0xFF) as u8;
        report[4] = ((offset >> 8) & 0xFF) as u8;
        report[5] = sub;

        println!("Chunk {chunk}: TX >> {}", hex_dump(&report));
        rgb_commands::drain(&device, transport);

        match rgb_commands::probe_command(&device, &report, transport, SEND_TIMEOUT_MS) {
            Ok(Some(resp)) => {
                println!("         RX << {}", hex_dump(&resp));
                all_data.extend_from_slice(&resp[8..]);
            }
            Ok(None) => { println!("         RX << (timeout)"); break; }
            Err(e) => {
                println!("         RX !! {e}");
                if is_disconnect(&e) { break; }
            }
        }
        std::thread::sleep(SCAN_DELAY);
    }

    if !all_data.is_empty() {
        println!("\nCombined ({} bytes):", all_data.len());
        for (i, chunk) in all_data.chunks(16).enumerate() {
            let off = i * 16;
            print!("  {off:04X}: {:<48}  ", hex_dump(chunk));
            for b in chunk {
                print!("{}", if b.is_ascii_graphic() { *b as char } else { '.' });
            }
            println!();
        }
    }
    Ok(())
}

/// Poll a command repeatedly and display when data changes.
///
/// ```text
/// ak680-probe watch 13 300    # poll CMD 0x13 every 300ms
/// ```
fn cmd_watch(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let cmd = args.first().map(|s| parse_hex_byte(s)).transpose()?.unwrap_or(0x13);
    let interval: u64 = args.get(1).map(|s| s.parse()).transpose()?.unwrap_or(500);

    let (device, transport, _) = open_device()?;
    println!("Watching cmd={cmd:#04X} every {interval}ms (Ctrl+C to stop)\n");

    let report = build_report(cmd, 24);
    let mut last = [0u8; WIRE_REPORT_SIZE];
    let mut count = 0u64;

    loop {
        rgb_commands::drain(&device, transport);
        match rgb_commands::probe_command(&device, &report, transport, SEND_TIMEOUT_MS) {
            Ok(Some(resp)) if resp != last => {
                count += 1;
                println!("[#{count}] {}", hex_dump(&resp));
                last = resp;
            }
            Err(e) if is_disconnect(&e) => { println!("Disconnected"); break; }
            _ => {}
        }
        std::thread::sleep(Duration::from_millis(interval));
    }
    Ok(())
}

/// Dump firmware by exploiting the unbounded read in CMD 0x17.
///
/// ```text
/// ak680-probe dump 64 firmware.bin 17    # 64KB via CMD 0x17
/// ```
///
/// The firmware does not validate the read offset against the table size.
/// CMD 0x17's flash base is `0xB600`; requesting offset > `0x0400` reads
/// executable code. Maximum dump size: 64KB (16-bit offset limit).
fn cmd_dump(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let size_kb: usize = args.first().map(|s| s.parse()).transpose()?.unwrap_or(64);
    let output = args.get(1).map(|s| s.as_str()).unwrap_or("firmware.bin");
    let cmd = args.get(2).map(|s| parse_hex_byte(s)).transpose()?.unwrap_or(0x17);

    let total = size_kb * 1024;
    let chunks = (total + 55) / 56;

    println!("Dump CMD {cmd:#04X}: {size_kb}KB ({total} bytes, {chunks} chunks) -> {output}");

    let (device, transport, name) = open_device()?;
    println!("Device: {name} [{transport}]\n");

    let mut data = Vec::with_capacity(total);
    let t0 = Instant::now();

    for chunk in 0..chunks {
        let offset = chunk * 56;
        if offset > 0xFFFF {
            println!("Offset overflow at chunk {chunk}, stopping at 64KB");
            break;
        }

        let mut report = build_report(cmd, 0x38);
        report[3] = (offset & 0xFF) as u8;
        report[4] = ((offset >> 8) & 0xFF) as u8;

        rgb_commands::drain(&device, transport);
        match rgb_commands::probe_command(&device, &report, transport, SEND_TIMEOUT_MS) {
            Ok(Some(resp)) => data.extend_from_slice(&resp[8..]),
            Ok(None) => {
                println!("Timeout at {offset:#06X}, padding with 0xFF");
                data.extend_from_slice(&[0xFF; 56]);
            }
            Err(e) => {
                if is_disconnect(&e) { println!("Disconnected at {offset:#06X}"); break; }
                println!("Error at {offset:#06X}: {e}, padding");
                data.extend_from_slice(&[0xFF; 56]);
            }
        }

        if chunk % 64 == 0 || chunk == chunks - 1 {
            let pct = (chunk + 1) as f64 / chunks as f64 * 100.0;
            let speed = data.len() as f64 / t0.elapsed().as_secs_f64() / 1024.0;
            print!("\r  [{pct:5.1}%] {:#06X} / {:#06X} ({speed:.1} KB/s)  ", data.len(), total);
            std::io::stdout().flush().ok();
        }
    }

    data.truncate(total);
    println!("\n\nDumped {} bytes in {:.1}s", data.len(), t0.elapsed().as_secs_f64());

    std::fs::write(output, &data)?;
    println!("Saved to {output}");

    // Quick analysis
    let non_zero = data.iter().filter(|&&b| b != 0).count();
    let non_ff = data.iter().filter(|&&b| b != 0xFF).count();
    println!("\nAnalysis:");
    println!("  Non-zero: {non_zero} ({:.1}%)", non_zero as f64 / data.len() as f64 * 100.0);
    println!("  Non-0xFF: {non_ff} ({:.1}%)", non_ff as f64 / data.len() as f64 * 100.0);

    // Strings
    println!("\nStrings:");
    let mut i = 0;
    while i < data.len() {
        if data[i].is_ascii_graphic() || data[i] == b' ' {
            let start = i;
            while i < data.len() && (data[i].is_ascii_graphic() || data[i] == b' ') { i += 1; }
            if i - start >= 4 {
                let s: String = data[start..i].iter().map(|&b| b as char).collect();
                println!("  {start:#06X}: \"{s}\"");
            }
        } else {
            i += 1;
        }
    }
    Ok(())
}

/// Set per-key colors and activate Custom Per-Key mode (effect 0x14).
///
/// ```text
/// ak680-probe fill-color FF0000             # all keys red
/// ak680-probe fill-color 00FF00 0,33,83     # specific keys green
/// ```
///
/// Reads the current RGB table (CMD 0x14), modifies the specified keys,
/// writes it back (CMD 0x24), then switches to effect 0x14 via CMD 0x23.
fn cmd_fill_color(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        println!("Usage: ak680-probe fill-color <RRGGBB> [key_indices]");
        println!("  All keys:     fill-color FF0000");
        println!("  Specific:     fill-color 00FF00 0,33,83");
        return Ok(());
    }

    let hex = args[0].trim_start_matches('#');
    if hex.len() != 6 { return Err("Need 6 hex digits (RRGGBB)".into()); }
    let r = u8::from_str_radix(&hex[0..2], 16)?;
    let g = u8::from_str_radix(&hex[2..4], 16)?;
    let b = u8::from_str_radix(&hex[4..6], 16)?;

    let (device, transport, name) = open_device()?;
    println!("Device: {name} [{transport}]");
    println!("Color:  R={r} G={g} B={b}");

    let mut table = rgb_commands::read_rgb_table(&device, transport)?;

    let key_list = ak680max_driver::protocol::key_list::ak680_max_key_list();
    let targets: Vec<usize> = if args.len() > 1 {
        args[1].split(',').map(|s| s.trim().parse::<usize>()).collect::<Result<_, _>>()?
    } else {
        key_list.iter().enumerate()
            .filter_map(|(i, e)| e.is_some().then_some(i))
            .collect()
    };

    println!("Setting {} key(s)...", targets.len());
    for &kc in &targets {
        rgb_commands::set_key_color(&mut table, kc, rgb_commands::KeyColor::new(r, g, b));
    }

    rgb_commands::write_rgb_table(&device, transport, &table)?;

    // Switch to Custom Per-Key mode, preserving brightness
    let cur = rgb_commands::get_led_state(&device, transport)?;
    let br = if cur.brightness >= 1 { cur.brightness } else { 5 };
    let led = LedState::custom_colors(br);
    rgb_commands::set_led_state(&device, transport, &led)?;

    println!("Done! {led}");
    Ok(())
}

/// Set the LED effect, brightness, and speed.
///
/// ```text
/// ak680-probe effect 0          # off
/// ak680-probe effect 0x0C 5 3   # rainbow rotation, bright=5, speed=3
/// ak680-probe effect 0x14 5     # custom per-key colors
/// ```
///
/// Effect IDs:
///
/// ```text
/// 0x00  Off               0x01  Solid Color       0x02  Keypress Light
/// 0x03  Breathing          0x04  Starfall          0x05  Rain
/// 0x06  Rainbow Shimmer    0x07  Fade              0x08  Rainbow Wave
/// 0x09  Center Waves       0x0A  Top-Down Wave     0x0B  Color Pulse Wave
/// 0x0C  Rainbow Rotation   0x0D  Row Flash         0x0E  Ripple Horizontal
/// 0x0F  Ripple Radial      0x10  Scanner           0x11  Center Pulse
/// 0x12  Shore Waves        0x13  Row Diverge       0x14  Custom Per-Key
/// ```
fn cmd_effect(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        println!("Usage: ak680-probe effect <id> [brightness=5] [speed=3]");
        println!();
        for e in EffectId::ALL {
            let tag = match e {
                EffectId::Off => " off",
                EffectId::CustomColors => "cust",
                _ if e.needs_engine() => "anim",
                _ => "stat",
            };
            println!("  {:#04X} [{tag}]  {}", e as u8, e.name());
        }
        return Ok(());
    }

    let id_byte = parse_int_or_hex(&args[0])?;
    let effect = EffectId::from_byte(id_byte)
        .ok_or_else(|| format!("Unknown effect: {id_byte:#04X}"))?;
    let brightness: u8 = args.get(1).map(|s| s.parse()).transpose()?.unwrap_or(5);
    let speed: u8 = args.get(2).map(|s| s.parse()).transpose()?.unwrap_or(3);

    let (device, transport, name) = open_device()?;
    println!("Device: {name} [{transport}]");

    let state = LedState::new(effect, brightness, speed);
    println!("Setting: {state}");
    rgb_commands::set_led_state(&device, transport, &state)?;

    let actual = rgb_commands::get_led_state(&device, transport)?;
    println!("Read back: {actual}");
    Ok(())
}

/// Read and display the current LED state register.
fn cmd_led_state() -> Result<(), Box<dyn std::error::Error>> {
    let (device, transport, name) = open_device()?;
    println!("Device: {name} [{transport}]");
    let state = rgb_commands::get_led_state(&device, transport)?;
    println!("LED: {state}");
    Ok(())
}

/// Test cross-interface communication (send on iface 3, read on iface 2).
///
/// This was used to verify that interfaces are isolated. The firmware does
/// not bridge between HID interfaces.
#[cfg(windows)]
fn cmd_cross(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        println!("Usage: ak680-probe cross <hex>");
        return Ok(());
    }

    let api = hidapi::HidApi::new()?;
    let (vid, pid) = (0x0C45u16, 0x80B2u16);

    let reader = api.device_list()
        .find(|d| d.vendor_id() == vid && d.product_id() == pid && d.usage_page() == 0xFF68)
        .ok_or("Interface 2 (0xFF68) not found")?
        .open_device(&api)?;

    let writer = api.device_list()
        .find(|d| d.vendor_id() == vid && d.product_id() == pid && d.usage_page() == 0xFF67)
        .ok_or("Interface 3 (0xFF67) not found")?
        .open_device(&api)?;

    println!("Cross-interface: send on iface 3, read on iface 2\n");

    let bytes = parse_hex(&args.join(""))?;
    let mut report = [0u8; 64];
    let n = bytes.len().min(64);
    report[..n].copy_from_slice(&bytes[..n]);

    // Drain iface 2
    { let mut buf = [0u8; 256]; while reader.read_timeout(&mut buf, 10).unwrap_or(0) > 0 {} }

    println!("TX (iface 3) >> {}", hex_dump(&report));
    let mut send_buf = [0u8; 65];
    send_buf[0] = 0x00;
    send_buf[1..].copy_from_slice(&report);
    writer.send_feature_report(&send_buf)?;

    let mut recv_buf = [0u8; 256];
    let n = reader.read_timeout(&mut recv_buf, 2000)?;
    if n == 0 {
        println!("RX (iface 2) << (timeout)");
    } else {
        let start = if recv_buf[0] == 0x55 { 0 } else if n >= 2 && recv_buf[1] == 0x55 { 1 } else { 0 };
        let take = (n - start).min(64);
        println!("RX (iface 2) << {}", hex_dump(&recv_buf[start..start + take]));
    }
    Ok(())
}

/// Scan commands via cross-interface (iface 3 send, iface 2 read).
fn cmd_cross_scan(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let start = args.first().map(|s| parse_hex_byte(s)).transpose()?.unwrap_or(0x00);
    let end = args.get(1).map(|s| parse_hex_byte(s)).transpose()?.unwrap_or(0x30);

    let api = hidapi::HidApi::new()?;
    let (vid, pid) = (0x0C45u16, 0x80B2u16);

    let reader = api.device_list()
        .find(|d| d.vendor_id() == vid && d.product_id() == pid && d.usage_page() == 0xFF68)
        .ok_or("Interface 2 not found")?.open_device(&api)?;
    let writer = api.device_list()
        .find(|d| d.vendor_id() == vid && d.product_id() == pid && d.usage_page() == 0xFF67)
        .ok_or("Interface 3 not found")?.open_device(&api)?;

    println!("Cross-scan {start:#04X}..={end:#04X}\n");
    let mut responded = 0u32;

    for cmd in start..=end {
        { let mut buf = [0u8; 256]; while reader.read_timeout(&mut buf, 10).unwrap_or(0) > 0 {} }

        let mut report = [0u8; 64];
        report[0] = 0xAA;
        report[1] = cmd;
        report[2] = 0x38;

        let mut send_buf = [0u8; 65];
        send_buf[0] = 0x00;
        send_buf[1..].copy_from_slice(&report);

        if writer.send_feature_report(&send_buf).is_err() { continue; }

        let mut recv_buf = [0u8; 256];
        let n = reader.read_timeout(&mut recv_buf, 200).unwrap_or(0);
        if n >= 64 {
            let s = if recv_buf[0] == WIRE_MAGIC_IN { 0 } else if recv_buf[1] == WIRE_MAGIC_IN { 1 } else { 0 };
            if s + 64 <= n {
                let hd = recv_buf[s + 8..s + 32].iter().any(|&b| b != 0);
                if hd {
                    println!("[{cmd:#04X}] << {} ***DATA***", hex_dump(&recv_buf[s..s + 64]));
                } else {
                    print!(".");
                    std::io::stdout().flush().ok();
                }
                responded += 1;
            }
        } else {
            print!("_");
            std::io::stdout().flush().ok();
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    println!("\n\n{responded} responded");
    Ok(())
}

/// Raw feature report exchange on interface 3 (0xFF67).
///
/// Interface 3 has a 65-byte feature report buffer that the firmware
/// does not process -- data written via SetFeature is echoed by GetFeature.
#[cfg(windows)]
fn cmd_raw(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use ak680max_driver::protocol::win_hid::{find_device_path, WinHidDevice};

    let path = find_device_path(0x0C45, 0x80B2, 0xFF67)?;
    println!("Opening iface 3 (0xFF67): {path}");
    let dev = WinHidDevice::open(&path)?;

    let caps = dev.get_caps()?;
    println!("Caps: input={}B output={}B feature={}B\n",
        caps.input_report_byte_length, caps.output_report_byte_length, caps.feature_report_byte_length);

    if args.is_empty() {
        println!("Usage: ak680-probe raw <hex>");
        return Ok(());
    }

    let bytes = parse_hex(&args.join(""))?;
    let feat_size = caps.feature_report_byte_length as usize;
    let data_size = feat_size - 1;
    let mut data = vec![0u8; data_size];
    let n = bytes.len().min(data_size);
    data[..n].copy_from_slice(&bytes[..n]);

    println!("TX >> {}", hex_dump(&data[..data.len().min(32)]));

    let mut send_buf = vec![0u8; feat_size];
    send_buf[0] = 0x00;
    send_buf[1..].copy_from_slice(&data);

    let ok = unsafe {
        windows_sys::Win32::Devices::HumanInterfaceDevice::HidD_SetFeature(
            dev.handle_raw(), send_buf.as_ptr() as _, feat_size as u32,
        )
    };
    if ok == 0 {
        unsafe extern "system" { fn GetLastError() -> u32; }
        let err = unsafe { GetLastError() };
        return Err(format!("HidD_SetFeature failed ({err:#010X})").into());
    }

    std::thread::sleep(Duration::from_millis(50));

    let mut recv_buf = vec![0u8; feat_size];
    recv_buf[0] = 0x00;
    let ok = unsafe {
        windows_sys::Win32::Devices::HumanInterfaceDevice::HidD_GetFeature(
            dev.handle_raw(), recv_buf.as_mut_ptr() as _, feat_size as u32,
        )
    };
    if ok == 0 {
        unsafe extern "system" { fn GetLastError() -> u32; }
        let err = unsafe { GetLastError() };
        println!("RX !! HidD_GetFeature failed ({err:#010X})");
    } else {
        let resp = &recv_buf[1..];
        println!("RX << {}", hex_dump(&resp[..resp.len().min(32)]));
        match resp[0] {
            0x55 => println!("  Magic 0x55 -> real response"),
            0xAA => println!("  Echo (data returned unchanged)"),
            b    => println!("  Unknown first byte: {b:#04X}"),
        }
    }
    Ok(())
}

/// Scan via HidD_SetOutputReport on interface 3.
#[cfg(windows)]
fn cmd_raw_scan(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use ak680max_driver::protocol::win_hid::{find_device_path, WinHidDevice};

    let start = args.first().map(|s| parse_hex_byte(s)).transpose()?.unwrap_or(0x00);
    let end = args.get(1).map(|s| parse_hex_byte(s)).transpose()?.unwrap_or(0x30);

    let path = find_device_path(0x0C45, 0x80B2, 0xFF67)?;
    let dev = WinHidDevice::open(&path)?;
    println!("Raw scan iface 3: {start:#04X}..={end:#04X}\n");

    let mut data_cmds = Vec::new();
    for cmd in start..=end {
        let mut report = [0u8; 64];
        report[0] = 0xAA;
        report[1] = cmd;
        report[2] = 24;

        if dev.set_output_report(&report).is_err() { continue; }
        std::thread::sleep(Duration::from_millis(30));

        match dev.get_input_report() {
            Ok(resp) if resp[0] == 0x55 && resp[1] == cmd => {
                if resp[8..32].iter().any(|&b| b != 0) {
                    println!("[{cmd:#04X}] << {} ***DATA***", hex_dump(&resp[..32]));
                    data_cmds.push(cmd);
                } else {
                    print!(".");
                    std::io::stdout().flush().ok();
                }
            }
            _ => { print!("_"); std::io::stdout().flush().ok(); }
        }
    }
    println!("\n\n{} with data: {:02X?}", data_cmds.len(), data_cmds);
    Ok(())
}

/// Send a 4097-byte output report on interface 3 (firmware update channel).
#[cfg(windows)]
fn cmd_bulk(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use ak680max_driver::protocol::win_hid::{find_device_path, WinHidDevice};

    let path = find_device_path(0x0C45, 0x80B2, 0xFF67)?;
    let dev = WinHidDevice::open(&path)?;
    let caps = dev.get_caps()?;
    let out_size = caps.output_report_byte_length as usize;
    let feat_size = caps.feature_report_byte_length as usize;
    println!("Output: {out_size}B, Feature: {feat_size}B");

    let cmd_byte = args.first().map(|s| parse_hex_byte(s)).transpose()?.unwrap_or(0x10);

    let mut payload = vec![0u8; out_size];
    payload[0] = 0x00;
    payload[1] = 0xAA;
    payload[2] = cmd_byte;
    payload[3] = 0x18;

    println!("Sending {out_size}B output report (cmd={cmd_byte:#04X})...");

    let ok = unsafe {
        windows_sys::Win32::Devices::HumanInterfaceDevice::HidD_SetOutputReport(
            dev.handle_raw(), payload.as_ptr() as _, out_size as u32,
        )
    };
    if ok == 0 {
        unsafe extern "system" { fn GetLastError() -> u32; }
        println!("Failed: {:#010X}", unsafe { GetLastError() });
    } else {
        println!("Sent OK");
    }

    std::thread::sleep(Duration::from_millis(100));
    let mut recv_buf = vec![0u8; feat_size];
    recv_buf[0] = 0x00;
    let ok = unsafe {
        windows_sys::Win32::Devices::HumanInterfaceDevice::HidD_GetFeature(
            dev.handle_raw(), recv_buf.as_mut_ptr() as _, feat_size as u32,
        )
    };
    if ok != 0 {
        println!("Feature after bulk: {}", hex_dump(&recv_buf[1..recv_buf.len().min(33)]));
    }

    // Check iface 2
    let api = hidapi::HidApi::new()?;
    if let Some(info) = api.device_list().find(|d|
        d.vendor_id() == 0x0C45 && d.product_id() == 0x80B2 && d.usage_page() == 0xFF68
    ) {
        if let Ok(reader) = info.open_device(&api) {
            let mut buf = [0u8; 512];
            let n = reader.read_timeout(&mut buf, 500)?;
            if n > 0 { println!("iface 2: {n} bytes: {}", hex_dump(&buf[..n.min(32)])); }
            else { println!("iface 2: nothing"); }
        }
    }
    Ok(())
}

// # Help and entry point

fn print_help() {
    println!("ak680-probe - Protocol exploration for AJAZZ AK680 MAX\n");
    println!("General:");
    println!("  info                            Device info + LED state");
    println!("  led-state                       Read current LED state");
    println!("  effect <id> [bright] [speed]    Set LED effect (0=off, 0x14=custom)");
    println!("  fill-color <RRGGBB> [keys]      Set per-key colors + activate");
    println!();
    println!("Protocol exploration:");
    println!("  scan [start] [end]              Probe command range (default 00-64)");
    println!("  send <hex>                      Send raw 64-byte report");
    println!("  deep <cmd> [pos]                Sweep header byte (default pos=5)");
    println!("  read <cmd> <sub> [chunks]       Multi-chunk read (default 4)");
    println!("  watch <cmd> [interval_ms]       Poll and show changes");
    println!("  dump <kb> [file] [cmd]          Firmware dump (default 64KB)");
    println!();
    println!("Cross-interface (diagnostic):");
    println!("  cross <hex>                     Send iface 3, read iface 2");
    println!("  cross-scan [start] [end]        Cross-interface scan");
    #[cfg(windows)] {
    println!("  raw <hex>                       Feature report on iface 3");
    println!("  raw-scan [start] [end]          Output report scan on iface 3");
    println!("  bulk [cmd]                      4KB output report on iface 3");
    }
    println!();
    println!("DANGER: commands 0x40-0x64 may freeze the keyboard (replug to recover)");
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("help");
    let rest = if args.len() > 2 { &args[2..] } else { &[] };

    let result = match cmd {
        "info"       => cmd_info(),
        "scan"       => cmd_scan(rest),
        "send"       => cmd_send(rest),
        "deep"       => cmd_deep(rest),
        "read"       => cmd_multi_read(rest),
        "watch"      => cmd_watch(rest),
        "dump"       => cmd_dump(rest),
        "fill-color" => cmd_fill_color(rest),
        "effect"     => cmd_effect(rest),
        "led-state"  => cmd_led_state(),
        "cross"      => cmd_cross(rest),
        "cross-scan" => cmd_cross_scan(rest),
        #[cfg(windows)]
        "raw"        => cmd_raw(rest),
        #[cfg(windows)]
        "raw-scan"   => cmd_raw_scan(rest),
        #[cfg(windows)]
        "bulk"       => cmd_bulk(rest),
        _ => { print_help(); Ok(()) }
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}