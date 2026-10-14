//! MP3 encoding through the user's own `lame` program, run as a separate process (never linked: LAME is
//! LGPL software and this keeps it out of Tau Omega's own licence). Raw 16-bit PCM goes in over a pipe.

use crate::pipeline::{Pipeline, Quantizer};
use crate::{AudioError, Encoded};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// How to encode. `Vbr(0)` is the best variable-rate quality (about 245 kbit/s); `Cbr` is a constant rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Vbr(u8),
    Cbr(u16),
}

impl Quality {
    fn args(self) -> Vec<String> {
        match self {
            Self::Vbr(q) => vec!["-V".into(), q.min(9).to_string()],
            Self::Cbr(k) => vec!["-b".into(), k.to_string()],
        }
    }
}

/// The tags `lame` writes itself (ID3v2). Everything is optional and ASCII-safe text is the caller's job.
#[derive(Debug, Clone, Default)]
pub struct Tags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<String>,
    pub track: Option<String>,
    pub genre: Option<String>,
}

fn names() -> &'static [&'static str] {
    if cfg!(windows) {
        &["lame.exe"]
    } else {
        &["lame"]
    }
}

/// Finds a usable `lame`: the user's chosen path, the app's own managed copy, `PATH`, then the usual
/// package-manager locations.
pub fn find(custom: Option<&Path>, managed_dir: Option<&Path>) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(c) = custom {
        candidates.push(c.to_path_buf());
    }
    if let Some(dir) = managed_dir {
        candidates.extend(names().iter().map(|n| dir.join(n)));
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            candidates.extend(names().iter().map(|n| dir.join(n)));
        }
    }
    for dir in [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/usr/bin",
        "/opt/local/bin",
        "C:\\Program Files\\LAME",
        "C:\\Program Files (x86)\\LAME",
    ] {
        candidates.extend(names().iter().map(|n| Path::new(dir).join(n)));
    }
    candidates
        .into_iter()
        .find(|p| p.is_file() && version(p).is_ok())
}

/// Runs `lame --version` and returns its first line, proving the program works.
pub fn version(lame: &Path) -> Result<String, AudioError> {
    let out = Command::new(lame)
        .arg("--version")
        .output()
        .map_err(|e| AudioError::Lame(format!("could not run {}: {e}", lame.display())))?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let line = text.lines().next().unwrap_or("").trim().to_string();
    if !out.status.success() || !line.to_ascii_lowercase().contains("lame") {
        return Err(AudioError::Lame(format!("{} is not LAME", lame.display())));
    }
    Ok(line)
}

/// Encodes the pipeline to `out` as MP3. `cover` is a JPEG or PNG to embed. Returns the frames encoded.
pub fn encode(
    lame: &Path,
    pipeline: &mut Pipeline,
    quality: Quality,
    tags: &Tags,
    cover: Option<&[u8]>,
    out: &Path,
    progress: &mut dyn FnMut(u64) -> bool,
) -> Result<Encoded, AudioError> {
    let rate = pipeline.target.rate;
    let channels = pipeline.target.channels;
    let mut cmd = Command::new(lame);
    cmd.args(["--silent", "-r", "-s"])
        .arg(format!("{}", rate as f64 / 1000.0))
        .args(["--bitwidth", "16", "--signed", "--little-endian"])
        .args(["-m", if channels == 1 { "m" } else { "j" }])
        .args(quality.args())
        .arg("--id3v2-only");
    for (flag, value) in [
        ("--tt", &tags.title),
        ("--ta", &tags.artist),
        ("--tl", &tags.album),
        ("--ty", &tags.year),
        ("--tn", &tags.track),
        ("--tg", &tags.genre),
    ] {
        if let Some(v) = value.as_deref().filter(|v| !v.is_empty()) {
            cmd.arg(flag).arg(v);
        }
    }
    let cover_file = match cover {
        Some(bytes) => {
            let path = out.with_extension("cover.tmp");
            std::fs::write(&path, bytes)?;
            cmd.arg("--ti").arg(&path);
            Some(path)
        }
        None => None,
    };
    cmd.arg("-")
        .arg(out)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| AudioError::Lame(format!("could not start lame: {e}")))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| AudioError::Lame("no pipe to lame".into()))?;
    let mut quant = Quantizer::new(16, true);
    let mut buf = Vec::new();
    let mut bytes = Vec::new();
    let mut frames = 0u64;
    let mut peak = 0.0f64;
    let mut failure: Option<AudioError> = None;
    loop {
        buf.clear();
        let n = match pipeline.read(8192, &mut buf) {
            Ok(n) => n,
            Err(e) => {
                failure = Some(e);
                break;
            }
        };
        if n == 0 {
            break;
        }
        frames += n as u64;
        if !progress(frames) {
            failure = Some(AudioError::Process("cancelled".into()));
            break;
        }
        bytes.clear();
        for s in &buf {
            peak = peak.max(s.abs());
            bytes.extend((quant.to_i32(*s) as i16).to_le_bytes());
        }
        if stdin.write_all(&bytes).is_err() {
            break; // lame exited; its status explains why
        }
    }
    drop(stdin);
    let output = child
        .wait_with_output()
        .map_err(|e| AudioError::Lame(e.to_string()))?;
    if let Some(path) = cover_file {
        let _ = std::fs::remove_file(path);
    }
    if let Some(e) = failure {
        let _ = std::fs::remove_file(out);
        return Err(e);
    }
    if !output.status.success() {
        let _ = std::fs::remove_file(out);
        let text = String::from_utf8_lossy(&output.stderr);
        return Err(AudioError::Lame(format!(
            "lame failed: {}",
            text.lines().last().unwrap_or("no message")
        )));
    }
    Ok(Encoded { frames, peak })
}
