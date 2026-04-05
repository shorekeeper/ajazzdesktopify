"""
Find known firmware strings and their cross-references.
Useful for locating USB descriptor setup and interface init code.

Usage: Shift+F2 -> Python -> paste -> Run
"""
import idautils, idc

targets = ["KEYBOARD", "AK680 MAX", "Interface3", "USBD", "OTGPE", "String"]

for seg in idautils.Segments():
    for head in idautils.Heads(idc.get_segm_start(seg), idc.get_segm_end(seg)):
        flags = idc.get_full_flags(head)
        if idc.is_data(flags):
            s = idc.get_strlit_contents(head)
            if s:
                s_str = s.decode('utf-8', errors='ignore')
                for t in targets:
                    if t in s_str:
                        print(f"  {head:#x}: \"{s_str}\"")
                        for xref in idautils.XrefsTo(head):
                            print(f"    xref from {xref.frm:#x}")

print("Done!")