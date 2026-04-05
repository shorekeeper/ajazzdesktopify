/***********************************************************************
 * AJAZZ AK680 MAX RGB / Pseudocode
 * 
 * Chip:     Sonix SN32F248B (ARM Cortex-M0+, Thumb)
 * Flash:    128 KB (0x00000–0x1FFFF)  
 * SRAM:     32 KB  (0x20000000–0x20007FFF)
 * ROM API:  0xFFFF0000+ (on-chip boot ROM)
 *
 * Dump:     75 KB (0x9000–0x1B5FF) via HID read overflow vulnerability
 * Missing:  0x0000–0x8FFF (boot/core code, not readable via HID)
 *
 * HID Interface 2 (0xFF68): Configuration commands (64-byte interrupt)
 * HID Interface 3 (0xFF67): Firmware update (4096-byte output reports)
 *
 ***********************************************************************/

#include <stdint.h>
#include <stdbool.h>

/*======================================================================
 * FLASH MEMORY MAP (confirmed via HID command dispatcher at 0xD504)
 *======================================================================
 *
 * 0x0000–0x8FFF   Boot/core code (36 KB)
 *                  NOT readable via HID  contains:
 *                    ADC key scanning
 *                    Hall-effect threshold comparison
 *                    Key debounce logic
 *                    HID keyboard report generation
 *                    USB device stack core
 *                    LED hardware DMA driver
 *                    Firmware update handler
 *
 * 0x8200           Factory RGB defaults (16 bytes, written during reset)
 * 0x8400           Factory press ADC defaults (256 bytes)
 * 0x8500           Factory release ADC defaults (256 bytes)
 *
 * 0x9000–0x91FF   DeviceInfo          (CMD 0x10, read-only)
 *   0x9003         Firmware signature byte (auto-fixed 0x12 -> 0x92)
 *
 * 0x9200–0x93FF   Device Config       (CMD 0x11 / 0x21)
 *   0x9203         Config mode byte    (loaded into SRAM by reload_device_config)
 *   0x9205         Config param 1
 *   0x9208         Config param 2
 *   0x9209         Config param 3
 *   0x920B         Config param 4
 *   0x920E         Config param 5
 *
 * 0x9600–0x97FF   Key Mapping         (CMD 0x12 / 0x22, HID keycodes)
 *                  Mirrored in SRAM at g_sram_buf_A during writes
 *
 * 0x9800–0x99FF   LED State persist   (CMD 0x23 writes here)
 *                  Format matches LedStateRegister struct
 *                  Read back into SRAM by reload_led_state_from_flash()
 *
 * 0x9A00–0x9BFF   Per-key RGB colors  (CMD 0x14 / 0x24)
 *                  128 slots × 4 bytes = 512 bytes
 *                  [led_index, R, G, B] per key
 *
 * 0x9C00–0xADFF   LED Animation cfg   (CMD 0x15/0x25, 9×512B sectors)
 *
 * 0xB000–0xB1FF   Unknown table       (CMD 0x16 / 0x26)
 *
 * 0xB200–0xB5FF   Release Actuation   (CMD 0x18 / 0x28)
 *                  Data starts at +0x400 offset (first 1024B zeros)
 *
 * 0xB600–0xB9FF   Press Actuation     (CMD 0x17 / 0x27)
 *                  128 slots × 8 bytes = 1024 bytes
 *                  [u16 unk, u16 hundredths_mm, u16 unk, u16 unk]
 *                  Mirrored in SRAM at g_sram_buf_B during writes
 *
 * 0xBA00+          Executable firmware code
 *                  (start of readable code region)
 *
 * Key data tables in upper flash:
 * 0x134D2          Animation palette (12 × 3B RGB entries)
 * 0x13552          LED scan order table
 * 0x135C2          Gamma correction LUT (96 entries)
 * 0x1385D          LED routing table (key -> hardware LED, 12 per key)
 *====================================================================*/

/*======================================================================
 * TYPE DEFINITIONS
 *====================================================================*/

/** 64-byte HID report as seen on the wire */
struct HidReport {
    uint8_t magic;       /**< 0xAA (host->device) / 0x55 (device->host) */
    uint8_t command;     /**< Operation ID */
    uint8_t length;      /**< Payload bytes this chunk (max 56) */
    uint8_t offset_lo;   /**< Low byte of 16-bit offset */
    uint8_t offset_hi;   /**< High byte of 16-bit offset */
    uint8_t sub_cmd;     /**< Command-specific parameter */
    uint8_t flags;       /**< 0x01 on last write chunk */
    uint8_t reserved;    /**< Always 0x00 */
    uint8_t payload[56]; /**< Chunk data */
};

/** Per-key actuation record (8 bytes at key_code × 8 in CMD 0x17/0x18) */
struct ActuationRecord {
    uint16_t unknown0;              /**< Always 0x0000 observed */
    uint16_t actuation_hundredths;  /**< Depth in 1/100 mm (e.g. 75 = 0.75mm) */
    uint16_t unknown4;              /**< RT-related candidate */
    uint16_t unknown6;              /**< RT-related candidate */
};

/** Per-key RGB record (4 bytes at key_code × 4 in CMD 0x14/0x24) */
struct RgbKeyRecord {
    uint8_t led_index;  /**< Hardware LED index (= key_code, preserve on write) */
    uint8_t red;        /**< 0–255 */
    uint8_t green;      /**< 0–255 */
    uint8_t blue;       /**< 0–255 */
};

/** LED state register (CMD 0x13 read / 0x23 write, 24 bytes) */
struct LedStateRegister {
    uint8_t effect_id;     /**< 0x00=off, 0x01–0x14=effect */
    uint8_t constant_ff;   /**< Always 0xFF */
    uint8_t anim_flag0;    /**< 0xFF if animated, 0x00 if static */
    uint8_t anim_flag1;    /**< Same as anim_flag0 */
    uint8_t reserved[4];   /**< Always 0x00 */
    uint8_t engine;        /**< 0x01=animation engine ON, 0x00=static/off */
    uint8_t brightness;    /**< 1–5 */
    uint8_t speed;         /**< 1–5 */
    uint8_t pad[13];       /**< Always 0x00 */
};

