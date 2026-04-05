use std::sync::Arc;
use std::collections::HashSet;

use crossbeam_channel::{Receiver, Sender};
use hidapi::HidDevice;
use parking_lot::Mutex;

use crate::device::connection;
use crate::model::key::{Key, KeyColor};
use crate::model::keyboard::{DriverProtocol, KeyboardState};
use crate::model::layer::Layer;
use crate::protocol::rgb_commands::{self, Transport, LedState, EffectId};
use crate::ui::{draw::DrawList, text::FontAtlas, input::{self, InputState}, theme as t, widgets as w};

// ═══════════════════════════════════════════════════════════════
//  Background worker messages
// ═══════════════════════════════════════════════════════════════

enum BgReq {
    Connect,
    SwitchLayer(Layer),
    Apply(Vec<Option<Key>>),
    ApplyRgb(Vec<Option<Key>>, Transport, Vec<u8>, Vec<u8>, Vec<u8>, Option<LedState>),
    SetEffect(LedState),
}

enum BgResp {
    Connected(HidDevice, KeyboardState),
    LayerSwitched(Layer, Vec<Option<Key>>),
    Applied,
    AppliedRgb(Vec<u8>, Vec<u8>, Vec<u8>, LedState),
    EffectSet(LedState),
    Error(String),
}

#[derive(Clone, Copy)] struct ActuationClip { down: f64, up: f64 }
struct CtxMenu { x: f32, y: f32, key_code: usize, fade_t: f32 }

// ═══════════════════════════════════════════════════════════════
//  App
// ═══════════════════════════════════════════════════════════════

pub struct App {
    tx: Sender<BgReq>, rx: Receiver<BgResp>,
    hid: Arc<Mutex<Option<HidDevice>>>,
    state: Option<KeyboardState>,
    grid: w::GridState,
    show_act: bool, error: Option<String>, busy: bool,
    s_press: w::SliderState, s_release: w::SliderState,
    s_rt_press: w::SliderState, s_rt_release: w::SliderState,
    s_brightness: w::SliderState, s_speed: w::SliderState,
    toggle_rt: w::ToggleState,
    hsv: w::HsvPickerState,
    btn_anim: w::ButtonAnim,
    tab_underline_x: f32,
    scroll_y: f32,
    transition_t: f32,
    ctx_menu: Option<CtxMenu>,
    clipboard: Option<ActuationClip>,
    effect_scroll: f32,
}

impl App {
    pub fn new() -> Self {
        let (rtx, rrx) = crossbeam_channel::unbounded();
        let (stx, srx) = crossbeam_channel::unbounded();
        let hid: Arc<Mutex<Option<HidDevice>>> = Arc::new(Mutex::new(None));
        let hid2 = Arc::clone(&hid);
        std::thread::Builder::new().name("hid".into()).spawn(move || bg_worker(rrx, stx, hid2)).unwrap();
        Self {
            tx: rtx, rx: srx, hid, state: None, grid: Default::default(),
            show_act: false, error: None, busy: false,
            s_press: Default::default(), s_release: Default::default(),
            s_rt_press: Default::default(), s_rt_release: Default::default(),
            s_brightness: Default::default(), s_speed: Default::default(),
            toggle_rt: Default::default(), hsv: Default::default(),
            btn_anim: Default::default(), tab_underline_x: 160.0,
            scroll_y: 0.0, transition_t: 0.0,
            ctx_menu: None, clipboard: None, effect_scroll: 0.0,
        }
    }

