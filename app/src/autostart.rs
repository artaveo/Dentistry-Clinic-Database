//! "Start Artaveo Dental when Windows starts" (roadmap 7.1): the clinic server must be running when
//! the day starts. A per-user Run entry starts the program minimised in the background
//! (`--background`) when its Windows account signs in; no administrator rights are needed.

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::System::Registry::{
        RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ,
    };

    const KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    const NAME: &str = "ArtaveoDental";

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn is_enabled() -> bool {
        let (key, name) = (wide(KEY), wide(NAME));
        // SAFETY: both strings are NUL-terminated and outlive the call; no output buffer is requested.
        unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            ) == 0
        }
    }

    pub fn set(on: bool) -> Result<(), String> {
        let (key, name) = (wide(KEY), wide(NAME));
        if !on {
            // SAFETY: NUL-terminated strings that outlive the call. A missing value is not an error.
            unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) };
            return Ok(());
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let command = wide(&format!("\"{}\" --background", exe.display()));
        // SAFETY: the data pointer and its byte length (including the terminating NUL) describe `command`.
        let code = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                REG_SZ,
                command.as_ptr().cast(),
                (command.len() * 2) as u32,
            )
        };
        if code == 0 {
            Ok(())
        } else {
            Err(format!("registry error {code}"))
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn is_enabled() -> bool {
        false
    }

    pub fn set(_on: bool) -> Result<(), String> {
        Err("only available on Windows".into())
    }
}

pub use imp::{is_enabled, set};