/*======================================================================
 * FUNCTIONS IN LOWFLASH (addresses confirmed, code NOT readable)
 *
 * These are called from our dump via BL/BLX instructions.
 * Their signatures are inferred from calling context.
 *====================================================================*/

/* Core infrastructure */
extern void sram_memcpy(void *dst, const void *src, int len);     /* 0x0260 */
extern void sram_operation(void *buf, int size);                   /* 0x0292  memset/clear */

/* LED subsystem */
extern void led_commit_to_hardware(void);                          /* 0x6FBC */
extern void static_led_handler(void);                              /* 0x7320  handles effect types < 8 */
extern void led_effect_step(void);                                 /* 0x73EC */
extern void led_buffer_init(void);                                 /* 0x2500 */

/* Key scan subsystem (ALL ADC logic lives here) */
extern void key_interrupt_handler(void);                           /* 0x5114  ADC scan complete ISR */
extern void periodic_key_handler(void);                            /* 0x7500  periodic key tick */
extern void key_hid_report_send(void);                             /* 0x6F98  send USB HID key report */
extern void macro_key_step_execute(void);                          /* 0x1284  execute one macro step */

/* Configuration */
extern void reload_led_state_from_flash(void);                     /* 0x1094 */
extern void flash_write_sector(const void *data, int len,
                               uint32_t flash_addr, int flags);    /* 0x4A44 */
extern void flash_post_write_handler(void);                        /* 0x7794 */

/* USB */
extern void usb_device_init(void);                                 /* 0x1770 */
extern void usb_remote_wakeup(void);                               /* 0x6A90 */
extern void hw_peripheral_init(void);                              /* 0x1670 */

/* System */
extern void enter_firmware_update_mode(uint8_t cmd);               /* 0x24AC  !! DANGER !! */
extern void gpio_pin_release_handler(void);                        /* 0x4154 */
extern void hw_watchdog_or_reset(void);                            /* 0x7FFE */

/*======================================================================
 * SRAM VARIABLES (addresses from decompilation cross-references)
 *====================================================================*/

/* HID communication buffers */
volatile uint8_t  flag_hid_data_ready;       /* 0x20000227  set by USB ISR */
uint8_t           hid_work_buffer[64];       /* 0x20005340  response assembly */
uint8_t           hid_copy_source[64];       /* 0x20005380 */
uint8_t           hid_response_buffer[64];   /* 0x200053C0 */
uint8_t           hid_input_buffer[64];      /* 0x20005400  incoming report */

/* Factory defaults (ADC threshold counts, NOT millimeters) */
uint16_t          press_adc_defaults[128];   /* 0x20002F36  default: 2200 */
uint16_t          release_adc_defaults[128]; /* 0x20003036  default: 1500 */

/* Key processing flags (set by LOWFLASH ISRs, consumed by main loop) */
volatile uint8_t  flag_key_interrupt;        /* 0x20000076  ADC scan done */
volatile uint8_t  flag_periodic_tick;        /* 0x20000077  timer tick */
volatile uint8_t  flag_key_state_update;     /* 0x20000078  key state changed */
uint8_t           timer_counters[16];        /* 0x20000092 */

/* Main state structure (huge, >400 bytes) */
uint8_t           main_state_struct[512];    /* 0x200000A2 */
/*  +8   u16   action_timer (countdown for key_action_state_machine)
 *  +26  u32   action_flags (bitfield dispatching actions)
 *  +126 u32   debounce_counter
 *  +130 u8    usb_suspended
 *  +148 u8    fn_key_flag
 *  +150 u8    fn_alt_flag
 *  +151 u8    some_enable_flag
 *  +163 u8    macro_steps_remaining
 *  +190 u16   led_timer_A
 *  +192 u16   led_reload_A
 *  +194 u8    led_counter_A
 *  +196 u16   fn_combo_timer
 *  +198 u16   fn_combo_reload
 *  +202 u16   main_timer
 *  +204 u16   main_reload
 *  +220 u16   effect_timer_A
 *  +222 u16   effect_reload_A
 *  +224 u8    effect_counter_A
 *  +226 u16   effect_timer_B
 *  +228 u16   effect_reload_B
 *  +230 u8    effect_counter_B
 *  +232 u16   effect_timer_C
 *  +234 u16   effect_reload_C
 *  +236 u8    effect_counter_C
 *  +252 u8    blink_phase (0–6, used by led_buffer_fill_cycle)
 *  +261 u8    led_effect_active
 *  +266 u8    animation_direction (1=brighten, 0=dim)
 *  +271 u8    effect_enable_A
 *  +272 u8    effect_enable_B
 *  +273 u8    effect_enable_C
 *  +279 u8    effect_completion_flag
 *  +290 u32   status_bitfield (GPIO output state, LED toggles)
 *  +385 u8    some_flag_cleared_on_timer
 */

/* LED buffers */
uint8_t           led_pwm_buffer[256];       /* 0x20004A3A  DMA to LED driver */

/*======================================================================
 * GLOBAL POINTERS (flash data table at 0xD904–0xDA20)
 *
 * These are 4-byte pointers loaded by the dispatcher.
 * They reference SRAM structures populated at boot.
 *====================================================================*/

