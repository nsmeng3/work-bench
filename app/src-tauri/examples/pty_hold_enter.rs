//! 复现"按住回车刷几行就停住"的压测：
//! 以 33ms 间隔连续写入 150 个 '\r'（模拟 macOS 键重复），
//! 用与 app 相同的 producer + flusher 管线收集输出，
//! 按 200ms 窗口打印字节数，检测输出停滞（>1s 无输出且仍在写入）。
//!
//! 跑法：`cargo run --example pty_hold_enter`

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const FLUSH_INTERVAL_MS: u64 = 16;
const FLUSH_THRESHOLD_BYTES: usize = 8 * 1024;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let mut cmd = CommandBuilder::new(&shell);
    cmd.arg("-i");
    cmd.cwd(dirs::home_dir().unwrap());
    cmd.env("TERM", "xterm-256color");

    let mut child = pair.slave.spawn_command(cmd)?;
    let mut reader = pair.master.try_clone_reader()?;
    let mut writer = pair.master.take_writer()?;

    // ---- 与 app 相同的 producer + flusher 管线 ----
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let (flush_tx, flush_rx) = mpsc::channel::<Vec<u8>>();
    thread::spawn(move || {
        let mut pending: Vec<u8> = Vec::with_capacity(FLUSH_THRESHOLD_BYTES * 2);
        let mut last_flush = Instant::now();
        loop {
            match rx.recv_timeout(Duration::from_millis(FLUSH_INTERVAL_MS)) {
                Ok(chunk) => {
                    pending.extend_from_slice(&chunk);
                    if pending.len() >= FLUSH_THRESHOLD_BYTES
                        || last_flush.elapsed() >= Duration::from_millis(FLUSH_INTERVAL_MS)
                    {
                        let _ = flush_tx.send(std::mem::take(&mut pending));
                        pending = Vec::with_capacity(FLUSH_THRESHOLD_BYTES * 2);
                        last_flush = Instant::now();
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if !pending.is_empty() {
                        let _ = flush_tx.send(std::mem::take(&mut pending));
                        pending = Vec::with_capacity(FLUSH_THRESHOLD_BYTES * 2);
                        last_flush = Instant::now();
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    if !pending.is_empty() {
                        let _ = flush_tx.send(std::mem::take(&mut pending));
                    }
                    break;
                }
            }
        }
    });

    // 等 shell 启动
    thread::sleep(Duration::from_millis(500));
    // 排空启动输出
    while flush_rx.try_recv().is_ok() {}

    // ---- 写入线程：模拟按住回车 ----
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_w = stop.clone();
    let writer_thread = thread::spawn(move || {
        let start = Instant::now();
        let mut n = 0;
        while start.elapsed() < Duration::from_secs(5) && !stop_w.load(std::sync::atomic::Ordering::Relaxed) {
            if writer.write_all(b"\r").is_err() {
                println!("[writer] write failed at #{}", n);
                break;
            }
            let _ = writer.flush();
            n += 1;
            thread::sleep(Duration::from_millis(33));
        }
        println!("[writer] sent {} newlines", n);
    });

    // ---- 统计线程：200ms 窗口字节数 ----
    let start = Instant::now();
    let mut window_bytes = 0usize;
    let mut window_start = start;
    let mut total = 0usize;
    let mut last_data = start;
    let mut stalled_reported = false;
    loop {
        match flush_rx.recv_timeout(Duration::from_millis(50)) {
            Ok(chunk) => {
                window_bytes += chunk.len();
                total += chunk.len();
                last_data = Instant::now();
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        let now = Instant::now();
        if now.duration_since(window_start) >= Duration::from_millis(200) {
            println!(
                "[{:5.1}s] +{} B (total {} B)",
                now.duration_since(start).as_secs_f64(),
                window_bytes,
                total
            );
            window_bytes = 0;
            window_start = now;
        }
        // 停滞检测：写线程还活着但 1s 没收到输出
        if !stalled_reported
            && now.duration_since(last_data) > Duration::from_secs(1)
            && !writer_thread.is_finished()
        {
            println!("!!! STALL: 1s 无输出，写入仍在进行");
            stalled_reported = true;
        }
        if writer_thread.is_finished() && now.duration_since(last_data) > Duration::from_secs(2) {
            break;
        }
    }

    println!("[done] total {} B", total);
    let _ = child.kill();
    let _ = child.wait();
    Ok(())
}
