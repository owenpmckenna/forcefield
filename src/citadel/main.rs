use serde_json::Value::String;
use crate::citadel::state::BackendState;
use crate::citadel::ui::ui_main::ui_main;

pub fn citadel_main() {
    let mut state = BackendState::get();
    ui_main(&mut state).unwrap();
    if let Some(wg) = state.current_wg_setup {
        wg.down();
    }
}