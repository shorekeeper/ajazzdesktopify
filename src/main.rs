#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ui;

pub use ak680max_driver::device;
pub use ak680max_driver::model;
pub use ak680max_driver::protocol;

use std::ffi::c_void;
use std::ptr::null_mut;
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::ui::{app::App, draw::DrawList, input::InputState, text::FontAtlas, vulkan::Vk};

const CLASS_NAME: &[u16] = &[b'A' as u16, b'K' as u16, b'6' as u16, b'8' as u16, b'0' as u16, 0];
const INIT_W: u32 = 1180;
const INIT_H: u32 = 740;

static mut INPUT: InputState = InputState {
    mouse_x: 0.0, mouse_y: 0.0,
    mouse_down: false, mouse_was_down: false,
    right_down: false, right_was_down: false,
    scroll_delta: 0.0,
    width: INIT_W, height: INIT_H,
    resized: false, dt: 0.016,
    elapsed: 0.0,
};

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    log::info!("Starting AK680 MAX Driver (Vulkan / Adwaita)");

    let hinstance = unsafe { GetModuleHandleW(std::ptr::null()) };

    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wnd_proc),
        hInstance: hinstance,
        hCursor: unsafe { LoadCursorW(null_mut(), IDC_ARROW) },
        hbrBackground: null_mut(),
        lpszClassName: CLASS_NAME.as_ptr(),
        cbClsExtra: 0, cbWndExtra: 0,
        hIcon: null_mut(), hIconSm: null_mut(),
        lpszMenuName: std::ptr::null(),
    };
    unsafe { RegisterClassExW(&wc) };

    let title: Vec<u16> = "AK680 MAX Driver".encode_utf16().chain(Some(0)).collect();
    let hwnd = unsafe {
        CreateWindowExW(0, CLASS_NAME.as_ptr(), title.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT, CW_USEDEFAULT, INIT_W as i32, INIT_H as i32,
            null_mut(), null_mut(), hinstance, std::ptr::null())
    };
    assert!(!hwnd.is_null(), "CreateWindowExW failed");

    let mut rc = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    unsafe { GetClientRect(hwnd, &mut rc) };
    let (cw, ch) = ((rc.right - rc.left) as u32, (rc.bottom - rc.top) as u32);
    unsafe { INPUT.width = cw; INPUT.height = ch; }

    let mut vk = Vk::new(hwnd as isize, hinstance as isize, cw, ch);
    let fa = FontAtlas::new();
    vk.upload_atlas(&fa.pixels);
    let mut app = App::new();
    let mut dl = DrawList::new(cw, ch);
    let start_time = Instant::now();

    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        let mut last_frame = Instant::now();

        'main: loop {
            while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_QUIT { break 'main; }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            let now = Instant::now();
            INPUT.dt = now.duration_since(last_frame).as_secs_f32().min(0.1);
            INPUT.elapsed = now.duration_since(start_time).as_secs_f32();
            last_frame = now;

            if INPUT.resized {
                INPUT.resized = false;
                if INPUT.width > 0 && INPUT.height > 0 { vk.resize(INPUT.width, INPUT.height); }
            }

            app.frame(&mut dl, &fa, &INPUT);
            vk.render(&dl.vertices, &dl.indices, &dl.commands);
            INPUT.end_frame();
            std::thread::sleep(Duration::from_micros(500));
        }
    }
}

unsafe extern "system" fn wnd_proc(hwnd: *mut c_void, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_MOUSEMOVE => {
            INPUT.mouse_x = (lp & 0xFFFF) as i16 as f32;
            INPUT.mouse_y = ((lp >> 16) & 0xFFFF) as i16 as f32;
            0
        }
        WM_LBUTTONDOWN => { INPUT.mouse_down = true; 0 }
        WM_LBUTTONUP   => { INPUT.mouse_down = false; 0 }
        WM_RBUTTONDOWN => { INPUT.right_down = true; 0 }
        WM_RBUTTONUP   => { INPUT.right_down = false; 0 }
        WM_MOUSEWHEEL  => { INPUT.scroll_delta += ((wp >> 16) as i16) as f32 / 120.0; 0 }
        WM_SIZE => {
            let w = (lp & 0xFFFF) as u32;
            let h = ((lp >> 16) & 0xFFFF) as u32;
            if w > 0 && h > 0 { INPUT.width = w; INPUT.height = h; INPUT.resized = true; }
            0
        }
        WM_DESTROY => { PostQuitMessage(0); 0 }
        _ => DefWindowProcW(hwnd, msg, wp, lp)
    }
}