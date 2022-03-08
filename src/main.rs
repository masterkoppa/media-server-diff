extern crate ffmpeg_next as ffmpeg;
extern crate clap;
extern crate walkdir;

use clap::Parser;
use std::path::PathBuf;
use walkdir::{DirEntry, WalkDir};
use rayon::prelude::*;
use tracing::{info,instrument,debug,warn};
use tracing_subscriber;
use std::time::Duration;

/// Utility to generate reports on the media file contents for a folder
/// which can be diffed using traditional tools, like diff
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Root directory to scan
    #[clap(short, long, parse(from_os_str), value_name = "DIRECTORY")]
    root_dir: PathBuf,
}

fn main() {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    info!("Path: {}", args.root_dir.display());

    if let Some(file_contents) = generate_report(args.root_dir) {
        println!("{}", file_contents);
    }
}

fn generate_report(path: PathBuf) -> Option<String> {
    if !path.is_dir() {
        ()
    }

    let paths: Vec<PathBuf> = WalkDir::new(path).into_iter().filter_map(|e| {
        match e {
            Ok(result) => {
                if should_inspect_file(&result) {
                    Some(result.into_path())
                } else {
                    None
                }
            },
            Err(error) => {
                warn!(path = error.path().unwrap().to_str().unwrap(), "Permissions error");
                None
            }
        }
    }).collect();

    debug!(num_paths = paths.len(), "Discovered path count");
    
    let results: Vec<_> = paths.par_iter()
        .filter_map(|path| analyze_path(path))
        .collect();


    Some(results.join("\n"))
}

/// Given a path, return a textual description of the media file that can
/// be used to differentiate between multiple copies of the same data set
/// that have diverged
#[instrument]
fn analyze_path(path: &PathBuf) -> Option<String> {
    match ffmpeg::format::input(path) {
        Ok(context) => {
            for (k, v) in context.metadata().iter() {
                debug!("{}: {}", k, v);
            }

            debug!(mime_types = context.format().mime_types().join(",").as_str());

            if !context.format().mime_types().into_iter().any(|mime_type| {
                // If mime types are available, ensure that they are valid for our purposes
                mime_type.starts_with("audio") || mime_type.starts_with("video")
            }) {
                ()
            }

            let file_name = path.file_name();
            let duration = Duration::from_micros(context.duration().try_into().unwrap_or(0));
            let duration_fmt = format_duration(&duration);

            todo!("Implement formatting")
        }
        Err(_) => {
            warn!("Error processing file, ignoring");
            None
        }
    }
}


/// Validates if a given DirEntry should be used for diff purposes
/// This is a simple filter, for non-file entries and .nfo files. As needs
/// evolve more cases should be included
fn should_inspect_file(entry: &DirEntry) -> bool {
    !entry.file_type().is_dir() && !entry.file_name().to_str().unwrap().ends_with(".nfo")
}

/// Format the duration in a human readable format
/// 
/// ```
/// let days = Duration::from_seconds(115197);
/// assert_eq!(format_duration(&days), String::from("01:00:00:00"))
/// ```
fn format_duration(duration: &Duration) -> String {
    let mut result = String::default();

    let minutes = duration.as_secs() / 60;
    let hours = minutes / 60;
    let days = hours / 24;

    if days > 0 {
        result.push_str(&format!("{:02}:", days));
    }

    if hours > 0 {
        result.push_str(&format!("{:02}:", hours % 24));
    }

    if minutes > 0 {
        result.push_str(&format!("{:02}:", minutes % 60));
    }

    if duration.as_secs() as f64 > 0.0 {
        result.push_str(&format!("{:02}", (duration.as_secs() % 60) as f64 + (duration.subsec_nanos() as f64 * 1e-9)));
    }

    result
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_days_format() {
        let days = Duration::from_secs(115197);
        assert_eq!(format_duration(&days), String::from("01:07:59:57"));

        let single_day = Duration::from_secs(60 * 60 * 24);
        assert_eq!(format_duration(&single_day), String::from("01:00:00:00"));
    }

    #[test]
    fn test_hours_format() {
        let hours = Duration::from_secs(28797);
        assert_eq!(format_duration(&hours), String::from("07:59:57"));

        let single_hour = Duration::from_secs(60 * 60);
        assert_eq!(format_duration(&single_hour), String::from("01:00:00"));
    }

    #[test]
    fn test_minutes() {
        let minutes = Duration::from_secs(91);
        assert_eq!(format_duration(&minutes), String::from("01:31"));

        let single_minute = Duration::from_secs(60);
        assert_eq!(format_duration(&single_minute), String::from("01:00"));
    }

    #[test]
    fn test_seconds() {
        let seconds = Duration::from_secs_f32(11.12);
        assert_eq!(format_duration(&seconds), "11.12");
    }
}