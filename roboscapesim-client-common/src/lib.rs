pub mod api;
pub mod util;

#[cfg(debug_assertions)]
pub const ASSETS_DIR: &str = "http://localhost:4000/assets/";
#[cfg(not(debug_assertions))]
pub const ASSETS_DIR: &str = "https://extensions.netsblox.org/extensions/RoboScapeOnline2/assets/";
