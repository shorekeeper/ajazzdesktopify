"""
Name all known addresses in the AK680 MAX firmware IDA project.
Covers SRAM variables, LOWFLASH functions, flash data pointers,
and decompiled functions.

Run after 01_create_segments.py.

Usage: Shift+F2 -> Python -> paste -> Run
"""
import idc

SN = 0x800  # SN_FORCE equivalent for IDA 8.5

all_names = {
    # === SRAM: HID communication buffers ===
    0x20000227: "flag_hid_data_ready",
    0x20005340: "hid_work_buffer",
    0x20005380: "hid_copy_source",
    0x200053C0: "hid_response_buffer",
    0x20005400: "hid_input_buffer",

    # === SRAM: Factory ADC defaults ===
    0x20002F36: "press_adc_defaults",
    0x20003036: "release_adc_defaults",

    # === SRAM: Key processing flags ===
    0x20000076: "flag_key_interrupt",
    0x20000077: "flag_periodic_tick",
    0x20000078: "flag_key_state_update",
    0x20000092: "timer_counters",
    0x200000A2: "main_state_struct",

    # === ROM API ===
    0xFFFF9444: "ROM_FlashWrite",

    # === LOWFLASH: Core infrastructure ===
    0x0260: "sram_memcpy",
    0x0292: "sram_operation",

    # === LOWFLASH: LED subsystem ===
    0x2500: "led_buffer_init",
    0x6FBC: "led_commit_to_hardware",
    0x7320: "static_led_handler",
    0x73EC: "led_effect_step",

    # === LOWFLASH: Key scan subsystem ===
    0x1284: "macro_key_step_execute",
    0x5114: "key_interrupt_handler",
    0x6F98: "key_hid_report_send",
    0x7500: "periodic_key_handler",

    # === LOWFLASH: Configuration ===
    0x1094: "reload_led_state_from_flash",
    0x4A44: "flash_write_sector",
    0x7794: "flash_post_write_handler",

    # === LOWFLASH: USB ===
    0x1670: "hw_peripheral_init",
    0x1770: "usb_device_init",
    0x6A90: "usb_remote_wakeup",

    # === LOWFLASH: System ===
    0x24AC: "enter_firmware_update_mode",
    0x4154: "gpio_pin_release_handler",
    0x7FFE: "hw_watchdog_or_reset",

    # === Decompiled functions ===
    0xBA00: "rgb_led_update",
    0xBDA0: "keyboard_timer_handler",
    0xC22C: "led_buffer_fill_cycle",
    0xC27C: "led_commit_wrapper",
    0xC614: "led_gamma_update",
    0xD398: "gpio_timer_tick",
    0xD504: "hid_command_dispatcher",
    0xDA24: "usb_connection_init",
    0xDFF4: "reload_device_config",
    0xE050: "sram_op_wrapper_A",
    0xE0D0: "sram_op_wrapper_B",
    0xE1F8: "main_loop_tick",
    0xE644: "usb_setup_handler",
    0xEA60: "usb_endpoint_alloc",
    0xF448: "usb_descriptor_setup",
    0x104B8: "usb_fifo_transfer",
    0x113B4: "usb_endpoint_config",
    0x11FAC: "key_action_state_machine",

    # === HID dispatcher data table (0xD904–0xDA20) ===
    0xD904: "g_state_ptr",
    0xD908: "g_hid_input_ptr",
    0xD90C: "g_led_effect_ptr",
    0xD910: "g_led_brightness_ptr",
    0xD914: "g_led_speed_ptr",
    0xD918: "g_led_extra_ptr",
    0xD91C: "g_led_anim_type_ptr",
    0xD920: "g_static_color_lut_ptr",
    0xD924: "g_runtime_info_ptr",
    0xD928: "g_sram_buf_A",
    0xD92C: "g_sram_buf_B",
    0xD930: "g_flash_commit_param",
    0xD934: "g_factory_rgb_ptr",
    0xD938: "g_press_adc_table",
    0xD93C: "g_release_adc_table",
    0xDA10: "g_factory_zero_buf",
    0xDA14: "g_factory_reset_ptr2",
    0xDA18: "g_factory_reset_ptr3",
    0xDA1C: "g_factory_buf_512",
    0xDA20: "g_factory_buf_1024",

    # === LED controller data table (0xBB30–0xBB50) ===
    0xBB30: "g_led_state_struct",
    0xBB34: "g_total_keys_ptr",
    0xBB38: "g_anim_palette",
    0xBB3C: "g_led_buf_R",
    0xBB40: "g_led_buf_G",
    0xBB44: "g_led_buf_B",
    0xBB48: "g_led_routing_table",
    0xBB4C: "g_led_buf_alpha",
    0xBB50: "g_leds_per_key_ptr",

    # === Device config pointers (0xE01C–0xE028) ===
    0xE01C: "g_config_sram_ptr",
    0xE020: "g_config_field_B",
    0xE024: "g_config_field_E",
    0xE028: "g_config_field_mode",

    # === Flash config bytes ===
    0x9003: "cfg_firmware_signature",
    0x9203: "cfg_flash_mode",
    0x9205: "cfg_flash_param1",
    0x9208: "cfg_flash_param2",
    0x9209: "cfg_flash_param3",
    0x920B: "cfg_flash_param4",
    0x920E: "cfg_flash_param5",
}

for addr, name in all_names.items():
    idc.set_name(addr, name, SN)

print(f"Named {len(all_names)} addresses. Done!")
print("Now press G -> 0xD504 -> F5 to decompile the command dispatcher.")