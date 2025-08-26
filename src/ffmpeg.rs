use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Receiver;
use std::thread;

pub struct FfmpegHandle {
    pub child: Child,
}

pub fn spawn_ffmpeg(
    width: u32,
    height: u32,
    fps: u32,
    rx: Receiver<Vec<u8>>,
) -> std::io::Result<FfmpegHandle> {
    let filename = format!(
        "./output/video/{}_{}.mp4",
        fps,
        chrono::Utc::now().format("%Y-%m-%d_%H-%M-%S")
    );
    // Write to MP4 (faststart) and a UDP MPEG-TS preview simultaneously via tee
    // Avoid empty_moov to improve compatibility; let ffmpeg finalize moov at exit
    let tee_outputs = format!(
        "[f=mp4:movflags=+faststart]{}|[f=mpegts]udp://127.0.0.1:12345?pkt_size=1316",
        filename
    );

    let mut child = Command::new("ffmpeg")
        .args(&[
            "-y",
            // raw RGBA frames on stdin
            "-f", "rawvideo",
            "-pix_fmt", "rgba",
            "-video_size", &format!("{}x{}", width, height),
            "-framerate", &format!("{}", fps),
            "-i", "-",
            // convert to RGB24 for x264 and then encode YUV420p
            "-vf", "format=rgb24",
            "-c:v", "libx264",
            "-preset", "veryfast",
            "-tune", "zerolatency",
            // keyframe cadence helps fragmented MP4 and UDP preview resilience
            "-g", &format!("{}", fps * 2),
            "-pix_fmt", "yuv420p",
            // map the video stream explicitly for tee
            "-map", "0:v:0",
            // tee to mp4 file and UDP preview
            "-f", "tee",
            &tee_outputs,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;

    // Forward ffmpeg stderr with a prefix so errors are visible
    if let Some(stderr) = child.stderr.take() {
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines().flatten() {
                eprintln!("[ffmpeg] {}", line);
            }
        });
    }

    let mut stdin = child.stdin.take().expect("ffmpeg stdin");
    let expected = (width as usize) * (height as usize) * 4;

    thread::spawn(move || {
        while let Ok(frame) = rx.recv() {
            if frame.len() != expected {
                eprintln!("[diag] bad frame size {} (expected {})", frame.len(), expected);
                continue;
            }
            if let Err(e) = stdin.write_all(&frame) {
                eprintln!("[ffmpeg] stdin write error: {}", e);
                break;
            }
        }
        let _ = stdin.flush();
    });

    Ok(FfmpegHandle { child })
}
