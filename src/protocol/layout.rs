/// Physical key positions on the 68% AK680 layout.
///
/// Coordinates are in "keyboard units" (1U = one standard key width).
/// The layout has 16 columns and 5 rows.

#[derive(Debug, Clone, Copy)]
pub struct KeyLayout {
    pub name: &'static str,
    pub width: f32,
    pub column: f32,
    pub row: u8,
}

/// Look up physical layout for a key by its string identifier.
pub fn get_key_layout(id: &str) -> Option<KeyLayout> {
    let (n, w, c, r) = match id {
        "Escape"    => ("Esc",    1.0,   0.0,   0),
        "Digit1"    => ("1 !",    1.0,   1.0,   0),
        "Digit2"    => ("2 @",    1.0,   2.0,   0),
        "Digit3"    => ("3 #",    1.0,   3.0,   0),
        "Digit4"    => ("4 $",    1.0,   4.0,   0),
        "Digit5"    => ("5 %",    1.0,   5.0,   0),
        "Digit6"    => ("6 ^",    1.0,   6.0,   0),
        "Digit7"    => ("7 &",    1.0,   7.0,   0),
        "Digit8"    => ("8 *",    1.0,   8.0,   0),
        "Digit9"    => ("9 (",    1.0,   9.0,   0),
        "Digit0"    => ("0 )",    1.0,  10.0,   0),
        "Minus"     => ("- _",    1.0,  11.0,   0),
        "Equal"     => ("= +",    1.0,  12.0,   0),
        "Backspace" => ("Bksp",   2.0,  13.0,   0),
        "Home"      => ("Home",   1.0,  15.0,   0),

        "Tab"          => ("Tab",  1.5,  0.0,  1),
        "KeyQ"         => ("Q",    1.0,  1.5,  1),
        "KeyW"         => ("W",    1.0,  2.5,  1),
        "KeyE"         => ("E",    1.0,  3.5,  1),
        "KeyR"         => ("R",    1.0,  4.5,  1),
        "KeyT"         => ("T",    1.0,  5.5,  1),
        "KeyY"         => ("Y",    1.0,  6.5,  1),
        "KeyU"         => ("U",    1.0,  7.5,  1),
        "KeyI"         => ("I",    1.0,  8.5,  1),
        "KeyO"         => ("O",    1.0,  9.5,  1),
        "KeyP"         => ("P",    1.0, 10.5,  1),
        "BracketLeft"  => ("[",    1.0, 11.5,  1),
        "BracketRight" => ("]",    1.0, 12.5,  1),
        "Backslash"    => ("\\",   1.5, 13.5,  1),
        "Delete"       => ("Del",  1.0, 15.0,  1),

        "CapsLock"  => ("Caps",   1.75,  0.0,  2),
        "KeyA"      => ("A",      1.0,   1.75, 2),
        "KeyS"      => ("S",      1.0,   2.75, 2),
        "KeyD"      => ("D",      1.0,   3.75, 2),
        "KeyF"      => ("F",      1.0,   4.75, 2),
        "KeyG"      => ("G",      1.0,   5.75, 2),
        "KeyH"      => ("H",      1.0,   6.75, 2),
        "KeyJ"      => ("J",      1.0,   7.75, 2),
        "KeyK"      => ("K",      1.0,   8.75, 2),
        "KeyL"      => ("L",      1.0,   9.75, 2),
        "Semicolon" => (";",      1.0,  10.75, 2),
        "Quote"     => ("'",      1.0,  11.75, 2),
        "Enter"     => ("Enter",  2.25, 12.75, 2),
        "PageUp"    => ("PgUp",   1.0,  15.0,  2),

        "ShiftLeft"  => ("Shift", 2.25, 0.0,   3),
        "KeyZ"       => ("Z",     1.0,  2.25,  3),
        "KeyX"       => ("X",     1.0,  3.25,  3),
        "KeyC"       => ("C",     1.0,  4.25,  3),
        "KeyV"       => ("V",     1.0,  5.25,  3),
        "KeyB"       => ("B",     1.0,  6.25,  3),
        "KeyN"       => ("N",     1.0,  7.25,  3),
        "KeyM"       => ("M",     1.0,  8.25,  3),
        "Comma"      => (", <",   1.0,  9.25,  3),
        "Period"     => (". >",   1.0, 10.25,  3),
        "Slash"      => ("/ ?",   1.0, 11.25,  3),
        "ShiftRight" => ("Shift", 1.75,12.25,  3),
        "ArrowUp"    => ("Up",    1.0, 14.0,   3),
        "PageDown"   => ("PgDn",  1.0, 15.0,   3),

        "ControlLeft"  => ("Ctrl",  1.25, 0.0,  4),
        "MetaLeft"     => ("Win",   1.25, 1.25, 4),
        "AltLeft"      => ("Alt",   1.25, 2.5,  4),
        "Space"        => ("Space", 6.25, 3.75, 4),
        "AltRight"     => ("Alt",   1.0, 10.0,  4),
        "Fn"           => ("Fn",    1.0, 11.0,  4),
        "ControlRight" => ("Ctrl",  1.0, 12.0,  4),
        "ArrowLeft"    => ("Left",  1.0, 13.0,  4),
        "ArrowDown"    => ("Down",  1.0, 14.0,  4),
        "ArrowRight"   => ("Right", 1.0, 15.0,  4),

        _ => return None,
    };
    Some(KeyLayout { name: n, width: w, column: c, row: r })
}