/* 0xD904 */ void     *g_state_ptr;              /* state struct with data_ready flag */
/* 0xD908 */ uint8_t  *g_hid_input_ptr;          /* 64-byte HID input buffer */
/* 0xD90C */ uint8_t  *g_led_effect_ptr;         /* current effect ID (internal encoding) */
/* 0xD910 */ uint8_t  *g_led_brightness_ptr;     /* brightness 1–5 */
/* 0xD914 */ uint8_t  *g_led_speed_ptr;          /* speed 1–5 */
/* 0xD918 */ uint8_t  *g_led_extra_ptr;          /* extra LED param */
/* 0xD91C */ uint8_t  *g_led_anim_type_ptr;      /* animation type (< 8 = static, >= 8 = anim, 0xFF = custom) */
/* 0xD920 */ uint8_t  *g_static_color_lut_ptr;   /* 8 × 3B static color palette */
/* 0xD924 */ uint32_t *g_runtime_info_ptr;       /* 4-byte runtime data for CMD 0x10 */
/* 0xD928 */ void     *g_sram_buf_A;             /* SRAM mirror of key mapping (0x9600) */
/* 0xD92C */ void     *g_sram_buf_B;             /* SRAM mirror of press actuation (0xB600) */
/* 0xD930 */ void     *g_flash_commit_param;     /* param for flash commit operation */
/* 0xD934 */ void     *g_factory_rgb_ptr;        /* factory RGB defaults */
/* 0xD938 */ uint16_t *g_press_adc_table;        /* SRAM press ADC thresholds */
/* 0xD93C */ uint16_t *g_release_adc_table;      /* SRAM release ADC thresholds */
/* 0xDA10 */ void     *g_factory_zero_buf;       /* zero-filled buffer for erasing */
/* 0xDA14 */ void     *g_factory_reset_ptr2;     /* factory reset helper */
/* 0xDA18 */ void     *g_factory_reset_ptr3;     /* factory reset helper */
/* 0xDA1C */ void     *g_factory_buf_512;        /* 512B buffer for factory reset */
/* 0xDA20 */ void     *g_factory_buf_1024;       /* 1024B buffer for factory reset */

/* LED controller pointers (flash data table at 0xBB30–0xBB50) */
/* 0xBB30 */ void     *g_led_state_struct;       /* LED state ([+10]=cur_key, [+14]=anim_type, [+52]=per-key) */
/* 0xBB34 */ uint8_t  *g_total_keys_ptr;         /* total key count */
/* 0xBB38 */ uint8_t  *g_anim_palette;           /* 3B per key (R,G,B) animation colors */
/* 0xBB3C */ uint8_t  *g_led_buf_R;              /* per-LED red channel buffer */
/* 0xBB40 */ uint8_t  *g_led_buf_G;              /* per-LED green channel buffer */
/* 0xBB44 */ uint8_t  *g_led_buf_B;              /* per-LED blue channel buffer */
/* 0xBB48 */ uint8_t  *g_led_routing_table;      /* 12 entries per key -> hardware LED index */
/* 0xBB4C */ uint8_t  *g_led_buf_alpha;          /* per-LED alpha/enable (0xFF = on) */
/* 0xBB50 */ uint8_t  *g_leds_per_key_ptr;       /* LEDs per physical key */

/* Device config pointers (flash data table at 0xE01C–0xE028) */
/* 0xE01C */ uint8_t  *g_config_sram_ptr;        /* SRAM config struct */
/* 0xE020 */ uint8_t  *g_config_field_B;         /* loaded from flash 0x920B */
/* 0xE024 */ uint8_t  *g_config_field_E;         /* loaded from flash 0x920E */
/* 0xE028 */ uint8_t  *g_config_field_mode;      /* loaded from flash 0x9203 */

/*======================================================================
 * @0xD504 (1290 bytes)  HID Command Dispatcher
 *
 * Entry: called from USB interrupt when a 64-byte report arrives
 *        on HID Interface 2 (usage page 0xFF68).
 *
 * VULNERABILITY: Read commands do NOT validate offset against table
 * boundaries. CMD 0x17 with offset > 0x400 reads arbitrary flash,
 * enabling a full 64 KB firmware dump.
 *====================================================================*/
