const COMMANDS: &[&str] = &["write_board"];

fn main() {
  tauri_plugin::Builder::new(COMMANDS)
    .build();
}
