//! What the trimming window asks ffmpeg and ffprobe for, as argument vectors, and the names it saves under: plain
//! functions, so that the tests hold them. Every path given to ffmpeg is absolute, so that none can be read as an
//! option, and a cut is written with -n, which never writes over a file.

use std::path::{Path, PathBuf};

/// Frames on the strip, and its height in pixels
pub const STRIP_FRAMES: u32 = 10;
pub const STRIP_HEIGHT: u32 = 64;
/// The preview's height, a GIF's frames a second and its widest
pub const PREVIEW_HEIGHT: u32 = 360;
pub const GIF_FPS: u32 = 12;
pub const GIF_WIDTH: u32 = 960;
/// The shortest part that can be kept, in seconds
pub const SHORTEST: f64 = 0.5;

/// What a cut is saved as
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Webm,
    Gif,
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Format::Webm => "webm",
            Format::Gif => "gif",
        }
    }
}

/// Seconds as a time on a clock: 0:04, 12:30, 1:02:03
pub fn clock(seconds: f64) -> String {
    let total = if seconds.is_finite() && seconds > 0.0 { seconds.round() as u64 } else { 0 };
    let (hours, minutes, secs) = (total / 3600, total / 60 % 60, total % 60);
    if hours > 0 { format!("{hours}:{minutes:02}:{secs:02}") } else { format!("{minutes}:{secs:02}") }
}

/// Seconds as ffmpeg reads them, to the millisecond
fn seconds(value: f64) -> String {
    format!("{:.3}", value.max(0.0))
}

/// The length ffprobe says a recording has, in seconds, or None
pub fn parse_duration(stdout: &str) -> Option<f64> {
    let value: f64 = stdout.lines().map(str::trim).find(|line| !line.is_empty())?.parse().ok()?;
    (value.is_finite() && value > 0.0).then_some(value)
}

pub fn probe_args(input: &Path) -> Vec<String> {
    vec!["-v".into(), "error".into(), "-show_entries".into(), "format=duration".into(), "-of".into(),
        "default=noprint_wrappers=1:nokey=1".into(), input.display().to_string()]
}

/// One picture of the whole recording, ten frames side by side, for the strip
pub fn strip_args(input: &Path, duration: f64, out: &Path) -> Vec<String> {
    let rate = f64::from(STRIP_FRAMES) / duration.max(SHORTEST);
    vec![
        "-nostdin".into(), "-v".into(), "error".into(), "-i".into(), input.display().to_string(),
        "-vf".into(), format!("fps={rate:.6},scale=-2:{STRIP_HEIGHT},tile={STRIP_FRAMES}x1"),
        "-frames:v".into(), "1".into(), "-y".into(), out.display().to_string(),
    ]
}

/// The frame at `at` seconds, for the preview
pub fn frame_args(input: &Path, at: f64, out: &Path) -> Vec<String> {
    vec![
        "-nostdin".into(), "-v".into(), "error".into(), "-ss".into(), seconds(at), "-i".into(), input.display().to_string(),
        "-frames:v".into(), "1".into(), "-vf".into(), format!("scale=-2:{PREVIEW_HEIGHT}"), "-y".into(),
        out.display().to_string(),
    ]
}

/// The part from `start` to `end` seconds, as WebM (VP9, as the recorder's own WebM is in a browser) or as a GIF of
/// its own palette; the sound, which GNOME's recordings have none of, is left out
pub fn cut_args(input: &Path, start: f64, end: f64, format: Format, out: &Path) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "-nostdin".into(), "-v".into(), "error".into(), "-n".into(), "-ss".into(), seconds(start), "-t".into(),
        seconds((end - start).max(SHORTEST)), "-i".into(), input.display().to_string(), "-an".into(),
    ];
    match format {
        Format::Webm => args.extend(
            ["-c:v", "libvpx-vp9", "-row-mt", "1", "-deadline", "good", "-cpu-used", "4", "-crf", "34", "-b:v", "0"]
                .map(String::from),
        ),
        Format::Gif => {
            args.push("-vf".into());
            args.push(format!(
                "fps={GIF_FPS},scale='min({GIF_WIDTH},iw)':-2:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4"
            ));
            args.extend(["-loop", "0"].map(String::from));
        }
    }
    args.push(out.display().to_string());
    args
}

