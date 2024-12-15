pub mod app;
pub mod error_template;
#[cfg(feature = "ssr")]
pub mod http;
#[cfg(feature = "ssr")]
pub mod state;
pub mod user;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::App;
    use leptos::mount::hydrate_body;
    console_error_panic_hook::set_once();
    hydrate_body(App);
}
