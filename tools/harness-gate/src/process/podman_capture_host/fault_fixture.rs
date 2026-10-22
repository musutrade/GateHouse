//! Standalone rustc test fixture; never linked into production or distributed.
use std::{
    fs,
    io::{Read, Write},
    thread,
    time::Duration,
};
unsafe extern "C" {
    fn signal(sig: i32, handler: usize) -> usize;
}
fn main() {
    let exe = std::env::current_exe().unwrap();
    let root = exe.parent().unwrap();
    let mode = fs::read_to_string(root.join("fault-mode")).unwrap();
    if mode == "term-resistant" {
        unsafe {
            signal(15, 1);
            signal(2, 1);
        }
    }
    fs::write(root.join("fault-pid"), std::process::id().to_string()).unwrap();
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input).unwrap();
    match mode.as_str() {
        "partial-exit" => {
            std::io::stdout().write_all(b"{\"schema\":").unwrap();
            std::process::exit(7);
        }
        "stderr" => {
            std::io::stderr().write_all(b"fixture stderr").unwrap();
        }
        "stderr-near-cap" | "stderr-at-cap" | "stderr-over-cap" => {
            let count = match mode.as_str() {
                "stderr-near-cap" => 65535,
                "stderr-at-cap" => 65536,
                _ => 65537,
            };
            let _ = std::io::stderr().write_all(&vec![b'e'; count]);
        }
        "oversize" => {
            let bytes = [b'x'; 65536];
            for _ in 0..512 {
                if std::io::stdout().write_all(&bytes).is_err() {
                    break;
                }
            }
        }
        "term-resistant" => loop {
            thread::sleep(Duration::from_secs(1));
        },
        _ => std::process::exit(8),
    }
}
