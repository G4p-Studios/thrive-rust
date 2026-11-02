# Thrive Rust Client

A Rust port of the [Thrive Mastodon client](https://github.com/G4p-Studios/Thrive). The window, menus, keyboard shortcuts, and dialogs follow the Python client. The interface is built with [wxDragon](https://crates.io/crates/wxdragon), so the same actions are native controls instead of wxPython.

## What is it?

Thrive is a keyboard-first desktop client for Mastodon. It is designed for people using screen readers such as [NVDA](https://nvaccess.org/about-nvda/) and [JAWS](https://www.freedomscientific.com/products/software/jaws/), with labelled controls, keyboard navigation, accessible post content, and sound cues.

Screen reader announcements that are not already the name of a focused control go through a live region, the same approach used by WhatSnap. On Windows that is a UI Automation notification, which NVDA, JAWS, and Narrator speak. Colours follow the Windows app theme: dark grey text fields when apps use dark mode, and the system colours when high contrast is on.

## Running Thrive

You need a recent Rust toolchain, CMake, and the Visual Studio C++ build tools.

```
cargo run --release
```

The first build downloads the wxWidgets libraries used by wxDragon and takes longer than later builds.

On first launch, enter your instance domain, choose Authenticate, approve Thrive in the browser, then paste the code into the dialog. The session is stored as JSON in `user.dat` beside the working directory. A Python Thrive `user.dat` is a pickle file, so sign in once in this client to create a new session. Treat `user.dat` as sensitive because it contains the access token.

Sound packs live in `sounds/mastodon-<name>/`, the same layout as the Python client and TweeseCake. Choose one with Alt+S. The default pack is included.

## Checks

```
cargo test
```

## Credits

* Alex Chapman: initial product idea and development.
* Hamid, A.K.A. Kalahami: Bug fixes, code improvements and feature additions.
* Mason Armstrong and Mckensie Parker: Alpha testers.
* Stuart Hughes, A.K.A. TwoThousandStu: pip requirements, compile script, readme file and initial alpha release binary.
