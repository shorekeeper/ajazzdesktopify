"""
Search the flash dump for ADC-related constants and actuation
threshold references. Helps locate key scan data tables.

Usage: Shift+F2 -> Python -> paste -> Run
"""
import struct
import idaapi, idc

data = open("flash_full.bin", "rb").read()
base = 0x9000

print("=== ADC register references (0x40046xxx) ===")
adc_refs = []
for i in range(0, len(data) - 3, 2):
    val = struct.unpack_from("<I", data, i)[0]
    if 0x40046000 <= val <= 0x40046FFF:
        addr = base + i
        func = idaapi.get_func(addr)
        fname = idc.get_func_name(addr) if func else "?"
        adc_refs.append((addr, val, fname))
    if val in (0x20002F36, 0x20003036):
        addr = base + i
        func = idaapi.get_func(addr)
        fname = idc.get_func_name(addr) if func else "?"
        print(f"  ADC table ref at {addr:#x}: {val:#x} in {fname}")

print(f"\nADC register refs: {len(adc_refs)}")
for addr, val, fname in adc_refs[:30]:
    print(f"  {addr:#x}: -> {val:#x} in {fname}")

print("\n=== Actuation threshold constants (2200 / 1500) ===")
for i in range(0, len(data) - 1, 2):
    val = struct.unpack_from("<H", data, i)[0]
    if val in (2200, 1500):
        addr = base + i
        if 0xBA00 <= addr <= 0x1B600:
            func = idaapi.get_func(addr)
            fname = idc.get_func_name(addr) if func else "?"
            print(f"  {addr:#x}: {val} in {fname}")

print("Done!")