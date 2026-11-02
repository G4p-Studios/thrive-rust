//! Thrive keeps the Python client's dark grey and white text. The decision of
//! when to paint those colours follows WhatSnap: system dark mode, and no
//! custom colours while Windows high contrast is on.

use wxdragon::prelude::*;

const DARK_BG: Colour = Colour::rgb(40, 40, 40);
const LIGHT_TEXT: Colour = Colour::rgb(255, 255, 255);

pub fn apply_before_windows() {
    if is_dark() {
        let _ = set_appearance(Appearance::Dark);
    }
}

pub fn is_dark() -> bool {
    use_custom_colours() && system_prefers_dark()
}

pub fn paint(widget: &impl WxWidget) {
    if !is_dark() {
        return;
    }
    widget.set_background_color(DARK_BG);
    widget.set_foreground_color(LIGHT_TEXT);
}

pub fn use_custom_colours() -> bool {
    !high_contrast_active()
}

#[cfg(windows)]
fn system_prefers_dark() -> bool {
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
    use windows::core::w;

    let mut value = 1u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let read = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(std::ptr::from_mut(&mut value).cast()),
            Some(&mut size),
        )
    };
    read.is_ok() && value == 0
}

#[cfg(not(windows))]
fn system_prefers_dark() -> bool {
    false
}

#[cfg(windows)]
fn high_contrast_active() -> bool {
    use windows::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
    use windows::Win32::UI::WindowsAndMessaging::{
        SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
    };

    let size = std::mem::size_of::<HIGHCONTRASTW>() as u32;
    let mut info = HIGHCONTRASTW {
        cbSize: size,
        ..Default::default()
    };
    let queried = unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            size,
            Some(std::ptr::from_mut(&mut info).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    queried.is_err() || info.dwFlags.contains(HCF_HIGHCONTRASTON)
}

#[cfg(not(windows))]
fn high_contrast_active() -> bool {
    false
}
