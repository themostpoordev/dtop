use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn is_quit(key: KeyEvent) -> bool { matches!(key.code, KeyCode::Char('q')) || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)) }
