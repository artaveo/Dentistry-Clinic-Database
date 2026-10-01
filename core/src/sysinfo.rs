//! Platform facts for the System Info page (roadmap 1.8): build architecture,
//! the machine's real architecture (so an x86 build running emulated on an
//! ARM64 PC is visible to the owner), OS version and computer name.

/// Architecture this binary was compiled for.
pub fn build_arch() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        std::env::consts::ARCH
    }
}

/// Native architecture of the machine.
pub fn machine_arch() -> String {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::SystemInformation::{
            IMAGE_FILE_MACHINE_AMD64, IMAGE_FILE_MACHINE_ARM64, IMAGE_FILE_MACHINE_I386,
        };
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, IsWow64Process2};
        let (mut process, mut native) = (0u16, 0u16);
        // Reports the native machine even for x64 code emulated on ARM64.
        if unsafe { IsWow64Process2(GetCurrentProcess(), &mut process, &mut native) } != 0 {
            return match native {
                IMAGE_FILE_MACHINE_AMD64 => "x64".into(),
                IMAGE_FILE_MACHINE_ARM64 => "arm64".into(),
                IMAGE_FILE_MACHINE_I386 => "x86".into(),
                other => format!("0x{other:04x}"),
            };
        }
        build_arch().into()
    }
    #[cfg(not(windows))]
    {
        build_arch().into()
    }
}

pub fn os_description() -> String {
    #[cfg(windows)]
    {
        use windows_sys::Wdk::System::SystemServices::RtlGetVersion;
        use windows_sys::Win32::System::SystemInformation::OSVERSIONINFOW;
        let mut v: OSVERSIONINFOW = unsafe { std::mem::zeroed() };
        v.dwOSVersionInfoSize = std::mem::size_of::<OSVERSIONINFOW>() as u32;
        if unsafe { RtlGetVersion(&mut v) } == 0 {
            // Windows 11 still reports 10.0; it is told apart by build ≥ 22000.
            let name =
                if v.dwMajorVersion == 10 && v.dwBuildNumber >= 22000 { "Windows 11" } else { "Windows 10" };
            return format!("{name} (10.{}.{})", v.dwMinorVersion, v.dwBuildNumber);
        }
        "Windows".into()
    }
    #[cfg(not(windows))]
    {
        format!("{} ({})", std::env::consts::OS, std::env::consts::FAMILY)
    }
}
