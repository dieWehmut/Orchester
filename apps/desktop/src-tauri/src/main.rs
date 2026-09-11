#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

fn main() {
    orchester_desktop::run().expect("failed to run the Orchester desktop shell");
}
