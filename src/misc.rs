use std::ffi::CString;
use std::mem;
use std::ptr;
use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::windef::{HWND, RECT};
use winapi::um::libloaderapi::GetModuleHandleA;
use winapi::um::profileapi::{QueryPerformanceCounter, QueryPerformanceFrequency};
use winapi::um::winnt::{LARGE_INTEGER, LPCSTR};
use winapi::um::winuser::{
    AdjustWindowRect, CreateWindowExA, DefWindowProcA, DispatchMessageA, LoadCursorA, PeekMessageA,
    PostQuitMessage, RegisterClassA, SetProcessDPIAware, SetWindowTextA, CW_USEDEFAULT, IDC_ARROW,
    MSG, PM_REMOVE, VK_ESCAPE, WM_DESTROY, WM_KEYDOWN, WM_QUIT, WNDCLASSA, WS_CAPTION,
    WS_MINIMIZEBOX, WS_OVERLAPPED, WS_SYSMENU, WS_VISIBLE,
};

#[macro_export]
macro_rules! vhr {
    ($name:expr) => {
        unsafe {
            let hr = $name;
            assert_eq!(hr, 0);
        };
    };
}

pub struct FrameStats {
    pub time: f64,
    pub delta_time: f32,
    previous_time: f64,
    header_refresh_time: f64,
    num_frames: u64,
    start_counter: LARGE_INTEGER,
    frequency: LARGE_INTEGER,
}

unsafe extern "system" fn wndproc(
    window: HWND,
    message: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let handled = match message {
        WM_DESTROY => {
            PostQuitMessage(0);
            true
        }
        WM_KEYDOWN => {
            if wparam == VK_ESCAPE as usize {
                PostQuitMessage(0);
                true
            } else {
                false
            }
        }
        _ => false,
    };

    if handled {
        0
    } else {
        DefWindowProcA(window, message, wparam, lparam)
    }
}

pub fn create_window(name: &CString, width: u32, height: u32) -> HWND {
    unsafe {
        SetProcessDPIAware();

        let mut winclass: WNDCLASSA = mem::zeroed();
        winclass.lpfnWndProc = Some(wndproc);
        winclass.hInstance = GetModuleHandleA(ptr::null());
        winclass.hCursor = LoadCursorA(ptr::null_mut(), IDC_ARROW as LPCSTR);
        winclass.lpszClassName = name.as_ptr();
        RegisterClassA(&winclass);
    }

    let style = WS_OVERLAPPED | WS_SYSMENU | WS_CAPTION | WS_MINIMIZEBOX;
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: width as i32,
        bottom: height as i32,
    };
    unsafe {
        AdjustWindowRect(&mut rect, style, 0);
        CreateWindowExA(
            0,
            name.as_ptr(),
            name.as_ptr(),
            style | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            rect.right - rect.left,
            rect.bottom - rect.top,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
        )
    }
}

pub fn handle_window_messages() -> bool {
    unsafe {
        let mut message: MSG = mem::zeroed();
        while PeekMessageA(&mut message, ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
            DispatchMessageA(&message);
            if message.message == WM_QUIT {
                return false;
            }
        }
    }
    true
}

impl FrameStats {
    pub fn new() -> Self {
        let mut start_counter: LARGE_INTEGER = unsafe { mem::zeroed() };
        let mut frequency: LARGE_INTEGER = unsafe { mem::zeroed() };
        unsafe {
            QueryPerformanceCounter(&mut start_counter as *mut _ as *mut LARGE_INTEGER);
            QueryPerformanceFrequency(&mut frequency as *mut _ as *mut LARGE_INTEGER);
        }
        Self {
            time: 0.0,
            delta_time: 0.0,
            previous_time: -1.0,
            header_refresh_time: 0.0,
            num_frames: 0,
            start_counter,
            frequency,
        }
    }

    pub fn update(&mut self, window: HWND, name: &CString) {
        if self.previous_time < 0.0 {
            self.previous_time = self.time();
            self.header_refresh_time = self.previous_time;
        }

        self.time = self.time();
        self.delta_time = (self.time - self.previous_time) as f32;
        self.previous_time = self.time;

        if (self.time - self.header_refresh_time) >= 1.0 {
            let fps = (self.num_frames as f64) / (self.time - self.header_refresh_time);
            let ms = (1.0 / fps) * 1000.0;
            let header = CString::new(format!(
                "[{:.1} fps  {:.3} ms] {}",
                fps,
                ms,
                name.to_str().unwrap()
            ))
            .unwrap();
            unsafe {
                SetWindowTextA(window, header.as_ptr());
            }
            self.header_refresh_time = self.time;
            self.num_frames = 0;
        }
        self.num_frames += 1;
    }

    pub fn time(&self) -> f64 {
        let mut counter: LARGE_INTEGER = unsafe { mem::zeroed() };
        unsafe { QueryPerformanceCounter(&mut counter as *mut _ as *mut LARGE_INTEGER) };

        let counter = unsafe { *counter.QuadPart() };
        let start_counter = unsafe { *self.start_counter.QuadPart() };
        let frequency = unsafe { *self.frequency.QuadPart() };

        ((counter - start_counter) as f64) / (frequency as f64)
    }
}
