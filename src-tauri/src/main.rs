// Klar is resident all day and starts from the tray; a console window flashing
// up on launch would be a bug on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    klar_lib::run();
}