void hid_command_dispatcher(void)
{
    /* Check if new HID data arrived (set by USB ISR) */
    if (!g_state->data_ready)
        return;
    g_state->data_ready = 0;

    /* Verify magic byte */
    uint8_t *input = g_hid_input;
    if (input[0] != 0xAA) {
        g_state->response_ready = 1;  /* send empty response */
        return;
    }

    /* Copy input to response buffer, set response magic */
    uint8_t *resp = input - 64;  /* response buffer is 64 bytes before input */
    for (uint8_t i = 0; i < 64; i++)
        resp[i] = input[i];
    resp[0] = 0x55;

    /* Parse header fields */
    uint8_t  cmd       = input[1];
    uint8_t  length    = input[2];   /* payload bytes this chunk (max 56) */
    uint8_t  offset_lo = input[3];
    uint8_t  offset_hi = input[4];
    /* input[5] = sub_cmd */
    /* input[6] = flags (0x01 on last write chunk) */
    /* input[7] = reserved */

    /*--------------------------------------------------------------
     * READ COMMANDS (0x10–0x18)
     *
     * Each command reads from a fixed flash base address plus
     * the 16-bit offset from the header. No bounds checking!
     *--------------------------------------------------------------*/
    if ((cmd >> 4) == 1) {
        uint32_t flash_base = 0x9C00; /* default for CMD 0x15 */

        switch (cmd) {
            case 0x10: flash_base = 0x9000; break;  /* DeviceInfo */
            case 0x11: flash_base = 0x9200; break;  /* Device Config */
            case 0x12: flash_base = 0x9600; break;  /* Key Mapping */
            /* case 0x13: special  see below */
            case 0x14: flash_base = 0x9A00; break;  /* Per-key RGB */
            case 0x15: flash_base = 0x9C00; break;  /* LED Animation */
            case 0x16: flash_base = 0xB000; break;  /* Unknown table */
            case 0x17: flash_base = 0xB600; break;  /* Press Actuation */
            case 0x18: flash_base = 0xB200; break;  /* Release Actuation */

            case 0x13:
                /*----------------------------------------------
                 * CMD 0x13: LED State Register (from SRAM)
                 *
                 * Unlike all other reads, this populates from
                 * live SRAM values, not flash.
                 *
                 * Effect ID encoding:
                 *   Internal <  0x0B -> wire = internal + 1
                 *   Internal >= 0x0B -> wire = internal (no shift)
                 *
                 * Animation type determines flags:
                 *   < 8  -> static: flags from color LUT, engine OFF
                 *   >= 8 -> animated: flags = 0xFF, engine ON
                 *----------------------------------------------*/
            {
                uint8_t effect = *g_led_effect_ptr;
                resp[8] = (effect >= 0x0B) ? effect : (effect + 1);

                resp[17] = *g_led_brightness_ptr;  /* payload[9]  */
                resp[18] = *g_led_speed_ptr;       /* payload[10] */
                resp[19] = *g_led_extra_ptr;       /* payload[11] */

                uint8_t anim_type = *g_led_anim_type_ptr;
                if (anim_type >= 8) {
                    /* Animated effect */
                    resp[9]  = 0xFF;  /* constant */
                    resp[10] = 0xFF;  /* anim_flag0 */
                    resp[11] = 0xFF;  /* anim_flag1 */
                    resp[16] = 0x01;  /* engine ON */
                } else {
                    /* Static effect  use color from palette LUT */
                    uint8_t *color = g_static_color_lut_ptr + anim_type * 3;
                    resp[9]  = color[0];  /* R */
                    resp[10] = color[1];  /* G */
                    resp[11] = color[2];  /* B */
                    resp[16] = 0x00;      /* engine OFF */
                }
                break;
            }
        }

        /* Generic flash read  UNBOUNDED, vulnerability here */
        if (cmd != 0x13) {
            uint32_t addr = flash_base + offset_lo + (offset_hi << 8);
            for (uint8_t k = 0; k < length; k++)
                resp[8 + k] = *(volatile uint8_t *)(addr + k);
        }

        /* CMD 0x10 special: auto-fix firmware signature + append runtime */
        if (cmd == 0x10) {
            if (*(uint8_t *)0x9003 == 0x12) {
                uint8_t val = 0x92;
                flash_write_sector(&val, 1, 0x9003, 0);
            }
            uint32_t rt = *g_runtime_info_ptr;
            resp[22] = (rt >>  0) & 0xFF;
            resp[23] = (rt >>  8) & 0xFF;
            resp[24] = (rt >> 16) & 0xFF;
            resp[25] = (rt >> 24) & 0xFF;
            resp[26] = 0;
        }

        g_state->response_ready = 1;
        return;
    }

    /*--------------------------------------------------------------
     * WRITE COMMANDS (0x21–0x28)
     *
     * Writes payload data to flash via flash_write_sector().
     * Handles sector boundary crossing (512-byte sectors).
     * Post-write hooks for Config (0x21) and LED State (0x23).
     *--------------------------------------------------------------*/
    if ((cmd >> 4) == 2) {
        uint32_t flash_base;
        switch (cmd) {
            case 0x21: flash_base = 0x9200; break;  /* Device Config */
            case 0x22: flash_base = 0x9600; break;  /* Key Mapping */
            case 0x23: flash_base = 0x9800; break;  /* LED State -> persistent! */
            case 0x24: flash_base = 0x9A00; break;  /* Per-key RGB */
            case 0x25: flash_base = 0x9C00; break;  /* LED Animation */
            case 0x26: flash_base = 0xB000; break;  /* Unknown */
            case 0x27: flash_base = 0xB600; break;  /* Press Actuation */
            case 0x28: flash_base = 0xB200; break;  /* Release Actuation */
            default: return;
        }

        uint32_t dest = flash_base + offset_lo + (offset_hi << 8);
        uint16_t offset16 = offset_lo + (offset_hi << 8);

        /* SRAM mirror update for certain tables */
        if (flash_base == 0x9600)  /* Key Mapping */
            sram_memcpy(g_sram_buf_A + offset16, input + 8, length);
        else if (flash_base == 0xB600)  /* Press Actuation */
            sram_memcpy(g_sram_buf_B + offset16, input + 8, length);

        /* Flash write with sector-crossing logic */
        uint32_t sec_start = dest >> 9;           /* 512-byte sectors */
        uint32_t sec_end   = (dest + length) >> 9;

        if (sec_start == sec_end) {
            /* Single sector  straightforward write */
            flash_write_sector(input + 8, length, dest, 0);
        } else {
            /* Cross-sector  split into two writes */
            uint8_t first_part = -(uint8_t)dest;  /* bytes to sector boundary */
            flash_write_sector(input + 8, first_part, dest, 0);
            if (length - first_part > 0)
                flash_write_sector(input + 8 + first_part,
                                   length - first_part,
                                   dest + first_part, 0);
        }

        /* Post-write hooks */
        if (cmd == 0x21)  /* Device Config -> reload SRAM copy */
            reload_device_config();
        if (cmd == 0x23)  /* LED State -> apply immediately */
            reload_led_state_from_flash();

        g_state->response_ready = 1;
        return;
    }

    /*--------------------------------------------------------------
     * SYSTEM COMMANDS (0x64–0x67)
     *--------------------------------------------------------------*/
    if ((cmd >> 4) == 6) {
        switch (cmd) {
            case 0x64:  /* FLASH COMMIT  finalize pending writes */
                sram_operation(g_flash_commit_param, 16);
                g_state->byte1 = 1;
                break;

            case 0x65:  /* ENTER FIRMWARE UPDATE MODE */
                /*
                 * !!! DANGER  DO NOT SEND WITHOUT VALID FIRMWARE IMAGE !!!
                 *
                 * Calls LOWFLASH 0x24AC which:
                 *   1. Re-enumerates USB device
                 *   2. Activates Interface 3 for 4096-byte output reports
                 *   3. Device becomes a DFU target
                 *   4. Without proper firmware data -> BRICKED keyboard
                 */
                enter_firmware_update_mode(0x65);
                g_state->byte1 = 0;
                break;

            case 0x66:  /* SET UPDATE FLAG */
                g_state->byte2 = 1;
                break;

            case 0x67:  /* CLEAR UPDATE FLAG */
                g_state->byte2 = 0;
                break;
        }
        g_state->response_ready = 1;
        return;
    }

    /*--------------------------------------------------------------
     * FACTORY RESET (cmd low nibble == 0xF, sub-command in input[2])
     *
     * Erases flash regions and restores factory defaults.
     * Different sub values reset different subsystems.
     *--------------------------------------------------------------*/
    if ((~cmd << 28) == 0) {  /* cmd & 0x0F == 0x0F */
        /* Prepare a 512-byte zero buffer */
        sram_operation(g_hid_input + 64, 512);
        uint8_t sub = input[2];

        if (sub == 1) {
            /* Reset key mapping + actuation tables */
            flash_write_sector(zeros, 512, 0x9600, 0);  /* Key Mapping */
            flash_write_sector(zeros, 512, 0xB600, 0);  /* Press Actuation pt1 */
            flash_write_sector(zeros, 512, 0xB800, 0);  /* Press Actuation pt2 */
            flash_write_sector(zeros, 512, 0xB200, 0);  /* Release Actuation pt1 */
            flash_write_sector(zeros, 512, 0xB400, 0);  /* Release Actuation pt2 */
            sram_operation(g_sram_buf_A, 512);           /* Clear SRAM mirror */
            sram_operation(g_sram_buf_B, 1024);          /* Clear SRAM mirror */
            flash_post_write_handler();
        }

        if (sub == 2) {
            /* Reset LED state + per-key RGB colors */
            flash_write_sector(zeros, 16,  0x9800, 0);            /* LED State */
            flash_write_sector(zeros, 512, 0x9A00, 0);            /* Per-key RGB */
            flash_write_sector(g_factory_rgb_ptr, 16, 0x8200, 0); /* Factory RGB */
            flash_post_write_handler();
        }

        if (sub == 4) {
            /* Reset LED animation config (9 sectors) */
            for (uint8_t i = 0; i < 9; i++)
                flash_write_sector(zeros, 512, 0x9C00 + i * 512, 0);
            flash_post_write_handler();
        }

        if (sub == 5) {
            /* Reset actuation to firmware defaults (ADC units) */
            for (uint8_t k = 0; k < 128; k++) {
                g_press_adc_table[k]   = 2200;  /* press default */
                g_release_adc_table[k] = 1500;  /* release default */
            }
            flash_write_sector(g_press_adc_table,   256, 0x8400, 0);
            flash_write_sector(g_release_adc_table, 256, 0x8500, 0);
        }

        if (sub == 0xFF) {
            /* FULL FACTORY RESET  erases everything */
            flash_write_sector(zeros, 16,   0x9800, 0);  /* LED State */
            flash_write_sector(zeros, 512,  0x9A00, 0);  /* Per-key RGB */
            flash_write_sector(g_factory_reset_ptr2, 16, 0x8200, 0);
            flash_write_sector(g_factory_reset_ptr3, 16, 0x9200, 0);  /* Config */
            flash_write_sector(zeros, 512,  0x9600, 0);  /* Key Mapping */
            flash_write_sector(zeros, 512,  0xB600, 0);  /* Press Act pt1 */
            flash_write_sector(zeros, 512,  0xB800, 0);  /* Press Act pt2 */
            flash_write_sector(zeros, 512,  0xB200, 0);  /* Release Act pt1 */
            flash_write_sector(zeros, 512,  0xB400, 0);  /* Release Act pt2 */
            flash_write_sector(zeros, 512,  0xB000, 0);  /* Unknown table */
            sram_operation(g_factory_buf_512, 512);
            sram_operation(g_factory_buf_1024, 1024);
            for (uint8_t i = 0; i < 9; i++)
                flash_write_sector(zeros, 512, 0x9C00 + i * 512, 0);
            flash_post_write_handler();
            reload_device_config();
        }

        g_state->response_ready = 1;
        return;
    }

    /* Unknown command  send empty response */
    g_state->response_ready = 1;
}

