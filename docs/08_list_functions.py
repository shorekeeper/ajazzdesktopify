"""
List all functions in the dump sorted by address.
Only shows functions > 50 bytes to filter out stubs.

Usage: Shift+F2 -> Python -> paste -> Run
"""
import idautils, idc

print("=== All functions in dump (> 50 bytes) ===")
count = 0
for func_ea in idautils.Functions(0x9000, 0x1B600):
    name = idc.get_func_name(func_ea)
    end = idc.get_func_attr(func_ea, idc.FUNCATTR_END)
    size = end - func_ea
    if size > 50:
        count += 1
        print(f"  {func_ea:#x} ({size:5d} bytes) {name}")

print(f"\nTotal: {count} functions > 50 bytes")
print("Done!")