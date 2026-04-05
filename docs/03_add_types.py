"""
Add structure definitions to Local Types for the AK680 MAX firmware.

Usage: Shift+F2 -> Python -> paste -> Run
Then check Shift+F1 (Local Types) for the new structs.
"""
import idc

idc.parse_decls("""
struct HidReport {
    unsigned char magic;
    unsigned char command;
    unsigned char length;
    unsigned char offset_lo;
    unsigned char offset_hi;
    unsigned char sub_cmd;
    unsigned char flags;
    unsigned char reserved;
    unsigned char payload[56];
};

struct ActuationRecord {
    unsigned short unknown0;
    unsigned short actuation_hundredths_mm;
    unsigned short unknown4;
    unsigned short unknown6;
};

struct RgbKeyRecord {
    unsigned char led_index;
    unsigned char red;
    unsigned char green;
    unsigned char blue;
};

struct LedStateRegister {
    unsigned char effect_id;
    unsigned char constant_ff;
    unsigned char anim_flag0;
    unsigned char anim_flag1;
    unsigned char reserved[4];
    unsigned char engine;
    unsigned char brightness;
    unsigned char speed;
    unsigned char pad[13];
};
""", 0)

print("Structures added to Local Types. Check Shift+F1.")