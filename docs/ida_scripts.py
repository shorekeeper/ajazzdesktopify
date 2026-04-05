"""
IDA Pro analysis scripts for AK680 MAX firmware dumps.

Load firmware.bin into IDA Pro 8.x with:
  Processor: ARM Little-endian
  Loading segment: 0x0
  T register: set to 1 (Alt+G -> T -> 1)

Code starts at offset 0x400. Data at 0x000-0x3FF is the actuation table.
"""

# -- Find HID protocol constants and command dispatch -----------------------

def find_hid_commands():
    """Locate all CMP instructions referencing known HID command bytes."""
    import idc, idautils

    print("=== HID Protocol Constants ===")
    print()

    # Magic byte
    print("Magic byte 0xAA (CMP):")
    for head in idautils.Heads(0x400, 0x10000):
        line = idc.generate_disasm_line(head, 0)
        if line and '#0xAA' in line and 'CMP' in line.upper():
            print(f"  {head:#06x}: {line}")

    print()

    # Command IDs
    for val in [0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
                0x21, 0x22, 0x24, 0x25, 0x27, 0x28, 0x64, 0x65, 0x66, 0x67]:
        found = False
        for head in idautils.Heads(0x400, 0x10000):
            line = idc.generate_disasm_line(head, 0)
            if line and f'#0x{val:X}' in line and ('CMP' in line.upper() or 'SUBS' in line.upper()):
                if not found:
                    print(f"CMD 0x{val:02X}:")
                    found = True
                print(f"  {head:#06x}: {line}")
        if not found:
            print(f"CMD 0x{val:02X}: (not found)")

    print()

    # Large functions (command dispatchers)
    print("Functions with 10+ comparisons (likely dispatchers):")
    for func_ea in idautils.Functions(0x400, 0x10000):
        func_end = idc.find_func_end(func_ea)
        if func_end == idc.BADADDR:
            continue
        cmp_count = sum(
            1 for head in idautils.Heads(func_ea, func_end)
            if idc.print_insn_mnem(head) in ('CMP', 'SUBS', 'CMN')
        )
        if cmp_count >= 10:
            size = func_end - func_ea
            print(f"  sub_{func_ea:X} ({size} bytes, {cmp_count} compares)")


def find_flash_addresses():
    """Find references to known flash table addresses."""
    import idc, idautils

    print()
    print("=== Flash Address References ===")
    print()

    targets = {
        0x9000: "DeviceInfo",
        0x9200: "DevConfig",
        0x9600: "KeyMap",
        0x9A00: "RGB",
        0x9C00: "LEDAnim",
        0xB000: "Table16",
        0xB200: "ReleaseAct",
        0xB600: "PressAct",
        0x134D2: "LEDPalette",
    }

    for addr, name in targets.items():
        found = False
        for head in idautils.Heads(0x400, 0x10000):
            for i in range(2):
                op = idc.get_operand_value(head, i)
                if op == addr:
                    if not found:
                        print(f"{name} (0x{addr:X}):")
                        found = True
                    print(f"  {head:#06x}: {idc.generate_disasm_line(head, 0)}")
        if not found:
            print(f"{name} (0x{addr:X}): (no direct ref)")


def find_strings():
    """List all strings found in the firmware."""
    import idc, idautils

    print()
    print("=== Strings ===")
    print()
    for head in idautils.Heads(0, 0x10000):
        if idc.is_strlit(idc.get_full_flags(head)):
            s = idc.get_strlit_contents(head, -1, idc.STRTYPE_C)
            if s and len(s) >= 4:
                print(f"  {head:#06x}: \"{s.decode('utf-8', errors='replace')}\"")


# -- Run all analyses -------------------------------------------------------

if __name__ == '__main__':
    find_hid_commands()
    find_flash_addresses()
    find_strings()