use std::os::raw::{c_uint, c_void};
type mach_port_t = u32;

fn dlsym(name: &str, handle: *mut c_void) -> *mut c_void {
    let c = std::ffi::CString::new(name).unwrap();
    unsafe { libc::dlsym(handle, c.as_ptr()) }
}

fn main() {
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
    let matching = unsafe { f("AppleSMC\0".as_ptr() as *mut _) };

    let f: unsafe extern "C" fn(mach_port_t, *mut c_void, *mut mach_port_t) -> c_uint = {
        let p = dlsym("IOServiceGetMatchingServices", iokit);
        unsafe { std::mem::transmute(p) }
    };
    let mut services: mach_port_t = 0;
    unsafe { f(master_port, matching, &mut services) };

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
    eprintln!("IOServiceGetMatchingService: service={}", service);

    if service != 0 {
        let f: unsafe extern "C" fn(mach_port_t, mach_port_t, u32, *mut mach_port_t) -> c_uint = {
            let p = dlsym("IOServiceOpen", iokit);
            unsafe { std::mem::transmute(p) }
        };
        let mut connect: mach_port_t = 0;
        for ct in [0u32, 1, 2, 3, 4, 5] {
            let r = unsafe { f(service, 0, ct, &mut connect) };
            eprintln!("  IOServiceOpen(connector={ct}): {} connect={}", r, connect);
            if r == 0 {
                break;
            }
        }

        let f: unsafe extern "C" fn(mach_port_t) = {
            let p = dlsym("IOServiceClose", iokit);
            unsafe { std::mem::transmute(p) }
        };
        if connect != 0 {
            unsafe { f(connect) };
        }
        let f2: unsafe extern "C" fn(mach_port_t) -> c_uint = {
            let p = dlsym("IOObjectRelease", iokit);
            unsafe { std::mem::transmute(p) }
        };
        unsafe { f2(service) };
    }
}