/*======================================================================
 * @0xBA00 (304 bytes)  RGB LED Frame Update
 *
 * Called every animation frame by the LED timer interrupt.
 * Writes R/G/B values into three separate DMA buffers.
 *
 * LED routing table: 12 entries per key, mapping logical key
 * indices to physical LED hardware addresses. Each key can
 * drive up to 12 individual LEDs (for per-key RGB with multiple
 * LED elements per switch).
 *
 * Animation mode (state[+14]):
 *   < 8    Static effects, dispatched to static_led_handler() @0x7320
 *   8–0xFE Animated effects, progressive (one key per frame)
 *   0xFF   Custom Per-Key, all keys updated every frame from
 *          per-key data structure at state[+52]
 *====================================================================*/
void rgb_led_update(void)
{
    uint8_t *state   = g_led_state_struct;
    uint8_t *palette = g_anim_palette;       /* 3 bytes per key (R,G,B) */
    uint8_t *buf_R   = g_led_buf_R;          /* per-LED red channel */
    uint8_t *buf_G   = g_led_buf_G;          /* per-LED green channel */
    uint8_t *buf_B   = g_led_buf_B;          /* per-LED blue channel */
    uint8_t *routing = g_led_routing_table;  /* 12 entries per key */
    uint8_t *alpha   = g_led_buf_alpha;

    uint8_t key_count    = *g_total_keys_ptr;
    uint8_t leds_per_key = *g_leds_per_key_ptr;
    uint8_t anim_type    = state[14];
    uint8_t cur_key      = state[10];

    /*--- Progressive animation: one key per frame ---*/
    if (cur_key < key_count) {
        for (uint8_t led = 0; led < leds_per_key; led++) {
            if (anim_type < 8) {
                static_led_handler();  /* LOWFLASH 0x7320 */
                return;
            }
            uint8_t hw = routing[cur_key * 12 + led];
            buf_R[hw] = palette[cur_key * 3 + 0];
            buf_G[hw] = palette[cur_key * 3 + 1];
            buf_B[hw] = palette[cur_key * 3 + 2];
            alpha[hw] = 0xFF;
        }
        state[10]++;
    }

    /*--- Custom Per-Key mode (type 0xFF) ---*/
    if (anim_type == 0xFF) {
        state[10] = 0;  /* reset frame counter */
        uint8_t *per_key = state + 52;  /* per-key data array */

        for (uint8_t key = 0; key < key_count; key++) {
            for (uint8_t led = 0; led < leds_per_key; led++) {
                uint8_t hw = routing[key * 12 + led];

                if (per_key[4]) {
                    /* use_palette_flag -> animation palette colors */
                    buf_R[hw] = palette[key * 3 + 0];
                    buf_G[hw] = palette[key * 3 + 1];
                    buf_B[hw] = palette[key * 3 + 2];
                } else {
                    /* Custom color from per-key data */
                    buf_R[hw] = per_key[1];  /* R */
                    buf_G[hw] = per_key[2];  /* G */
                    buf_B[hw] = per_key[3];  /* B */
                }
                alpha[hw] = 0xFF;
            }
            /* state[10] increments implicitly for indexing */
        }
    }
}

