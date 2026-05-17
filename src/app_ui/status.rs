use std::cell::RefCell;
use std::time::Duration;

use slint::{ComponentHandle, Timer, TimerMode};

use crate::AppWindow;

thread_local! {
    static MEMORY_TIMER: RefCell<Option<Timer>> = RefCell::new(None);
}

pub fn install(app: &AppWindow) {
    app.set_app_version(format!("v{}", env!("CARGO_PKG_VERSION")).into());
    update_memory_usage(app);

    let app_weak = app.as_weak();
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_secs(1), move || {
        let Some(app) = app_weak.upgrade() else {
            return;
        };
        update_memory_usage(&app);
    });

    MEMORY_TIMER.with(|slot| {
        *slot.borrow_mut() = Some(timer);
    });
}

fn update_memory_usage(app: &AppWindow) {
    app.set_memory_usage(format_memory_usage().into());
}

fn format_memory_usage() -> String {
    match current_rss_bytes().map(bytes_to_mb) {
        Some(mb) => format!("mem {mb} mb"),
        None => "mem -- mb".to_string(),
    }
}

fn bytes_to_mb(bytes: u64) -> u64 {
    bytes.div_ceil(1024 * 1024)
}

#[cfg(target_os = "linux")]
fn current_rss_bytes() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let kb = status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))?
        .split_whitespace()
        .next()?
        .parse::<u64>()
        .ok()?;

    Some(kb * 1024)
}

#[cfg(target_os = "macos")]
fn current_rss_bytes() -> Option<u64> {
    use std::mem::{MaybeUninit, size_of};

    type KernReturn = i32;
    type MachPort = u32;
    type TaskFlavor = i32;
    type MachMsgTypeNumber = u32;

    const KERN_SUCCESS: KernReturn = 0;
    const TASK_BASIC_INFO_64: TaskFlavor = 5;

    #[repr(C)]
    struct TimeValue {
        seconds: i32,
        microseconds: i32,
    }

    #[repr(C)]
    struct TaskBasicInfo64 {
        virtual_size: u64,
        resident_size: u64,
        resident_size_max: u64,
        user_time: TimeValue,
        system_time: TimeValue,
        policy: i32,
        suspend_count: i32,
    }

    unsafe extern "C" {
        fn mach_task_self() -> MachPort;
        fn task_info(
            target_task: MachPort,
            flavor: TaskFlavor,
            task_info_out: *mut i32,
            task_info_out_count: *mut MachMsgTypeNumber,
        ) -> KernReturn;
    }

    let mut info = MaybeUninit::<TaskBasicInfo64>::uninit();
    let mut count = (size_of::<TaskBasicInfo64>() / size_of::<i32>()) as MachMsgTypeNumber;
    let result = unsafe {
        task_info(
            mach_task_self(),
            TASK_BASIC_INFO_64,
            info.as_mut_ptr().cast::<i32>(),
            &mut count,
        )
    };

    if result == KERN_SUCCESS {
        Some(unsafe { info.assume_init() }.resident_size)
    } else {
        None
    }
}

#[cfg(windows)]
fn current_rss_bytes() -> Option<u64> {
    use std::ffi::c_void;
    use std::mem::size_of;

    type Bool = i32;
    type Dword = u32;
    type Handle = *mut c_void;

    #[repr(C)]
    struct ProcessMemoryCounters {
        cb: Dword,
        page_fault_count: Dword,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> Handle;
    }

    #[link(name = "psapi")]
    unsafe extern "system" {
        fn GetProcessMemoryInfo(
            process: Handle,
            counters: *mut ProcessMemoryCounters,
            size: Dword,
        ) -> Bool;
    }

    let size = size_of::<ProcessMemoryCounters>() as Dword;
    let mut counters = ProcessMemoryCounters {
        cb: size,
        page_fault_count: 0,
        peak_working_set_size: 0,
        working_set_size: 0,
        quota_peak_paged_pool_usage: 0,
        quota_paged_pool_usage: 0,
        quota_peak_non_paged_pool_usage: 0,
        quota_non_paged_pool_usage: 0,
        pagefile_usage: 0,
        peak_pagefile_usage: 0,
    };
    let result = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, size) };

    if result != 0 {
        Some(counters.working_set_size as u64)
    } else {
        None
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn current_rss_bytes() -> Option<u64> {
    None
}
