use std::io::Write;
use std::process::{Command, Stdio, Child};
use std::sync::mpsc::Receiver;
use std::thread;

pub struct FfmpegHandle {
    pub child: Child,
}

pub fn spawn_ffmpeg(width: u32, height: u32, fps: u32, rx: Receiver<Vec<u8>>) -> std::io::Result<FfmpegHandle> {
    let filename = format!("output_{}_{}.mp4", fps, chrono::Utc::now().format("%Y-%m-%d_%H-%M-%S"));
    let mut child = Command::new("ffmpeg")
        .args(&[
            "-y",
            "-f", "rawvideo",
            "-pixel_format", "rgba",
            "-video_size", &format!("{}x{}", width, height),
            "-framerate", &format!("{}", fps),
            "-i", "-",
            "-vf", "format=rgb24",
            "-vcodec", "libx264",
            "-preset", "veryfast",
            "-crf", "23",
            "-pix_fmt", "yuv420p",
            &filename,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()?;

    let mut stdin = child.stdin.take().expect("ffmpeg stdin");

    thread::spawn(move || {
        while let Ok(frame) = rx.recv() {
            if let Err(e) = stdin.write_all(&frame) {
                eprintln!("ffmpeg stdin write error: {}", e);
                break;
            }
        }
        let _ = stdin.flush();
    });

    Ok(FfmpegHandle { child })
}

