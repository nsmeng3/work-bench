//! M7-4 Spike：验证 portable-pty 在 macOS arm64 能 spawn zsh、写入、读输出、kill。
//!
//! 跑法：`cargo run --example pty_spike`
//!
//! 期望输出：
//! - 启动子进程成功
//! - 写入 `echo hello\n` 后能在 3 秒内读到含 "hello" 的字节
//! - kill 成功

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    println!("[spike] shell = {}", shell);

    let mut cmd = CommandBuilder::new(&shell);
    cmd.arg("-i"); // interactive，让 zsh 打印 prompt
    cmd.env("TERM", "xterm-256color");

    let mut child = pair.slave.spawn_command(cmd)?;
    println!("[spike] spawned child pid={:?}", child.process_id());

    // 拿到 master 的 reader / writer
    let mut reader = pair.master.try_clone_reader()?;
    let mut writer = pair.master.take_writer()?;

    // 后台线程把读到的字节通过 mpsc 送回主线程
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => {
                    println!("[spike-reader] EOF");
                    break;
                }
                Ok(n) => {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("[spike-reader] read err: {}", e);
                    break;
                }
            }
        }
    });

    // 等 500ms 让 zsh 起 prompt
    thread::sleep(Duration::from_millis(500));

    // 写入 echo hello
    writer.write_all(b"echo hello\n")?;
    writer.flush()?;
    println!("[spike] wrote: echo hello");

    // 3 秒内收集输出
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut collected: Vec<u8> = Vec::new();
    while Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(chunk) => collected.extend_from_slice(&chunk),
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        if String::from_utf8_lossy(&collected).contains("hello") {
            break;
        }
    }

    let text = String::from_utf8_lossy(&collected);
    println!("[spike] collected {} bytes", collected.len());
    println!("[spike] output (escaped): {:?}", text);

    let got_hello = text.contains("hello");
    println!("[spike] got 'hello': {}", got_hello);

    // kill 子进程
    child.kill()?;
    let status = child.wait()?;
    println!("[spike] child exit status: {:?}", status);

    if got_hello {
        println!("[spike] OK");
        Ok(())
    } else {
        Err("did not read 'hello' from pty".into())
    }
}
