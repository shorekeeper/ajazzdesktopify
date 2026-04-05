/// Sparse key-code-to-name mappings for both keyboard variants.
///
/// Firmware key codes are sparse indices (not all values 0-127 correspond
/// to physical keys). These tables map each valid index to a string
/// identifier used by the layout module.
///
/// # RGB model key layout (68 keys)
///
/// ```text
/// Row 0:  0=Esc  17-28=1-0,-,=  92=Bksp  104=Home
/// Row 1: 32=Tab  33-44=Q-]  60=\  106=Del
/// Row 2: 48=Caps 49-59=A-'  76=Enter  105=PgUp
/// Row 3: 64=LShift 65-74=Z-/ 75=RShift 90=Up 108=PgDn
/// Row 4: 80=LCtrl 81=Win 82=LAlt 83=Space 84-91=RAlt..Arrows
/// ```

pub const KEY_SLOT_COUNT: usize = 112;
pub const RGB_KEY_SLOT_COUNT: usize = 112;

/// Lightless (no-RGB) model, VID `0x3151` PID `0x502C`.
pub fn ak680_max_lightless_key_list() -> [Option<&'static str>; KEY_SLOT_COUNT] {
    let mut l: [Option<&'static str>; KEY_SLOT_COUNT] = [None; KEY_SLOT_COUNT];
    l[0] = Some("Escape");    l[1] = Some("Tab");
    l[2] = Some("CapsLock");  l[3] = Some("ShiftLeft");
    l[4] = Some("ControlLeft");
    l[6]  = Some("Digit1");   l[7]  = Some("KeyQ");
    l[8]  = Some("KeyA");     l[12] = Some("Digit2");
    l[13] = Some("KeyW");     l[14] = Some("KeyS");
    l[15] = Some("KeyZ");     l[16] = Some("MetaLeft");
    l[18] = Some("Digit3");   l[19] = Some("KeyE");
    l[20] = Some("KeyD");     l[21] = Some("KeyX");
    l[22] = Some("AltLeft");  l[24] = Some("Digit4");
    l[25] = Some("KeyR");     l[26] = Some("KeyF");
    l[27] = Some("KeyC");     l[30] = Some("Digit5");
    l[31] = Some("KeyT");     l[32] = Some("KeyG");
    l[33] = Some("KeyV");     l[36] = Some("Digit6");
    l[37] = Some("KeyY");     l[38] = Some("KeyH");
    l[39] = Some("KeyB");     l[40] = Some("Space");
    l[42] = Some("Digit7");   l[43] = Some("KeyU");
    l[44] = Some("KeyJ");     l[45] = Some("KeyN");
    l[48] = Some("Digit8");   l[49] = Some("KeyI");
    l[50] = Some("KeyK");     l[51] = Some("KeyM");
    l[54] = Some("Digit9");   l[55] = Some("KeyO");
    l[56] = Some("KeyL");     l[57] = Some("Comma");
    l[58] = Some("AltRight"); l[60] = Some("Digit0");
    l[61] = Some("KeyP");     l[62] = Some("Semicolon");
    l[63] = Some("Period");   l[64] = Some("Fn");
    l[66] = Some("Minus");    l[67] = Some("BracketLeft");
    l[68] = Some("Quote");    l[69] = Some("Slash");
    l[70] = Some("ControlRight"); l[72] = Some("Equal");
    l[73] = Some("BracketRight"); l[75] = Some("ShiftRight");
    l[76] = Some("ArrowLeft"); l[78] = Some("Backspace");
    l[79] = Some("Backslash"); l[80] = Some("Enter");
    l[81] = Some("ArrowUp");  l[82] = Some("ArrowDown");
    l[84] = Some("Home");     l[85] = Some("Delete");
    l[86] = Some("PageUp");   l[87] = Some("PageDown");
    l[88] = Some("ArrowRight");
    l
}

/// RGB model, VID `0x0C45` PID `0x80B2`.
pub fn ak680_max_key_list() -> [Option<&'static str>; RGB_KEY_SLOT_COUNT] {
    let mut l: [Option<&'static str>; RGB_KEY_SLOT_COUNT] = [None; RGB_KEY_SLOT_COUNT];
    // Row 0
    l[0]  = Some("Escape");
    l[17] = Some("Digit1");   l[18] = Some("Digit2");
    l[19] = Some("Digit3");   l[20] = Some("Digit4");
    l[21] = Some("Digit5");   l[22] = Some("Digit6");
    l[23] = Some("Digit7");   l[24] = Some("Digit8");
    l[25] = Some("Digit9");   l[26] = Some("Digit0");
    l[27] = Some("Minus");    l[28] = Some("Equal");
    // Row 1
    l[32] = Some("Tab");      l[33] = Some("KeyQ");
    l[34] = Some("KeyW");     l[35] = Some("KeyE");
    l[36] = Some("KeyR");     l[37] = Some("KeyT");
    l[38] = Some("KeyY");     l[39] = Some("KeyU");
    l[40] = Some("KeyI");     l[41] = Some("KeyO");
    l[42] = Some("KeyP");     l[43] = Some("BracketLeft");
    l[44] = Some("BracketRight");
    // Row 2
    l[48] = Some("CapsLock"); l[49] = Some("KeyA");
    l[50] = Some("KeyS");     l[51] = Some("KeyD");
    l[52] = Some("KeyF");     l[53] = Some("KeyG");
    l[54] = Some("KeyH");     l[55] = Some("KeyJ");
    l[56] = Some("KeyK");     l[57] = Some("KeyL");
    l[58] = Some("Semicolon"); l[59] = Some("Quote");
    l[60] = Some("Backslash");
    // Row 3
    l[64] = Some("ShiftLeft"); l[65] = Some("KeyZ");
    l[66] = Some("KeyX");     l[67] = Some("KeyC");
    l[68] = Some("KeyV");     l[69] = Some("KeyB");
    l[70] = Some("KeyN");     l[71] = Some("KeyM");
    l[72] = Some("Comma");    l[73] = Some("Period");
    l[74] = Some("Slash");    l[75] = Some("ShiftRight");
    l[76] = Some("Enter");
    // Row 4
    l[80] = Some("ControlLeft"); l[81] = Some("MetaLeft");
    l[82] = Some("AltLeft");  l[83] = Some("Space");
    l[84] = Some("AltRight"); l[85] = Some("Fn");
    l[87] = Some("ControlRight"); l[88] = Some("ArrowLeft");
    l[89] = Some("ArrowDown"); l[90] = Some("ArrowUp");
    l[91] = Some("ArrowRight");
    // Row 0 extras
    l[92]  = Some("Backspace"); l[104] = Some("Home");
    // Row 1 extras
    l[105] = Some("PageUp");  l[106] = Some("Delete");
    // Row 3 extras
    l[108] = Some("PageDown");
    l
}