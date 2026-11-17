//! A Plugin's Plugin Log: lines it writes through the Kernel's `nama.log` (ADR 0061).

/// Logs a `format!`-style line at `debug`, which the Server shows only when `RUST_LOG` asks for it,
/// such as with `nama::plugin=debug`. Never log a setting's value, a token or a response's body.
#[macro_export]
macro_rules! debug {
    ($($arg:tt)+) => {
        $crate::write_log_line(0, &::std::format!($($arg)+))
    };
}

/// Logs a `format!`-style line at `info`. Never log a setting's value, a token or a response's
/// body.
#[macro_export]
macro_rules! info {
    ($($arg:tt)+) => {
        $crate::write_log_line(1, &::std::format!($($arg)+))
    };
}

/// Logs a `format!`-style line at `warn`. Never log a setting's value, a token or a response's
/// body.
#[macro_export]
macro_rules! warn {
    ($($arg:tt)+) => {
        $crate::write_log_line(2, &::std::format!($($arg)+))
    };
}

/// Logs a `format!`-style line at `error`. Never log a setting's value, a token or a response's
/// body.
#[macro_export]
macro_rules! error {
    ($($arg:tt)+) => {
        $crate::write_log_line(3, &::std::format!($($arg)+))
    };
}

/// Hands `line` to `nama.log` at `level`: 0 `debug`, 1 `info`, 2 `warn`, 3 `error`. Called by
/// [`info!`](crate::info!) and the others. Off wasm32, as in a Plugin's unit tests, it does
/// nothing, since there is no Kernel to write to.
#[doc(hidden)]
#[cfg_attr(
    not(target_arch = "wasm32"),
    expect(unused_variables, reason = "off wasm32 there is no Kernel to write to")
)]
pub fn write_log_line(level: i32, line: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        use extism_pdk::{ManagedMemory, Memory};

        #[expect(unsafe_code, reason = "the Plugin Log is a Wasm import")]
        #[link(wasm_import_module = "nama")]
        unsafe extern "C" {
            #[link_name = "log"]
            safe fn nama_log(level: i32, line: u64);
        }
        // Freed when dropped, once the Kernel has read it. A line Extism can't allocate is lost,
        // as logging mustn't fail the call.
        if let Ok(memory) = Memory::from_bytes(line) {
            let memory = ManagedMemory::from(memory);
            nama_log(level, memory.offset());
        }
    }
}