/*======================================================================
 * @0xC614 (102 bytes)  LED Gamma/Brightness Update
 *
 * 34 LED groups × 3 physical LEDs each = 102 total LEDs.
 * Each group has a phase counter (0–95, 96 steps).
 * Phase indexes into gamma LUT at flash 0x135C2 for
 * the corrected PWM duty cycle value.
 *
 * The direction flag determines whether the animation is
 * brightening (counting up) or dimming (counting down).
 * At the boundaries, the counter wraps around.
 *====================================================================*/
void led_gamma_update(void)
{
    for (uint8_t i = 0; i < 0x22; i++) {   /* 34 LED groups */
        uint8_t *phase = &sram_led_phases[i];

        if (animation_direction) {
            /* Counting up (brightening) */
            if (++(*phase) >= 96)
                *phase = 0;
        } else {
            /* Counting down (dimming) */
            if (*phase > 0)
                (*phase)--;
            else
                *phase = 95;  /* wrap 0 -> 95 */
        }

        /* Gamma-corrected brightness from LUT */
        uint8_t brightness = gamma_lut_135C2[*phase];

        /* Apply to 3 physical LEDs per group via routing */
        for (uint8_t j = 0; j < 3; j++) {
            uint8_t hw = led_routing_1385D[i * 3 + j];
            led_pwm_buffer[hw] = brightness;
        }
    }
}

/*======================================================================
 * @0xDFF4 (24 bytes)  Reload Device Config
 *
 * Called after CMD 0x21 (Device Config write) completes.
 * Reads 6 specific bytes from flash 0x9200+ into SRAM
 * config structure. Most of config region is unused (zeros).
 *====================================================================*/
void reload_device_config(void)
{
    g_config_sram_ptr[5] = *(volatile uint8_t *)0x9205;
    g_config_sram_ptr[6] = *(volatile uint8_t *)0x9208;
    g_config_sram_ptr[7] = *(volatile uint8_t *)0x9209;
    *g_config_field_B    = *(volatile uint8_t *)0x920B;
    *g_config_field_E    = *(volatile uint8_t *)0x920E;
    *g_config_field_mode = *(volatile uint8_t *)0x9203;
}

/*======================================================================
 * @0xBDA0 (376 bytes)  Keyboard Timer Handler
 *
 * Manages multiple countdown timers for LED effects and key
 * processing. Toggles bits in the status bitfield at +290 in
 * main_state_struct. Controls GPIO pins:
 *   GPIO Port 0 bit 14 (0x40038010/14)
 *   GPIO Port 1 bit 10 (0x4003A010/14)
 *   GPIO Port 1 bit 11 (0x4003A010/14)
 *====================================================================*/
void keyboard_timer_handler(void)
{
    /* Timer A: main interval (status bit 6 toggle) */
    if (main_state.enable_151 && main_state.timer_202 == 0) {
        main_state.timer_202 = main_state.reload_204;
        main_state.status_290 ^= 0x40;
    }

    /* Timer B: effect B (status bit 11 toggle, countdown) */
    if (main_state.effect_enable_B_272 && main_state.timer_226 == 0) {
        main_state.timer_226 = main_state.reload_228;
        main_state.counter_230--;
        main_state.status_290 ^= 0x800;
        if (main_state.counter_230 == 0)
            main_state.effect_enable_B_272 = 0;
    }

    /* Timer C: effect C (status bit 12 toggle, countdown) */
    if (main_state.effect_enable_C_273 && main_state.timer_232 == 0) {
        main_state.timer_232 = main_state.reload_234;
        main_state.counter_236--;
        main_state.status_290 ^= 0x1000;
        if (main_state.counter_236 == 0)
            main_state.effect_enable_C_273 = 0;
    }

    /* Timer A: effect A (status bit 9 toggle, countdown + callback) */
    if (main_state.effect_enable_A_271 && main_state.timer_220 == 0) {
        main_state.timer_220 = main_state.reload_222;
        main_state.counter_224--;
        main_state.status_290 ^= 0x200;
        if (main_state.counter_224 == 0)
            main_state.effect_enable_A_271 = 0;
        if (main_state.completion_flag_279 == 1) {
            main_state.completion_flag_279 = 0;
            led_commit_wrapper();  /* sub_C328 */
        }
    }

    /* LED effect timer (status bit 10 toggle) */
    if (main_state.led_effect_active_261 && main_state.timer_190 == 0) {
        main_state.timer_190 = main_state.reload_192;
        main_state.counter_194--;
        main_state.status_290 ^= 0x400;
    }

    /* Fn key combo timer (status bit 4 toggle) */
    if ((main_state.fn_flag_148 | main_state.fn_alt_150) && main_state.fn_timer_196 == 0) {
        main_state.fn_timer_196 = main_state.fn_reload_198;
        main_state.status_290 ^= 0x10;
    }

    /* Merge key scan results into low bits of status */
    uint32_t status = main_state.status_290 & 0xFFFFFFF0;
    if (key_scan_results[0] & 1) status |= 1;
    if (key_scan_results[0] & 2) status |= 2;
    if (key_scan_results[0] & 4) status |= 4;
    if ((1 << main_state.current_layer) & main_state.layer_mask) status |= 8;
    main_state.status_290 = status;

    /* Apply to GPIO */
    if (status & 2) GPIO_P0_SET |= (1 << 14);
    else             GPIO_P0_CLR |= (1 << 14);

    if (status & 8) GPIO_P1_SET |= (1 << 10);
    else             GPIO_P1_CLR |= (1 << 10);

    if (main_state.current_layer == 1)
        GPIO_P1_SET |= (1 << 11);
    else
        GPIO_P1_CLR |= (1 << 11);
}