    fn key_list(&self) -> Vec<Option<&'static str>> {
        match self.state.as_ref().map(|s| s.config.protocol) {
            Some(DriverProtocol::Rgb) => crate::protocol::key_list::ak680_max_key_list().to_vec(),
            _ => crate::protocol::key_list::ak680_max_lightless_key_list().to_vec(),
        }
    }

    pub fn frame(&mut self, dl: &mut DrawList, fa: &FontAtlas, inp: &InputState) {
        self.poll_bg();
        let (sw, sh) = (inp.width as f32, inp.height as f32);
        dl.clear(inp.width, inp.height);

        if self.state.is_none() {
            self.draw_connect(dl, fa, inp, sw, sh);
        } else {
            self.draw_main(dl, fa, inp, sw, sh);
            if self.transition_t > 0.002 {
                self.transition_t = input::smooth(self.transition_t, 0.0, t::ANIM_NORMAL, inp.dt);
                dl.rect(0.0, 0.0, sw, sh, [t::BG_BASE[0], t::BG_BASE[1], t::BG_BASE[2], self.transition_t]);
            }
        }
    }

    // ─── Background poller ─────────────────────────────────────

    fn poll_bg(&mut self) {
        while let Ok(r) = self.rx.try_recv() {
            self.busy = false;
            match r {
                BgResp::Connected(dev, st) => {
                    *self.hid.lock() = Some(dev);
                    self.state = Some(st); self.error = None; self.transition_t = 1.0;
                }
                BgResp::LayerSwitched(l, keys) => {
                    if let Some(s) = &mut self.state { s.active_layer = l; s.keys = keys; s.has_unsaved_changes = false; }
                }
                BgResp::Applied => {
                    if let Some(s) = &mut self.state { s.has_unsaved_changes = false; }
                }
                BgResp::AppliedRgb(p, r, c, led) => {
                    if let Some(s) = &mut self.state {
                        s.raw_actuation_table = Some(p); s.raw_release_table = Some(r);
                        s.raw_rgb_table = Some(c); s.led_state = Some(led);
                        s.has_unsaved_changes = false;
                    }
                }
                BgResp::EffectSet(led) => {
                    if let Some(s) = &mut self.state { s.led_state = Some(led); }
                }
                BgResp::Error(e) => { log::error!("{e}"); self.error = Some(e); }
            }
        }
    }

    // ─── Connect screen ────────────────────────────────────────

    fn draw_connect(&mut self, dl: &mut DrawList, fa: &FontAtlas, inp: &InputState, sw: f32, sh: f32) {
        let (bw, bh) = (420.0, 230.0);
        let (bx, by) = ((sw - bw) * 0.5, (sh - bh) * 0.5);
        w::card(dl, bx, by, bw, bh);
        fa.draw_centered(dl, "AK680 MAX Driver", bx + bw * 0.5, by + 44.0, t::FONT_TITLE, t::ACCENT);
        fa.draw_centered(dl, "Connect your keyboard via USB", bx + bw * 0.5, by + 80.0, t::FONT_NORMAL, t::TEXT_SEC);
        if let Some(ref e) = self.error {
            dl.rounded_rect(bx + 20.0, by + 105.0, bw - 40.0, 40.0, 6.0, [0.2, 0.07, 0.07, 1.0]);
            fa.draw_text(dl, e, bx + 28.0, by + 113.0, t::FONT_SMALL, t::RED);
        }
        let bx2 = bx + (bw - 120.0) * 0.5;
        let by2 = by + bh - 64.0;
        if w::button(dl, fa, inp, bx2, by2, 120.0, 36.0, "Connect", t::ACCENT, !self.busy) {
            self.busy = true; self.error = None;
            let _ = self.tx.send(BgReq::Connect);
        }
    }

    // ─── Main screen ──────────────────────────────────────────

    fn draw_main(&mut self, dl: &mut DrawList, fa: &FontAtlas, inp: &InputState, sw: f32, sh: f32) {
        let fully = self.state.as_ref().map_or(false, |s| s.fully_supported);
        let is_rgb = self.state.as_ref().map_or(false, |s| s.config.protocol == DriverProtocol::Rgb);

        // ── Top bar ────────────────────────────────────────────
        let surf_light = [t::BG_SURFACE[0] + 0.018, t::BG_SURFACE[1] + 0.018, t::BG_SURFACE[2] + 0.018, 1.0];
        dl.gradient_rect(0.0, 0.0, sw, t::TOP_BAR_H, t::BG_SURFACE, surf_light);
        dl.rect(0.0, t::TOP_BAR_H - 1.0, sw, 1.0, t::BORDER);

        let title_main = "AK680 MAX ";
        fa.draw_text(dl, title_main, 14.0, 16.0, t::FONT_HEADER, t::TEXT);
        let tw = fa.measure(title_main, t::FONT_HEADER).0;
        fa.draw_text(dl, "Driver", 14.0 + tw, 16.0, t::FONT_HEADER, t::TEXT_DIM);

        if fully { self.draw_layer_tabs(dl, fa, inp); }

        if self.state.as_ref().map_or(false, |s| s.has_unsaved_changes) {
            let bt = "Unsaved";
            let bw = fa.measure(bt, t::FONT_SMALL).0 + 16.0;
            dl.rounded_rect(sw - bw - 12.0, 14.0, bw, 20.0, 4.0, [0.15, 0.12, 0.02, 1.0]);
            fa.draw_text(dl, bt, sw - bw - 4.0, 17.0, t::FONT_SMALL, t::AMBER);
        }

        // ── Scrollable content ─────────────────────────────────
        let clip_top = t::TOP_BAR_H;
        let clip_h = sh - t::TOP_BAR_H - t::BOT_BAR_H;
        if inp.in_rect(0.0, clip_top, sw, clip_h) { self.scroll_y -= inp.scroll_delta * 40.0; }

        let mut cy = clip_top + 8.0 - self.scroll_y;

        // Device info
        if let Some(ref st) = self.state {
            fa.draw_text(dl, st.config.name, 14.0, cy, t::FONT_NORMAL, t::TEXT);
            let nw = fa.measure(st.config.name, t::FONT_NORMAL).0;
            let pulse = 3.5 + 0.7 * (inp.elapsed * 3.0).sin();
            dl.circle(14.0 + nw + 14.0, cy + 7.0, pulse, t::GREEN);
            fa.draw_text(dl, "Connected", 14.0 + nw + 24.0, cy, t::FONT_SMALL, t::GREEN);
            if let Some(ref info) = st.device_info {
                let batt = if info.battery_level > 0 { format!("{}%", info.battery_level) } else { "Wired".into() };
                let s = format!("FW {:.2}  |  Bat: {}  |  RT: {}", info.firmware_version, batt, info.rt_precision);
                fa.draw_text(dl, &s, 14.0, cy + 18.0, t::FONT_SMALL, t::TEXT_DIM);
            }
            cy += 42.0;
        }

        if !fully { self.scroll_y = 0.0; return; }

        dl.set_clip(0.0, clip_top, sw, clip_h);

        // ── Keyboard card ──────────────────────────────────────
        cy += 4.0;
        let grid_inner_h = 5.0 * (t::UNIT_PX * 0.84 + t::KEY_GAP) + t::CARD_PAD * 2.0;
        w::card(dl, 8.0, cy, sw - 16.0, grid_inner_h);

        let kl = self.key_list();
        let gr = if let Some(ref st) = self.state {
            w::keyboard_grid(dl, fa, inp, &st.keys, &mut self.grid, self.show_act, &kl,
                t::CARD_PAD + 8.0, cy + t::CARD_PAD, sw - 32.0 - t::CARD_PAD * 2.0)
        } else {
            w::GridResult { height: 0.0, hovered: None, right_click: None }
        };
        cy += grid_inner_h + 12.0;

        // ── Options card ───────────────────────────────────────
        let opts_h = if self.grid.selected.is_empty() { 70.0 } else { 200.0 };
        w::card(dl, 8.0, cy, sw - 16.0, opts_h);

        if self.grid.selected.is_empty() {
            fa.draw_centered(dl, "Click or drag keys to select", sw * 0.5, cy + opts_h * 0.5, t::FONT_NORMAL, t::TEXT_DIM);
        } else if let Some(ref mut st) = self.state {
            let sel = &self.grid.selected;
            if let Some(snap) = first_snap(&st.keys, sel) {
                let inner_x = 8.0 + t::CARD_PAD;
                let inner_w = sw - 16.0 - t::CARD_PAD * 2.0;
                let col_gap = 12.0;
                let sep_w = 1.0;
                let col_w = (inner_w - 2.0 * (col_gap * 2.0 + sep_w)) / 3.0;
                let oy = cy + t::CARD_PAD;

                let cnt = sel.len();
                let label = if cnt == 1 { "1 key".to_owned() } else { format!("{cnt} keys") };

                // Actuation column
                let ax = inner_x;
                w::mini_card(dl, ax, oy, col_w, opts_h - t::CARD_PAD * 2.0);
                fa.draw_text(dl, &label, ax + 10.0, oy + 8.0, t::FONT_SMALL, t::ACCENT);
                fa.draw_text(dl, "Actuation", ax + 10.0, oy + 24.0, t::FONT_SMALL, t::TEXT_DIM);
                let mut pv = snap.down_actuation;
                let mut rv = snap.up_actuation;
                if w::slider(dl, fa, inp, &mut self.s_press, ax + 10.0, oy + 42.0, col_w - 20.0, &mut pv,
                    st.config.min_actuation, st.config.max_actuation, 0.01, "Press", t::PRESS_COL) {
                    for &c in sel { if let Some(Some(k)) = st.keys.get_mut(c) { k.down_actuation = pv; } }
                    st.has_unsaved_changes = true;
                }
                if w::slider(dl, fa, inp, &mut self.s_release, ax + 10.0, oy + 74.0, col_w - 20.0, &mut rv,
                    st.config.min_actuation, st.config.max_actuation, 0.01, "Release", t::RELEASE_COL) {
                    for &c in sel { if let Some(Some(k)) = st.keys.get_mut(c) { k.up_actuation = rv; } }
                    st.has_unsaved_changes = true;
                }

                let sep1x = ax + col_w + col_gap;
                dl.rect(sep1x, oy + 6.0, sep_w, opts_h - t::CARD_PAD * 2.0 - 12.0, t::SEPARATOR);

                // RT column
                let rtx = sep1x + col_gap + sep_w;
                w::mini_card(dl, rtx, oy, col_w, opts_h - t::CARD_PAD * 2.0);
                fa.draw_text(dl, "Rapid Trigger", rtx + 10.0, oy + 8.0, t::FONT_SMALL, t::TEXT_DIM);
                let mut rt_on = snap.rapid_trigger;
                if w::toggle_anim(dl, inp, &mut self.toggle_rt, rtx + col_w - 46.0, oy + 6.0, &mut rt_on) {
                    for &c in sel { if let Some(Some(k)) = st.keys.get_mut(c) { k.rapid_trigger = rt_on; } }
                    st.has_unsaved_changes = true;
                }
                if rt_on {
                    let mut rtp = snap.rt_press;
                    let mut rtr = snap.rt_release;
                    if w::slider(dl, fa, inp, &mut self.s_rt_press, rtx + 10.0, oy + 42.0, col_w - 20.0, &mut rtp,
                        st.config.rt_min_sensitivity, st.config.rt_max_sensitivity, 0.01, "Press", t::PRESS_COL) {
                        for &c in sel { if let Some(Some(k)) = st.keys.get_mut(c) { k.rt_press_sensitivity = rtp; } }
                        st.has_unsaved_changes = true;
                    }
                    if w::slider(dl, fa, inp, &mut self.s_rt_release, rtx + 10.0, oy + 74.0, col_w - 20.0, &mut rtr,
                        st.config.rt_min_sensitivity, st.config.rt_max_sensitivity, 0.01, "Release", t::RELEASE_COL) {
                        for &c in sel { if let Some(Some(k)) = st.keys.get_mut(c) { k.rt_release_sensitivity = rtr; } }
                        st.has_unsaved_changes = true;
                    }
                }

                let sep2x = rtx + col_w + col_gap;
                dl.rect(sep2x, oy + 6.0, sep_w, opts_h - t::CARD_PAD * 2.0 - 12.0, t::SEPARATOR);

                // Colour column
                let ccx = sep2x + col_gap + sep_w;
                let mc_h = opts_h - t::CARD_PAD * 2.0;
                w::mini_card(dl, ccx, oy, col_w, mc_h);
                fa.draw_text(dl, "Key Color", ccx + 10.0, oy + 8.0, t::FONT_SMALL, t::TEXT_DIM);

                let wheel_r = (col_w * 0.36).min(55.0);
                let wheel_cx = ccx + col_w * 0.5;
                let wheel_cy = oy + 34.0 + wheel_r;

                if !self.hsv.is_dragging() {
                    if let Some(k) = sel.iter().find_map(|&c| st.keys.get(c)?.as_ref()) {
                        self.hsv.sync_from_rgb(k.color.r, k.color.g, k.color.b);
                    }
                }

                if w::hsv_picker(dl, fa, inp, &mut self.hsv, wheel_cx, wheel_cy, wheel_r) {
                    let (r, g, b) = self.hsv.to_rgb();
                    let nc = KeyColor { r, g, b };
                    for &c in sel { if let Some(Some(k)) = st.keys.get_mut(c) { k.color = nc; } }
                    st.has_unsaved_changes = true;
                }

                if w::color_presets(dl, fa, inp, &mut st.keys, sel, ccx + 10.0, oy + mc_h - 32.0) {
                    st.has_unsaved_changes = true;
                }
            }
        }
        cy += opts_h + 12.0;

        // ── Effect card (RGB only) ─────────────────────────────
        if is_rgb {
            cy = self.draw_effect_card(dl, fa, inp, cy, sw);
        }

        let content_h = cy + 12.0 - (clip_top - self.scroll_y);
        self.scroll_y = self.scroll_y.clamp(0.0, (content_h - clip_h).max(0.0));

        dl.reset_clip(inp.width, inp.height);

        // Scrollbar
        if content_h > clip_h {
            let bar_h = (clip_h * clip_h / content_h).max(20.0);
            let bar_y = clip_top + self.scroll_y / content_h * clip_h;
            dl.rounded_rect(sw - 6.0, bar_y, 4.0, bar_h, 2.0, [1.0, 1.0, 1.0, 0.12]);
        }

        // ── Bottom bar ─────────────────────────────────────────
        let by = sh - t::BOT_BAR_H;
        dl.rect(0.0, by, sw, t::BOT_BAR_H, t::BG_SURFACE);
        dl.rect(0.0, by, sw, 1.0, t::BORDER);

        let has_ch = self.state.as_ref().map_or(false, |s| s.has_unsaved_changes);
        if w::button_anim(dl, fa, inp, &mut self.btn_anim, 14.0, by + 10.0, 110.0, 32.0,
            "\u{2713} Apply", t::ACCENT, has_ch && !self.busy) {
            self.send_apply();
        }
        if w::button_anim(dl, fa, inp, &mut self.btn_anim, 132.0, by + 10.0, 110.0, 32.0,
            "\u{25C6} Select All", t::BG_RAISED, true) {
            let kl = self.key_list();
            self.grid.select_all(&kl);
        }
        let atog = if self.show_act { "\u{25B8} Hide Values" } else { "\u{25B8} Show Values" };
        if w::button_anim(dl, fa, inp, &mut self.btn_anim, 250.0, by + 10.0, 120.0, 32.0, atog, t::BG_RAISED, true) {
            self.show_act = !self.show_act;
        }

        // Tooltip
        if let Some(ref hk) = gr.hovered {
            if self.ctx_menu.is_none() {
                if let Some(ref st) = self.state { w::draw_tooltip(dl, fa, &st.keys, hk); }
            }
        }
        // Context menu
        if let Some((code, mx, my)) = gr.right_click {
            self.ctx_menu = Some(CtxMenu { x: mx, y: my, key_code: code, fade_t: 0.0 });
        }
        self.draw_ctx_menu(dl, fa, inp);
    }

    // ─── Effect card ───────────────────────────────────────────

    fn draw_effect_card(&mut self, dl: &mut DrawList, fa: &FontAtlas, inp: &InputState, cy: f32, sw: f32) -> f32 {
        let card_h = 110.0;
        w::card(dl, 8.0, cy, sw - 16.0, card_h);

        let inner_x = 8.0 + t::CARD_PAD;
        let oy = cy + t::CARD_PAD;
        fa.draw_text(dl, "LED Effect", inner_x, oy, t::FONT_SMALL, t::TEXT_DIM);

        // Current effect info
        let cur_effect = self.state.as_ref()
            .and_then(|s| s.led_state)
            .map(|l| l.effect)
            .unwrap_or(EffectId::Off);
        let cur_name = cur_effect.name();
        fa.draw_text(dl, cur_name, inner_x + 80.0, oy, t::FONT_SMALL, t::ACCENT);

        // Effect buttons (scrollable row)
        let btn_y = oy + 20.0;
        let btn_h = 26.0;
        let gap = 4.0;
        let mut bx = inner_x;

        // Off button
        let off_active = cur_effect == EffectId::Off;
        let off_w = fa.measure("Off", t::FONT_SMALL).0 + 16.0;
        let off_bg = if off_active { t::ACCENT } else { t::BG_KEY };
        let off_tc = if off_active { t::TEXT } else { t::TEXT_SEC };
        dl.rounded_rect(bx, btn_y, off_w, btn_h, 4.0, off_bg);
        fa.draw_centered(dl, "Off", bx + off_w * 0.5, btn_y + btn_h * 0.5, t::FONT_SMALL, off_tc);
        if inp.clicked() && inp.in_rect(bx, btn_y, off_w, btn_h) && !self.busy {
            self.send_effect(LedState::off());
        }
        bx += off_w + gap;

        // Custom button
        let cust_active = cur_effect == EffectId::CustomColors;
        let cust_w = fa.measure("Custom", t::FONT_SMALL).0 + 16.0;
        let cust_bg = if cust_active { t::ACCENT } else { t::BG_KEY };
        let cust_tc = if cust_active { t::TEXT } else { t::TEXT_SEC };
        dl.rounded_rect(bx, btn_y, cust_w, btn_h, 4.0, cust_bg);
        fa.draw_centered(dl, "Custom", bx + cust_w * 0.5, btn_y + btn_h * 0.5, t::FONT_SMALL, cust_tc);
        if inp.clicked() && inp.in_rect(bx, btn_y, cust_w, btn_h) && !self.busy {
            let br = self.state.as_ref().and_then(|s| s.led_state).map(|l| l.brightness.max(1)).unwrap_or(5);
            self.send_effect(LedState::custom_colors(br));
        }
        bx += cust_w + gap + 8.0;

        // Separator
        dl.rect(bx, btn_y + 2.0, 1.0, btn_h - 4.0, t::SEPARATOR);
        bx += 9.0;

        // Effect list
        for eff in EffectId::UI_LIST {
            let name = eff.name();
            let ew = fa.measure(name, t::FONT_SMALL).0 + 14.0;
            let active = cur_effect == eff;
            let hov = inp.in_rect(bx, btn_y, ew, btn_h);
            let bg = if active { t::ACCENT }
                     else if hov { t::BG_KEY_HOVER }
                     else { t::BG_KEY };
            let tc = if active { t::TEXT }
                     else if hov { t::TEXT }
                     else { t::TEXT_SEC };
            dl.rounded_rect(bx, btn_y, ew, btn_h, 4.0, bg);
            fa.draw_centered(dl, name, bx + ew * 0.5, btn_y + btn_h * 0.5, t::FONT_SMALL, tc);
            if inp.clicked() && hov && !self.busy {
                let br = self.state.as_ref().and_then(|s| s.led_state).map(|l| l.brightness.max(1)).unwrap_or(5);
                self.send_effect(LedState::new(eff, br, 3));
            }
            bx += ew + gap;
        }

        // Brightness & Speed sliders
        let slider_y = btn_y + btn_h + 10.0;
        let slider_w = (sw - 16.0 - t::CARD_PAD * 2.0 - 20.0) * 0.5;

        let mut br_val = self.state.as_ref()
            .and_then(|s| s.led_state).map(|l| l.brightness as f64).unwrap_or(5.0);
        if w::slider(dl, fa, inp, &mut self.s_brightness, inner_x, slider_y, slider_w, &mut br_val,
            1.0, 5.0, 1.0, "Bright", t::ACCENT) {
            if let Some(ref st) = self.state {
                if let Some(led) = st.led_state {
                    let mut new_led = led;
                    new_led.brightness = br_val.round() as u8;
                    self.send_effect(new_led);
                }
            }
        }

        let mut sp_val = self.state.as_ref()
            .and_then(|s| s.led_state).map(|l| l.speed as f64).unwrap_or(3.0);
        if w::slider(dl, fa, inp, &mut self.s_speed, inner_x + slider_w + 20.0, slider_y, slider_w, &mut sp_val,
            1.0, 5.0, 1.0, "Speed", t::GREEN) {
            if let Some(ref st) = self.state {
                if let Some(led) = st.led_state {
                    let mut new_led = led;
                    new_led.speed = sp_val.round() as u8;
                    self.send_effect(new_led);
                }
            }
        }

        cy + card_h + 12.0
    }

    // ─── Layer tabs ────────────────────────────────────────────

    fn draw_layer_tabs(&mut self, dl: &mut DrawList, fa: &FontAtlas, inp: &InputState) {
        let mut lx = 160.0;
        let mut active_lx = lx;
        for layer in Layer::ALL {
            let active = self.state.as_ref().map_or(false, |s| s.active_layer == layer);
            let bg = if active { t::ACCENT_DIM } else { [0.0; 4] };
            let tc = if active { t::ACCENT } else { t::TEXT_SEC };
            dl.rounded_rect(lx, 10.0, 64.0, 28.0, 6.0, bg);
            fa.draw_centered(dl, layer.display_name(), lx + 32.0, 24.0, t::FONT_SMALL, tc);
            if active { active_lx = lx; }
            if inp.clicked() && inp.in_rect(lx, 10.0, 64.0, 28.0) && !active && !self.busy {
                self.busy = true;
                let _ = self.tx.send(BgReq::SwitchLayer(layer));
            }
            lx += 70.0;
        }
        self.tab_underline_x = input::smooth(self.tab_underline_x, active_lx, t::ANIM_NORMAL, inp.dt);
        dl.rounded_rect(self.tab_underline_x + 12.0, 37.0, 40.0, 2.5, 1.0, t::ACCENT);
    }

    // ─── Context menu ──────────────────────────────────────────

    fn draw_ctx_menu(&mut self, dl: &mut DrawList, fa: &FontAtlas, inp: &InputState) {
        let mut menu = match self.ctx_menu.take() { Some(m) => m, None => return };
        menu.fade_t = input::smooth(menu.fade_t, 1.0, t::ANIM_FAST, inp.dt);
        let a = menu.fade_t;
        let items = ["Copy Actuation", "Paste Actuation", "Reset to Default"];
        let (iw, ih) = (172.0f32, 28.0f32);
        let h = items.len() as f32 * ih + 8.0;
        let (mx, my) = (menu.x, menu.y);

        dl.rounded_rect(mx + 3.0, my + 3.0, iw, h, 8.0, [0.0, 0.0, 0.0, 0.25 * a]);
        dl.rounded_rect(mx, my, iw, h, 8.0, t::with_alpha(t::BG_SURFACE, a));
        dl.rounded_rect(mx, my, iw, h, 8.0, t::with_alpha(t::CARD_BORDER, a));
        dl.rounded_rect(mx + 1.0, my + 1.0, iw - 2.0, h - 2.0, 7.0, t::with_alpha(t::BG_SURFACE, a));

        let mut action: Option<usize> = None;
        for (i, &lab) in items.iter().enumerate() {
            let iy = my + 4.0 + i as f32 * ih;
            let hov = inp.in_rect(mx, iy, iw, ih);
            if hov { dl.rounded_rect(mx + 4.0, iy + 1.0, iw - 8.0, ih - 2.0, 4.0, [1.0, 1.0, 1.0, 0.06 * a]); }
            fa.draw_text(dl, lab, mx + 12.0, iy + 7.0, t::FONT_NORMAL, t::with_alpha(if hov { t::TEXT } else { t::TEXT_SEC }, a));
            if hov && inp.clicked() { action = Some(i); }
        }

        let close = action.is_some()
            || (inp.clicked() && !inp.in_rect(mx, my, iw, h))
            || (inp.right_clicked() && !inp.in_rect(mx, my, iw, h));

        if let Some(i) = action {
            let kc = menu.key_code;
            match i {
                0 => {
                    if let Some(ref st) = self.state {
                        if let Some(Some(k)) = st.keys.get(kc) {
                            self.clipboard = Some(ActuationClip { down: k.down_actuation, up: k.up_actuation });
                        }
                    }
                }
                1 => {
                    if let (Some(clip), Some(ref mut st)) = (self.clipboard, self.state.as_mut()) {
                        let tgt: Vec<usize> = if self.grid.selected.is_empty() { vec![kc] }
                                               else { self.grid.selected.iter().copied().collect() };
                        for c in tgt { if let Some(Some(k)) = st.keys.get_mut(c) { k.down_actuation = clip.down; k.up_actuation = clip.up; } }
                        st.has_unsaved_changes = true;
                    }
                }
                2 => {
                    if let Some(ref mut st) = self.state {
                        let tgt: Vec<usize> = if self.grid.selected.is_empty() { vec![kc] }
                                               else { self.grid.selected.iter().copied().collect() };
                        for c in tgt { if let Some(Some(k)) = st.keys.get_mut(c) { k.down_actuation = 1.20; k.up_actuation = 1.20; } }
                        st.has_unsaved_changes = true;
                    }
                }
                _ => {}
            }
        }

        if !close { self.ctx_menu = Some(menu); }
    }

    // ─── Send helpers ──────────────────────────────────────────

    fn send_apply(&mut self) {
        let Some(ref st) = self.state else { return };
        self.busy = true;
        if let (Some(tr), Some(p), Some(r), Some(c)) = (st.transport, &st.raw_actuation_table, &st.raw_release_table, &st.raw_rgb_table) {
            let _ = self.tx.send(BgReq::ApplyRgb(st.keys.clone(), tr, p.clone(), r.clone(), c.clone(), st.led_state));
        } else {
            let _ = self.tx.send(BgReq::Apply(st.keys.clone()));
        }
    }

    fn send_effect(&mut self, led: LedState) {
        if self.busy { return; }
        self.busy = true;
        let _ = self.tx.send(BgReq::SetEffect(led));
    }
}