/// Where a cut is saved: beside the recording, "NAME (trimmed).EXT", or with a number when that is taken
pub fn output_path(input: &Path, format: Format, exists: impl Fn(&Path) -> bool) -> PathBuf {
    let folder = input.parent().unwrap_or_else(|| Path::new("/"));
    let stem = input.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_else(|| "recording".into());
    let extension = format.extension();
    let mut candidate = folder.join(format!("{stem} (trimmed).{extension}"));
    let mut n = 2;
    while exists(&candidate) {
        candidate = folder.join(format!("{stem} (trimmed {n}).{extension}"));
        n += 1;
    }
    candidate
}

/// The two ends held apart and inside the recording: the end moved by hand pushes nothing, it stops at the other
pub fn clamp_ends(start: f64, end: f64, duration: f64, moved_start: bool) -> (f64, f64) {
    let duration = duration.max(SHORTEST);
    let mut start = start.clamp(0.0, duration);
    let mut end = end.clamp(0.0, duration);
    if end - start < SHORTEST {
        if moved_start {
            start = (end - SHORTEST).max(0.0);
            end = (start + SHORTEST).min(duration);
        } else {
            end = (start + SHORTEST).min(duration);
            start = (end - SHORTEST).max(0.0);
        }
    }
    (start, end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_as_a_clock_shows_them() {
        assert_eq!(clock(4.2), "0:04");
        assert_eq!(clock(52.0), "0:52");
        assert_eq!(clock(750.4), "12:30");
        assert_eq!(clock(3723.0), "1:02:03");
        assert_eq!(clock(f64::NAN), "0:00");
        assert_eq!(clock(-3.0), "0:00");
    }

    #[test]
    fn durations_ffprobe_prints() {
        assert_eq!(parse_duration("52.480000\n"), Some(52.48));
        assert_eq!(parse_duration("N/A\n"), None);
        assert_eq!(parse_duration(""), None);
        assert_eq!(parse_duration("0\n"), None);
    }

    #[test]
    fn names_beside_the_recording_never_over_one() {
        let input = Path::new("/home/a/Videos/Screencasts/Screencast From 2026-10-31.webm");
        let taken = |path: &Path| path.ends_with("Screencast From 2026-10-31 (trimmed).webm");
        assert_eq!(output_path(input, Format::Gif, |_| false),
            Path::new("/home/a/Videos/Screencasts/Screencast From 2026-10-31 (trimmed).gif"));
        assert_eq!(output_path(input, Format::Webm, taken),
            Path::new("/home/a/Videos/Screencasts/Screencast From 2026-10-31 (trimmed 2).webm"));
    }

    #[test]
    fn a_cut_is_an_argument_vector_that_never_overwrites() {
        let input = Path::new("/v/in.webm");
        let out = Path::new("/v/in (trimmed).webm");
        let args = cut_args(input, 4.0, 41.5, Format::Webm, out);
        assert!(args.contains(&"-n".to_string()));
        assert!(!args.contains(&"-y".to_string()));
        let at = |word: &str| args.iter().position(|arg| arg == word).unwrap();
        assert_eq!(args[at("-ss") + 1], "4.000");
        assert_eq!(args[at("-t") + 1], "37.500");
        assert_eq!(args[at("-i") + 1], "/v/in.webm");
        assert_eq!(args.last().unwrap(), "/v/in (trimmed).webm");
        let gif = cut_args(input, 0.0, 3.0, Format::Gif, Path::new("/v/in.gif"));
        assert!(gif.iter().any(|arg| arg.contains("palettegen") && arg.contains("paletteuse")));
        assert!(gif.windows(2).any(|pair| pair == ["-loop", "0"]));
    }

    #[test]
    fn the_ends_stay_apart_and_inside() {
        let ends = |s: f64, e: f64, moved_start: bool| {
            let (s, e) = clamp_ends(s, e, 52.0, moved_start);
            ((s * 1000.0).round() / 1000.0, (e * 1000.0).round() / 1000.0)
        };
        assert_eq!(ends(-1.0, 99.0, true), (0.0, 52.0));
        assert_eq!(ends(10.0, 10.2, true), (9.7, 10.2));
        assert_eq!(ends(10.0, 10.2, false), (10.0, 10.5));
        assert_eq!(ends(52.0, 52.0, false), (51.5, 52.0));
    }

    #[test]
    fn frames_for_the_strip_and_the_preview() {
        let strip = strip_args(Path::new("/v/in.webm"), 50.0, Path::new("/run/t/strip.png"));
        assert!(strip.iter().any(|arg| arg == "fps=0.200000,scale=-2:64,tile=10x1"));
        let frame = frame_args(Path::new("/v/in.webm"), 12.25, Path::new("/run/t/frame.png"));
        assert_eq!(frame[frame.iter().position(|arg| arg == "-ss").unwrap() + 1], "12.250");
    }
}