/*======================================================================
 * @0xE1F8 (192 bytes)  Main Loop Tick
 *
 * Called every iteration of the main firmware loop.
 * Checks three interrupt flags (set by LOWFLASH ISRs)
 * and dispatches to the appropriate handler.
 *
 * Processing pipeline:
 *   ADC ISR -> flag_key_interrupt -> key_interrupt_handler (LOWFLASH)
 *   Timer ISR -> flag_periodic_tick -> periodic_key_handler (LOWFLASH)
 *   Key logic -> flag_key_state_update -> key_action_state_machine
 *====================================================================*/
void main_loop_tick(void)
{
    /* Priority 1: Key interrupt (ADC scan complete) */
    if (flag_key_interrupt) {
        flag_key_interrupt = 0;
        key_interrupt_handler();       /* LOWFLASH 0x5114 */
        return;
    }

    /* Priority 2: Periodic tick (timer-based key processing) */
    if (flag_periodic_tick) {
        flag_periodic_tick = 0;
        if (main_state.debounce_counter > 0)
            main_state.debounce_counter--;
        periodic_key_handler();        /* LOWFLASH 0x7500 */
        return;
    }

    /* Priority 3: Key state changed -> process actions */
    if (flag_key_state_update) {
        flag_key_state_update = 0;
        key_action_state_machine();    /* 0x11FAC */

        /* Send USB remote wakeup if device is suspended */
        if (!main_state.usb_suspended
            && usb_state.mode == 4
            && (system_flags & 0x20)
            && !sleep_flag)
        {
            usb_remote_wakeup();       /* LOWFLASH 0x6A90 */
        }

        /* Key repeat / macro countdown */
        if (timer_counters[2] > 0) {
            timer_counters[2]--;
            if (timer_counters[2] == 0)
                main_state.flag_385 = 0;
        }
    }
}

/*======================================================================
 * @0x11FAC (162 bytes)  Key Action State Machine
 *
 * Processes queued key actions using a countdown timer and
 * a bitfield to determine which action to execute.
 * Called from main_loop_tick when flag_key_state_update is set.
 *====================================================================*/
void key_action_state_machine(void)
{
    if (main_state.action_timer == 0) return;

    main_state.action_timer--;
    if (main_state.action_timer != 0) return;

    /* Timer expired -> dispatch by action bitfield */
    uint32_t flags = main_state.action_flags;

    if (flags & 0x01) {
        /* Bit 0: LED commit */
        led_commit_wrapper();
        main_state.action_flags &= ~0x01;
        return;
    }
    if (flags & 0x02) {
        /* Bit 1: Send HID keyboard report */
        key_hid_report_send();          /* LOWFLASH 0x6F98 */
        return;
    }
    if (flags & 0x08) {
        /* Bit 3: Set pending operation flag */
        timer_counters[15] = 1;
        main_state.action_flags &= ~0x08;
        return;
    }
    if (flags & 0x40) {
        /* Bit 6: Write key config to flash */
        flash_write_sector(/*...*/);    /* LOWFLASH 0x4A44 */
        return;
    }
    if ((flags & 0xF00) != 0) {
        /* Bits 8–11: Macro key step execution */
        if (main_state.macro_active && main_state.macro_steps > 0) {
            main_state.macro_steps--;
            main_state.action_timer = 5;  /* 5 ticks between macro steps */
            macro_key_step_execute();      /* LOWFLASH 0x1284 */
        }
    }
}

/*======================================================================
 * @0xC22C (72 bytes)  LED Buffer Fill Cycle
 *
 * Alternates between filling 126-byte LED buffer with 0x00 and
 * 0xFF. Used for blinking/flashing LED effects. After 7 complete
 * cycles, commits the final state to hardware.
 *====================================================================*/
void led_buffer_fill_cycle(void)
{
    uint8_t phase = main_state.blink_phase;  /* +252 */
    uint8_t *buf = (uint8_t *)0x2000493A;    /* LED output buffer */

    if (phase & 1) {
        for (uint8_t i = 0; i < 126; i++) buf[i] = 0x00;  /* OFF */
    } else {
        for (uint8_t i = 0; i < 126; i++) buf[i] = 0xFF;  /* ON */
    }

    main_state.blink_phase = phase + 1;
    if (phase + 1 >= 7)
        led_commit_to_hardware();  /* LOWFLASH 0x6FBC */
}

