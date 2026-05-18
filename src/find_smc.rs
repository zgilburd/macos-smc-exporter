use std::os::raw::{c_uint, c_void};
type mach_port_t = u32;

fn dlsym(name: &str, handle: *mut c_void) -> *mut c_void {
    let c = std::ffi::CString::new(name).unwrap();
    unsafe { libc::dlsym(handle, c.as_ptr()) }
}

fn try_open(service_name: &str) {
    let iokit = unsafe {
        libc::dlopen(
            b"/System/Library/Frameworks/IOKit.framework/IOKit\0".as_ptr() as *const _,
            libc::RTLD_NOW,
        )
    };

    let mut master_port: mach_port_t = 0;
    let f: unsafe extern "C" fn(mach_port_t, *mut mach_port_t) -> c_uint = {
        let p = dlsym("IOMasterPort", iokit);
        unsafe { std::mem::transmute(p) }
    };
    unsafe { f(0, &mut master_port) };

    let f: unsafe extern "C" fn(*mut c_void) -> *mut c_void = {
        let p = dlsym("IOServiceMatching", iokit);
        unsafe { std::mem::transmute(p) }
    };
    let matching = unsafe {
        let mut buf = vec![0u8; service_name.len() + 1];
        buf[..service_name.len()].copy_from_slice(service_name.as_bytes());
        f(buf.as_mut_ptr() as *mut _)
    };

    let f: unsafe extern "C" fn(mach_port_t, *mut c_void, *mut mach_port_t) -> c_uint = {
        let p = dlsym("IOServiceGetMatchingServices", iokit);
        unsafe { std::mem::transmute(p) }
    };
    let mut services: mach_port_t = 0;
    let r = unsafe { f(master_port, matching, &mut services) };
    eprintln!("  {}: matching={:?} get_services={}", service_name, matching, r);

    if r == 0 && services != 0 {
        let f: unsafe extern "C" fn(mach_port_t) -> c_uint = {
            let p = dlsym("IOObjectRelease", iokit);
            unsafe { std::mem::transmute(p) }
        };
        unsafe { f(services) };

        let f: unsafe extern "C" fn(mach_port_t, *mut c_void) -> mach_port_t = {
            let p = dlsym("IOServiceGetMatchingService", iokit);
            unsafe { std::mem::transmute(p) }
        };
        let service = unsafe { f(master_port, matching) };
        eprintln!("    service={}", service);

        if service != 0 {
            let f: unsafe extern "C" fn(mach_port_t, mach_port_t, u32, *mut mach_port_t) -> c_uint = {
                let p = dlsym("IOServiceOpen", iokit);
                unsafe { std::mem::transmute(p) }
            };
            let mut connect: mach_port_t = 0;
            for ct in [0u32, 1, 2, 3, 4, 5] {
                let r = unsafe { f(service, 0, ct, &mut connect) };
                eprintln!("    IOServiceOpen(ct={ct}): {} connect={}", r, connect);
                if r == 0 {
                    let f: unsafe extern "C" fn(mach_port_t) = {
                        let p = dlsym("IOServiceClose", iokit);
                        unsafe { std::mem::transmute(p) }
                    };
                    unsafe { f(connect) };
                    let f2: unsafe extern "C" fn(mach_port_t) -> c_uint = {
                        let p = dlsym("IOObjectRelease", iokit);
                        unsafe { std::mem::transmute(p) }
                    };
                    unsafe { f2(service) };
                    break;
                }
            }
        }
    }
}

fn main() {
    for name in &[
        "AppleSMC",
        "AppleSMCKeysEndpoint",
        "RTBuddyEndpointService",
        "RTBuddy",
        "AppleARMIODevice",
        "AppleT6000SMC",
        "AppleT8103SMC",
        "AppleM3SMC",
    ] {
        try_open(name);
    }
}
