"""
Scan for hardware register references and ADC-related constants.
Helps identify key scan and peripheral init functions.

Usage: Shift+F2 -> Python -> paste -> Run
"""
import idautils, idc, idaapi

hw_targets = {
    0x40046: "ADC",
    0x40034: "CT16B0_Timer",
    0x40036: "CT16B1_Timer",
    0x40038: "GPIO_Port0",
    0x4003A: "GPIO_Port1",
    0x4003C: "GPIO_Port2",
    0x4003E: "GPIO_Port3",
    0x40057: "USB",
}

print("=== Scanning for hardware register references ===")
found = {}

for seg in idautils.Segments():
    start = idc.get_segm_start(seg)
    end = idc.get_segm_end(seg)
    if start < 0x9000 or start >= 0x20000000:
        continue
    for head in idautils.Heads(start, end):
        disasm = idc.generate_disasm_line(head, 0)
        if not disasm:
            continue
        for prefix, name in hw_targets.items():
            hex_str = f"0x{prefix:X}"
            hex_str_lower = f"0x{prefix:x}"
            if hex_str in disasm or hex_str_lower in disasm:
                func = idaapi.get_func(head)
                fname = idc.get_func_name(head) if func else "no_func"
                fstart = func.start_ea if func else head
                key = (fstart, name)
                if key not in found:
                    found[key] = []
                found[key].append(head)

func_hw = {}
for (func_addr, hw_name), addrs in found.items():
    if func_addr not in func_hw:
        func_hw[func_addr] = {}
    func_hw[func_addr][hw_name] = len(addrs)

print("\nFunctions accessing hardware:")
for func_addr in sorted(func_hw.keys()):
    fname = idc.get_func_name(func_addr)
    size = idc.get_func_attr(func_addr, idc.FUNCATTR_END) - func_addr if idaapi.get_func(func_addr) else 0
    hw_list = ", ".join(f"{n}({c})" for n, c in sorted(func_hw[func_addr].items()))
    print(f"  {func_addr:#x} ({size}B) {fname}: {hw_list}")

print("\nDone!")