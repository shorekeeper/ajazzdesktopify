"""
Find all BL/BLX calls from our dump into LOWFLASH (target < 0x9000).
Groups results by calling function and target address.

Usage: Shift+F2 -> Python -> paste -> Run
"""
import idautils, idc, idaapi

print("=== Calls to LOWFLASH (target < 0x9000) ===")
calls = {}

for seg in idautils.Segments():
    start = idc.get_segm_start(seg)
    end = idc.get_segm_end(seg)
    if start < 0x9000 or start >= 0x20000000:
        continue
    for head in idautils.Heads(start, end):
        mnem = idc.print_insn_mnem(head)
        if not mnem:
            continue
        if mnem in ('BL', 'BLX', 'B'):
            op = idc.get_operand_value(head, 0)
            if 0 < op < 0x9000:
                func = idaapi.get_func(head)
                fname = idc.get_func_name(head) if func else "no_func"
                fstart = func.start_ea if func else 0
                key = (fstart, fname)
                if key not in calls:
                    calls[key] = {}
                if op not in calls[key]:
                    calls[key][op] = 0
                calls[key][op] += 1

for (faddr, fname), targets in sorted(calls.items()):
    target_list = ", ".join(
        f"{idc.get_name(t) or f'0x{t:X}'}({c})"
        for t, c in sorted(targets.items())
    )
    print(f"  {faddr:#x} {fname}: {target_list}")

print(f"\nTotal: {len(calls)} calling functions")
print("Done!")