/*======================================================================
 * FIRMWARE UPDATE MECHANISM
 * (reconstructed from dispatcher + HID descriptor analysis)
 *
 * The keyboard uses a Sonix SN32F248B which has an on-chip boot ROM
 * (0xFFFF0000+) with flash programming routines.
 *
 * Protocol (DO NOT EXECUTE without valid firmware image!):
 *
 *   1. Host sends CMD 0x65 on Interface 2
 *      -> Firmware calls enter_firmware_update_mode(0x65) at 0x24AC
 *      -> Device USB re-enumerates
 *      -> Interface 3 (0xFF67) activates for 4096-byte output reports
 *
 *   2. Host sends 4096-byte blocks via Interface 3 output reports
 *      -> Each block = one flash page
 *      -> 65-byte feature report on Interface 3 used for status/handshake
 *      -> Interface 3 feature report is a simple echo buffer (no processing)
 *
 *   3. Host sends CMD 0x66 on Interface 2
 *      -> Sets g_state.byte2 = 1 (update ready flag)
 *
 *   4. Host sends CMD 0x64 on Interface 2
 *      -> Flash commit  finalizes written data
 *      -> This is what caused the keyboard freeze in Section 4.3 of README
 *        (empty payload -> wrote garbage to flash, undefined state)
 *
 *   5. Host sends CMD 0x67 on Interface 2
 *      -> Clears update flag, returns to normal operation
 *
 * Interface 3 HID descriptor capabilities:
 *   Output Report Size:  4097 bytes (1 report ID + 4096 data)
 *   Feature Report Size: 65 bytes   (1 report ID + 64 data, echo buffer)
 *   Input Report Size:   0 bytes    (no input reports)
 *
 * WARNING: The code at LOWFLASH 0x24AC is not readable and cannot
 * be verified. Sending CMD 0x65 without a complete valid firmware
 * image WILL brick the keyboard, requiring hardware JTAG/SWD recovery.
 *====================================================================*/

/*======================================================================
 * KEY PROCESSING PIPELINE
 *
 * The complete key processing flow, showing the boundary between
 * LOWFLASH (not readable) and our dump (fully reversed):
 *
 * ┌─────────────────────────────────────────────────────────────────┐
 * │ LOWFLASH (0x0000–0x8FFF)  NOT READABLE VIA HID                │
 * │                                                                 │
 * │ Hardware: Hall-effect sensors -> ADC channels                    │
 * │                                                                 │
 * │ 0x5114 key_interrupt_handler:                                   │
 * │   - Triggered by ADC conversion complete interrupt              │
 * │   - Reads ADC values for all hall-effect sensors                │
 * │   - Compares against thresholds in SRAM (from flash tables)     │
 * │   - Updates key state matrix                                    │
 * │   - Sets flag_key_interrupt = 1                                 │
 * │                                                                 │
 * │ 0x7500 periodic_key_handler:                                    │
 * │   - Called on timer tick                                        │
 * │   - Handles debounce timing                                     │
 * │   - Key repeat logic                                            │
 * │   - Sets flag_periodic_tick = 1                                 │
 * │                                                                 │
 * │ 0x6F98 key_hid_report_send:                                     │
 * │   - Assembles 8-byte HID keyboard report                       │
 * │   - Queues for transmission on USB Interface 0                  │
 * │                                                                 │
 * │ 0x1284 macro_key_step_execute:                                  │
 * │   - Executes one step of a key macro sequence                   │
 * │                                                                 │
 * │ Actuation thresholds (factory defaults, ADC counts):            │
 * │   Press:   2200 counts (stored at 0x20002F36, 128 × u16)       │
 * │   Release: 1500 counts (stored at 0x20003036, 128 × u16)       │
 * │   Factory flash backup at 0x8400 (press) and 0x8500 (release)  │
 * │                                                                 │
 * └──────────────────────────┬──────────────────────────────────────┘
 *                            │ SRAM flags
 *                            │ 0x20000076 flag_key_interrupt
 *                            │ 0x20000077 flag_periodic_tick
 *                            │ 0x20000078 flag_key_state_update
 *                            ▼
 * ┌─────────────────────────────────────────────────────────────────┐
 * │ OUR DUMP (0x9000–0x1B5FF)  FULLY REVERSED                     │
 * │                                                                 │
 * │ 0xE1F8 main_loop_tick:                                          │
 * │   ├─ flag_key_interrupt  -> LOWFLASH key_interrupt_handler       │
 * │   ├─ flag_periodic_tick  -> LOWFLASH periodic_key_handler        │
 * │   └─ flag_key_state_update:                                     │
 * │        ├─ 0x11FAC key_action_state_machine                      │
 * │        │   ├─ bit 0  -> LED commit                               │
 * │        │   ├─ bit 1  -> LOWFLASH key_hid_report_send             │
 * │        │   ├─ bit 3  -> set pending flag                         │
 * │        │   ├─ bit 6  -> flash write (save config)                │
 * │        │   └─ bits 8–11 -> macro execution                       │
 * │        └─ USB remote wakeup if needed                           │
 * │                                                                 │
 * │ 0xD504 hid_command_dispatcher:                                  │
 * │   ├─ Read  (0x10–0x18) -> flash read, NO bounds check           │
 * │   ├─ Write (0x21–0x28) -> flash write + SRAM mirror             │
 * │   ├─ System (0x64–0x67) -> commit/DFU/flags                     │
 * │   └─ Factory Reset (sub=1/2/4/5/0xFF)                          │
 * │                                                                 │
 * │ LED Pipeline:                                                   │
 * │   0xBA00 rgb_led_update     -> palette/per-key -> R/G/B buffers  │
 * │   0xC614 led_gamma_update   -> gamma LUT -> PWM buffer           │
 * │   0xC22C led_buffer_fill    -> blink ON/OFF cycles              │
 * │   0xBDA0 keyboard_timer     -> GPIO + countdown timers          │
 * │                                                                 │
 * │ Config:                                                         │
 * │   0xDFF4 reload_device_config -> flash 0x9200 -> SRAM            │
 * └─────────────────────────────────────────────────────────────────┘
 *====================================================================*/