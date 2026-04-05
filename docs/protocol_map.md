# AK680 MAX RGB Protocol Quick Reference

## Wire Format (64-byte interrupt reports, interface 0xFF68)

```
[0] Magic:    0xAA (out) / 0x55 (in)
[1] Command:  operation ID
[2] Length:   payload bytes this chunk (max 0x38 = 56)
[3] Offset_lo
[4] Offset_hi
[5] Sub-cmd
[6] Flags:    0x01 = last chunk
[7] Reserved
[8..63] Payload (56 bytes max)
```

## Command Table

| Read | Write | Flash   | Content           | Size   | Format          |
|------|-------|---------|-------------------|--------|-----------------|
| 0x10 | --    | 0x9000  | DeviceInfo        | 48B    | special         |
| 0x11 | 0x21  | 0x9200  | Device config     | ~512B  | mixed           |
| 0x12 | 0x22  | 0x9600  | Key mapping       | ~512B  | HID keycodes    |
| 0x13 | --    | SRAM    | State register    | 16B    | computed        |
| 0x14 | 0x24  | 0x9A00  | Per-key RGB       | 512B   | 4B/key          |
| 0x15 | 0x25  | 0x9C00  | LED anim / RT     | ~4608B | unclear         |
| 0x16 | 0x26  | 0xB000  | Unknown           | ~512B  | --              |
| 0x17 | 0x27  | 0xB600  | Press actuation   | 1024B  | 8B/key          |
| 0x18 | 0x28  | 0xB200  | Release actuation | 1024B  | 8B/key (@+0x400)|
| --   | 0x64  | --      | Flash commit      | --     | system          |
| --   | 0x65  | --      | Enter update mode | --     | system          |
| --   | 0x66  | --      | Exit update mode  | --     | system          |

## Data Formats

### Actuation (CMD 0x17/0x27, 0x18/0x28)
```
key_offset = key_code * 8
[+0] u16 LE  unknown (always 0)
[+2] u16 LE  actuation in 1/100 mm (e.g. 0x004B = 75 = 0.75mm)
[+4] u16 LE  unknown (RT related?)
[+6] u16 LE  unknown (RT related?)
```

### Per-key RGB (CMD 0x14/0x24)
```
key_offset = key_code * 4
[+0] u8  LED index (= key_code, preserve on write)
[+1] u8  Red   (0-255)
[+2] u8  Green (0-255)
[+3] u8  Blue  (0-255)
```

### Key Codes (RGB model, 68-key layout)
```
Row 0:  0=Esc 17-28=1-0,-,= 92=Bksp 104=Home
Row 1:  32=Tab 33-44=Q-] 60=\ 106=Del
Row 2:  48=Caps 49-59=A-' 76=Enter 105=PgUp
Row 3:  64=LShift 65-74=Z-/ 75=RShift 90=Up 108=PgDn
Row 4:  80=LCtrl 81=Win 82=LAlt 83=Space 84=RAlt 85=Fn 87=RCtrl 88-91=Arrows
```

## Flash Memory Map
```
0x9000  DeviceInfo
0x9200  Device Config
0x9600  Key Mapping
0x9A00  Per-key RGB colors
0x9C00  LED animation config (9 x 512B sectors)
0xB000  Unknown table
0xB200  Release actuation region
0xB600  Press actuation table  <-- firmware dump base
0xBA00+ Executable code (ARM Thumb2)
0x134D2 LED animation palette (12 x RGB, hardcoded)
0x13552 LED scan order table
0x135C2 Gamma/brightness LUT (64 entries)
```

## Vulnerability

Read commands do not validate offset against table boundaries.
CMD 0x17 with offset > 0x400 reads arbitrary flash memory.
Full 64KB firmware dump possible via sequential reads.
Write boundary checking status unknown -- not tested.