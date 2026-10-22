//! Standalone rustc test fixture; never linked into production or distributed.
use std::{
    fs,
    io::{Read, Write},
    thread,
    time::Duration,
};
unsafe extern "C" {
    fn signal(sig: i32, handler: usize) -> usize;
    fn fork() -> i32;
    fn setsid() -> i32;
    fn close(fd: i32) -> i32;

}
fn main() {
    let exe = std::env::current_exe().unwrap();
    let root = exe.parent().unwrap();
    let mode = fs::read_to_string(root.join("fault-mode")).unwrap();
    if mode == "term-resistant" || mode.starts_with("descendant-") {
        unsafe {
            signal(15, 1);
            signal(2, 1);
        }
    }
    fs::write(root.join("fault-pid"), std::process::id().to_string()).unwrap();
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input).unwrap();
    match mode.as_str() {
        "descendant-group" | "descendant-escaped" => {
            let pid = unsafe { fork() };
            assert!(pid >= 0);
            if pid == 0 {
                if mode == "descendant-escaped" {
                    assert!(unsafe { setsid() } > 0);
                }
                // Do not let the escaped negative hold capture pipes open.
                unsafe { close(0); close(1); close(2); }
                fs::write(root.join("descendant-pid"), std::process::id().to_string()).unwrap();
                // Safety bound even if the isolated supervisor itself fails.
                thread::sleep(Duration::from_secs(15));
                std::process::exit(0);
            }
            loop { thread::sleep(Duration::from_secs(1)); }
        }
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
