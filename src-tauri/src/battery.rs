//! Battery / AC / lid power-event monitoring.
//!
//! A dedicated thread owns a hidden message-only window and subscribes to the
//! four power-setting GUIDs. Every broadcast just marks the shared state dirty;
//! the engine repolls `GetSystemPowerStatus` (authoritative) at most every 10s.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Power::{
    GetSystemPowerStatus, RegisterPowerSettingNotification, SYSTEM_POWER_STATUS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DEVICE_NOTIFY_WINDOW_HANDLE, DefWindowProcW, DispatchMessageW, GetMessageW,
    HWND_MESSAGE, MSG, RegisterClassW, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE,
    WNDCLASS_STYLES, WNDCLASSW,
};
use windows::core::{GUID, PCWSTR};

// The `windows` crate does not generate the power-setting GUIDs, so they are
// declared here with the authoritative values from the Windows SDK
// (`winnt.h`, Windows SDK 10.0.26100.0).
const GUID_POWER_SAVING_STATUS: GUID = GUID::from_u128(0xe00958c0_c213_4ace_ac77_fecced2eeea5);
const GUID_ACDC_POWER_SOURCE: GUID = GUID::from_u128(0x5D3E9A59_E9D5_4B00_A6BD_FF34FF516548);
const GUID_LIDSWITCH_STATE_CHANGE: GUID = GUID::from_u128(0xBA3E0F4D_B817_4094_A2D1_D56379E6A0F3);
const GUID_BATTERY_PERCENTAGE_REMAINING: GUID =
    GUID::from_u128(0xA7AD8041_B45A_4CAE_87A3_EECBB468A9E1);

/// Set whenever a power broadcast arrived; the engine repolls immediately.
pub static POWER_DIRTY: AtomicBool = AtomicBool::new(true);

/// 0 = unknown, 1 = lid closed, 2 = lid open.
pub static LID_CLOSED: AtomicU8 = AtomicU8::new(0);

const PBT_POWERSETTINGCHANGE: u32 = 0x8013;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BatteryStatus {
    pub present: bool,
    pub percent: Option<u8>,
    pub plugged_in: bool,
    pub saver_on: bool,
}

impl Default for BatteryStatus {
    fn default() -> Self {
        BatteryStatus {
            present: false,
            percent: None,
            plugged_in: true,
            saver_on: false,
        }
    }
}

/// Authoritative battery read. Values of 255 mean "unknown" and are ignored.
pub fn poll() -> BatteryStatus {
    let mut raw = SYSTEM_POWER_STATUS::default();
    if unsafe { GetSystemPowerStatus(&mut raw) }.is_err() {
        return BatteryStatus::default();
    }

    let unknown_battery = raw.BatteryFlag == 255;
    let no_battery = raw.BatteryFlag & 128 != 0;
    let present = !unknown_battery && !no_battery;

    let percent = if raw.BatteryLifePercent == 255 || !present {
        None
    } else {
        Some(raw.BatteryLifePercent.min(100))
    };

    BatteryStatus {
        present,
        percent,
        plugged_in: raw.ACLineStatus == 1,
        saver_on: raw.SystemStatusFlag & 1 != 0,
    }
}

#[repr(C)]
struct PowerBroadcastSetting {
    power_setting: GUID,
    data_length: u32,
    data: [u8; 8],
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == 0x0218 {
        // WM_POWERBROADCAST
        if wparam.0 as u32 == PBT_POWERSETTINGCHANGE {
            let pbs = lparam.0 as *const PowerBroadcastSetting;
            if !pbs.is_null() {
                let setting = unsafe { &*pbs };
                if setting.power_setting == GUID_LIDSWITCH_STATE_CHANGE {
                    let closed = setting.data[0] == 0;
                    LID_CLOSED.store(if closed { 1 } else { 2 }, Ordering::Relaxed);
                }
            }
        }
        POWER_DIRTY.store(true, Ordering::Relaxed);
        return LRESULT(1);
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// Start the hidden message-only window on its own thread and subscribe to the
/// four power-setting GUIDs. Safe to call once, from anywhere.
pub fn spawn_listener() {
    let _ = std::thread::Builder::new()
        .name("ember-power".into())
        .spawn(move || unsafe { listener_main() });
}

unsafe fn listener_main() {
    let class_name: Vec<u16> = "EmberPowerMessageWindow\0".encode_utf16().collect();

    let hinstance = match unsafe { GetModuleHandleW(None) } {
        Ok(h) => windows::Win32::Foundation::HINSTANCE(h.0),
        Err(_) => return,
    };

    let wc = WNDCLASSW {
        style: WNDCLASS_STYLES(0),
        lpfnWndProc: Some(wndproc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance,
        hIcon: Default::default(),
        hCursor: Default::default(),
        hbrBackground: Default::default(),
        lpszMenuName: PCWSTR::null(),
        lpszClassName: PCWSTR(class_name.as_ptr()),
    };
    unsafe { RegisterClassW(&wc) };

    let hwnd = match unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class_name.as_ptr()),
            PCWSTR(class_name.as_ptr()),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(hinstance),
            None,
        )
    } {
        Ok(h) => h,
        Err(_) => return,
    };

    // `\u{1}`-free: HWND is passed as the notification recipient.
    let recipient = HANDLE(hwnd.0);
    let flags = DEVICE_NOTIFY_WINDOW_HANDLE;
    for guid in [
        &GUID_LIDSWITCH_STATE_CHANGE,
        &GUID_POWER_SAVING_STATUS,
        &GUID_ACDC_POWER_SOURCE,
        &GUID_BATTERY_PERCENTAGE_REMAINING,
    ] {
        let _ = unsafe { RegisterPowerSettingNotification(recipient, guid as *const GUID, flags) };
    }

    // Prime the lid state so the UI is correct before the first broadcast.
    POWER_DIRTY.store(true, Ordering::Relaxed);

    let mut msg = MSG::default();
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.as_bool() {
        let _ = unsafe { TranslateMessage(&msg) };
        unsafe { DispatchMessageW(&msg) };
    }
}