// ─── Snapshot helper ───────────────────────────────────────────

struct Snap { down_actuation: f64, up_actuation: f64, rapid_trigger: bool, rt_press: f64, rt_release: f64 }

fn first_snap(keys: &[Option<Key>], sel: &HashSet<usize>) -> Option<Snap> {
    let k = sel.iter().find_map(|&c| keys.get(c)?.as_ref())?;
    Some(Snap { down_actuation: k.down_actuation, up_actuation: k.up_actuation,
        rapid_trigger: k.rapid_trigger, rt_press: k.rt_press_sensitivity, rt_release: k.rt_release_sensitivity })
}

// ─── Background worker ────────────────────────────────────────

fn bg_worker(rx: Receiver<BgReq>, tx: Sender<BgResp>, hid: Arc<Mutex<Option<HidDevice>>>) {
    while let Ok(req) = rx.recv() {
        match req {
            BgReq::Connect => match connection::connect() {
                Ok((d, s)) => { let _ = tx.send(BgResp::Connected(d, s)); }
                Err(e) => { let _ = tx.send(BgResp::Error(format!("{e}"))); }
            },
            BgReq::SwitchLayer(l) => {
                let g = hid.lock();
                if let Some(ref d) = *g {
                    match connection::switch_layer(d, l) {
                        Ok(k) => { let _ = tx.send(BgResp::LayerSwitched(l, k)); }
                        Err(e) => { let _ = tx.send(BgResp::Error(format!("{e}"))); }
                    }
                }
            }
            BgReq::Apply(keys) => {
                let g = hid.lock();
                if let Some(ref d) = *g {
                    match connection::apply_all_keys(d, &keys) {
                        Ok(()) => { let _ = tx.send(BgResp::Applied); }
                        Err(e) => { let _ = tx.send(BgResp::Error(format!("{e}"))); }
                    }
                }
            }
            BgReq::ApplyRgb(keys, tr, p, r, c, led) => {
                let g = hid.lock();
                if let Some(ref d) = *g {
                    match connection::apply_rgb_keys(d, &keys, tr, &p, &r, &c, led) {
                        Ok((np, nr, nc, new_led)) => { let _ = tx.send(BgResp::AppliedRgb(np, nr, nc, new_led)); }
                        Err(e) => { let _ = tx.send(BgResp::Error(format!("{e}"))); }
                    }
                }
            }
            BgReq::SetEffect(led) => {
                let g = hid.lock();
                if let Some(ref d) = *g {
                    let transport = rgb_commands::Transport::OutputReport; // default
                    match rgb_commands::set_led_state(d, transport, &led) {
                        Ok(()) => { let _ = tx.send(BgResp::EffectSet(led)); }
                        Err(e) => { let _ = tx.send(BgResp::Error(format!("{e}"))); }
                    }
                }
            }
        }
    }
}