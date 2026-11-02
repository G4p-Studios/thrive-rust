mod announce;
mod api;
mod config;
mod render;
mod rows;
mod session;
mod sounds;
mod theme;
mod timeutil;
mod ui;

fn main() {
    if let Err(error) = wxdragon::main(|_| ui::start()) {
        eprintln!("Thrive could not start: {error}");
    }
}
