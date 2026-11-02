mod app;
mod dialogs;

use crate::api::Mastodon;
use crate::session::Session;
use crate::theme;
use wxdragon::prelude::*;

pub fn start() {
    theme::apply_before_windows();
    match crate::session::load() {
        Some(session) => match Mastodon::from_session(&session).and_then(|api| api.me().map(|me| (api, me))) {
            Ok((api, me)) => app::open(api, me),
            Err(error) => {
                let frame = auth_frame();
                alert(&frame, &format!("Error loading saved session: {error}"), "Error");
            }
        },
        None => {
            auth_frame();
        }
    }
}

fn auth_frame() -> Frame {
    let frame = Frame::builder()
        .with_title("Thrive Login")
        .with_size(Size::new(400, 180))
        .build();
    let panel = Panel::builder(&frame).build();
    theme::paint(&frame);
    theme::paint(&panel);
    let label = StaticText::builder(&panel)
        .with_label("Mastodon Instance URL:")
        .build();
    let input = TextCtrl::builder(&panel)
        .with_value("https://vee.seedy.cc")
        .build();
    let button = Button::builder(&panel).with_label("Authenticate").build();
    theme::paint(&label);
    theme::paint(&input);
    theme::paint(&button);
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    sizer.add(&label, 0, SizerFlag::All, 5);
    sizer.add(&input, 0, SizerFlag::All | SizerFlag::Expand, 5);
    sizer.add(&button, 0, SizerFlag::All | SizerFlag::AlignCenterHorizontal, 5);
    panel.set_sizer(sizer, true);
    let frame_for_click = frame;
    button.on_click(move |_| authenticate(frame_for_click, input));
    frame.show(true);
    frame.centre();
    frame
}

fn authenticate(frame: Frame, input: TextCtrl) {
    let instance = input.get_value();
    let (base, client_id, client_secret) = match Mastodon::register(instance.trim()) {
        Ok(value) => value,
        Err(error) => {
            error_box(&frame, &format!("Authentication failed: {error}"), "Error");
            return;
        }
    };
    open_browser(&Mastodon::authorize_url(&base, &client_id));
    let Some(code) = prompt(&frame, "Enter the authorization code from the browser:", "Enter Code") else {
        return;
    };
    let token = match Mastodon::log_in(&base, &client_id, &client_secret, code.trim()) {
        Ok(token) => token,
        Err(error) => {
            error_box(&frame, &format!("Authentication failed: {error}"), "Error");
            return;
        }
    };
    let session = Session {
        client_id,
        client_secret,
        access_token: token,
        instance_url: base,
        username: String::new(),
    };
    let api = match Mastodon::from_session(&session) {
        Ok(api) => api,
        Err(error) => {
            error_box(&frame, &format!("Authentication failed: {error}"), "Error");
            return;
        }
    };
    let me = match api.me() {
        Ok(me) => me,
        Err(error) => {
            error_box(&frame, &format!("Authentication failed: {error}"), "Error");
            return;
        }
    };
    let mut session = session;
    session.username = me.get("username").and_then(|value| value.as_str()).unwrap_or("").to_string();
    if let Err(error) = crate::session::save(&session) {
        error_box(&frame, &format!("Could not save the session: {error}"), "Error");
        return;
    }
    app::open(api, me);
    frame.close(false);
}

pub(crate) fn alert(parent: &impl WxWidget, message: &str, title: &str) {
    message_box(parent, message, title, MessageDialogStyle::OK | MessageDialogStyle::IconInformation);
}

pub(crate) fn error_box(parent: &impl WxWidget, message: &str, title: &str) {
    message_box(parent, message, title, MessageDialogStyle::OK | MessageDialogStyle::IconError);
}

pub(crate) fn confirm(parent: &impl WxWidget, message: &str, title: &str) -> bool {
    let dialog = MessageDialog::builder(parent, message, title)
        .with_style(MessageDialogStyle::YesNo | MessageDialogStyle::IconQuestion)
        .build();
    dialog.show_modal() == ID_YES
}

pub(crate) fn prompt(parent: &impl WxWidget, message: &str, title: &str) -> Option<String> {
    let dialog = TextEntryDialog::builder(parent, message, title).build();
    if dialog.show_modal() == ID_OK {
        let value = dialog.get_value().unwrap_or_default();
        if value.trim().is_empty() {
            None
        } else {
            Some(value)
        }
    } else {
        None
    }
}

fn message_box(parent: &impl WxWidget, message: &str, title: &str, style: MessageDialogStyle) {
    let dialog = MessageDialog::builder(parent, message, title).with_style(style).build();
    dialog.show_modal();
}

pub(crate) fn open_browser(url: &str) {
    #[cfg(windows)]
    {
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        use windows::core::{HSTRING, PCWSTR, w};

        // Ask the shell to open the URL. Going through cmd.exe splits the
        // link on & and misreads the empty window title as the path "\\".
        let file = HSTRING::from(url);
        unsafe {
            ShellExecuteW(None, w!("open"), &file, PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL);
        }
    }
    #[cfg(not(windows))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

pub(crate) fn copy_text(text: &str) {
    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        let _ = clipboard.set_text(text.to_string());
    }
}

pub(crate) fn styled_dialog(parent: &impl WxWidget, title: &str, width: i32, height: i32) -> (Dialog, Panel, BoxSizer) {
    let dialog = Dialog::builder(parent, title).with_size(width, height).build();
    let panel = Panel::builder(&dialog).build();
    theme::paint(&dialog);
    theme::paint(&panel);
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    (dialog, panel, sizer)
}
