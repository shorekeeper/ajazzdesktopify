"""
Create SRAM, ROM, and LOWFLASH segments for the AK680 MAX firmware IDA project.
Run after loading flash_full.bin with rebase to 0x9000.

Usage: Shift+F2 -> Python -> paste -> Run
"""
import idaapi

# SRAM (32 KB chip RAM — for naming addresses only)
ok1 = idaapi.add_segm(0, 0x20000000, 0x20008000, "SRAM", "DATA")
print(f"SRAM segment: {ok1}")

# ROM (on-chip boot ROM API)
ok2 = idaapi.add_segm(0, 0xFFFF0000, 0xFFFFA000, "ROM", "CODE")
print(f"ROM segment: {ok2}")

# LOWFLASH (unreachable code, for naming entry points)
ok3 = idaapi.add_segm(0, 0x0, 0x9000, "LOWFLASH", "CODE")
print(f"LOWFLASH segment: {ok3}")

print("Done! Now run 02_name_addresses.py")