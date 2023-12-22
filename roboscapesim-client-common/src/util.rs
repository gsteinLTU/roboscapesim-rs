
#[macro_export]
/// Macro to make console.logging easier
macro_rules! console_log {
    ($($tokens: tt)*) => {
        web_sys::console::log_1(&format!($($tokens)*).into())
    }
}
