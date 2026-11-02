//! Screen reader announcements, matching WhatSnap: a wxDragon static text
//! prepared with the `live-region` crate. WhatSnap does not call Prism
//! directly. `live-region` raises a UI Automation notification on Windows,
//! which NVDA, JAWS, and Narrator already speak.

use live_region::{Priority, announce_with_priority, set_live_region};
use wxdragon::prelude::*;

#[derive(Clone, Copy)]
pub struct Announcer {
    label: StaticText,
}

impl Announcer {
    pub fn new(parent: &impl WxWidget) -> Self {
        let label = StaticText::builder(parent).with_label("").build();
        let _ = set_live_region(&label);
        Self { label }
    }

    pub fn say(&self, message: &str) {
        announce_with_priority(self.label, message, Priority::Medium);
    }
}
