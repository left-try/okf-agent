mod fixture;

use okf_adapter_fs::discovery::FsSourceCatalog;
use okf_adapter_sqlite::repository::SqliteIndexStore;
use okf_application::repository_index::{self, IndexStore};
use serde_json::{Value, json};
use std::{fs, process::Command, time::Instant};

fn main() {
    if let Some(size) = std::env::args()
        .nth(1)
        .and_then(|arg| arg.parse::<usize>().ok())
    {
        let report = run_case(size).expect("benchmark case");
        println!("{}", serde_json::to_string(&report).unwrap());
        return;
    }
    let executable = std::env::current_exe().expect("benchmark executable path");
    println!(
        "platform: {}-{}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    for (label, files) in [("small", 250), ("large", 2_000)] {
        let mut samples = Vec::new();
        for _ in 0..5 {
            let output = Command::new(&executable)
                .arg(files.to_string())
                .output()
                .expect("run isolated benchmark");
            assert!(
                output.status.success(),
                "benchmark child failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            samples.push(serde_json::from_slice::<Value>(&output.stdout).expect("benchmark JSON"));
        }
        let median = |name: &str| {
            let mut values = samples
                .iter()
                .filter_map(|sample| sample[name].as_u64())
                .collect::<Vec<_>>();
            values.sort_unstable();
            values
                .get(values.len() / 2)
                .copied()
                .map(Value::from)
                .unwrap_or(Value::Null)
        };
        println!(
            "{label}: {}",
            json!({"samples":samples,"median":{"cold_index_ms":median("cold_index_ms"),"repeated_index_ms":median("repeated_index_ms"),"search_average_us":median("search_average_us"),"peak_working_set_bytes":median("peak_working_set_bytes")}})
        );
    }
}

#[derive(serde::Serialize)]
struct Report {
    files: usize,
    cold_index_ms: u128,
    repeated_index_ms: u128,
    search_average_us: u128,
    peak_working_set_bytes: Option<u64>,
}

fn run_case(files: usize) -> std::io::Result<Report> {
    let root = std::env::temp_dir().join(format!("okf-index-bench-{}-{files}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fixture::create(&root, files)?;
    let catalog = FsSourceCatalog;
    let index = SqliteIndexStore;
    let start = Instant::now();
    repository_index::build(&root, &catalog, &index).expect("cold index");
    let cold_index_ms = start.elapsed().as_millis();
    let start = Instant::now();
    repository_index::build(&root, &catalog, &index).expect("repeated index");
    let repeated_index_ms = start.elapsed().as_millis();
    let start = Instant::now();
    for _ in 0..30 {
        index
            .search(&root, "benchmark_search_token", 10)
            .expect("FTS query");
    }
    let search_average_us = start.elapsed().as_micros() / 30;
    let peak_working_set_bytes = peak_memory_bytes();
    fs::remove_dir_all(root)?;
    Ok(Report {
        files,
        cold_index_ms,
        repeated_index_ms,
        search_average_us,
        peak_working_set_bytes,
    })
}

#[cfg(target_os = "windows")]
fn peak_memory_bytes() -> Option<u64> {
    use std::ffi::c_void;
    #[repr(C)]
    struct Counters {
        cb: u32,
        faults: u32,
        peak_ws: usize,
        ws: usize,
        peak_pool: usize,
        pool: usize,
        peak_np: usize,
        np: usize,
        page: usize,
        peak_page: usize,
        private: usize,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut c_void;
    }
    #[link(name = "psapi")]
    unsafe extern "system" {
        fn GetProcessMemoryInfo(process: *mut c_void, counters: *mut Counters, size: u32) -> i32;
    }
    let mut counters = Counters {
        cb: std::mem::size_of::<Counters>() as u32,
        faults: 0,
        peak_ws: 0,
        ws: 0,
        peak_pool: 0,
        pool: 0,
        peak_np: 0,
        np: 0,
        page: 0,
        peak_page: 0,
        private: 0,
    };
    let ok = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) };
    (ok != 0).then_some(counters.peak_ws as u64)
}

#[cfg(target_os = "linux")]
fn peak_memory_bytes() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    let kb = status.lines().find_map(|line| {
        line.strip_prefix("VmHWM:")?
            .split_whitespace()
            .next()?
            .parse::<u64>()
            .ok()
    })?;
    Some(kb * 1024)
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn peak_memory_bytes() -> Option<u64> {
    None
}
