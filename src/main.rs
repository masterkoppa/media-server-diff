extern crate ffmpeg_next as ffmpeg;
extern crate clap;
extern crate walkdir;

use clap::Parser;
use std::path::PathBuf;
use walkdir::{DirEntry, WalkDir};
use rayon::prelude::*;
use tracing::{info,instrument,debug,warn};
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
    if !path.is_dir() {}

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
        .filter_map(analyze_path)
        .collect();


    Some(results.join("\n"))
}

/// Given a path, return a textual description of the media file that can
/// be used to differentiate between multiple copies of the same data set
/// that have diverged
#[instrument]
#[allow(clippy::ptr_arg)]
fn analyze_path(path: &PathBuf) -> Option<String> {
    match ffmpeg::format::input(path) {
        Ok(context) => {
            debug!(mime_types = context.format().mime_types().join(",").as_str());

            if !context.format().mime_types().into_iter().any(|mime_type| {
                // If mime types are available, ensure that they are valid for our purposes
                mime_type.starts_with("audio") || mime_type.starts_with("video")
            }) {}

            let file_name = path.to_string_lossy();
            let duration = format_duration(&Duration::from_micros(
                context.duration()
                    .try_into()
                    .unwrap_or(0)
            ));

            let bit_rate =  format_bit_rate(context.bit_rate());

            let mut stream_descriptions: Vec<String> = vec!();

            // Calculate the best "streams" available
            if let Some(stream) = context.streams().best(ffmpeg::media::Type::Video) {
                println!("Best video stream index: {}", stream.index());
                stream_descriptions.push(format!("Video: {} kb/s", stream.rate()));

                for (k, v) in stream.metadata().iter() {
                    debug!("{}: {}", k, v);
                }
            }

            if let Some(stream) = context.streams().best(ffmpeg::media::Type::Audio) {
                println!("Best video stream index: {}", stream.index());
                stream_descriptions.push(format!("Audio: {} kb/s", stream.rate()));

                for (k, v) in stream.metadata().iter() {
                    debug!("{}: {}", k, v);
                }
            }

            for stream in context.streams() {
                debug!("Stream Index: {}", stream.index());
                for (k, v) in stream.metadata().iter() {
                    debug!("{}: {}", k, v);
                }

                
            }

            Some(format!(
                "{}\n\tDuration: {}\n\tBit rate: {}\n\t{}",
                file_name,
                duration,
                bit_rate,
                stream_descriptions.join("\n\t")
            ))
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

/// Format a base 10 bit rate number into a human readable format
fn format_bit_rate(bit_rate: i64) -> String {
    if bit_rate > 1_000_000 {
        format!("{:.2} MB/s", (bit_rate as f64) / 1_000_000.0)
    } else if bit_rate > 1000 {
        format!("{:.2} KB/s", (bit_rate as f64) / 1_000.0)
    } else {
        format!("{} B/s", bit_rate)
    }
}

/// Format the duration in a specified human readable format
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

    result.push_str(&format!("{:02}:", minutes % 60));
    result.push_str(&format!("{:02}", duration.as_secs() % 60));

    if duration.subsec_nanos() as f64 * 1e-7 > 0.0 {
        result.push_str(&format!(".{}", (duration.subsec_nanos() as f64 * 1e-7) as u64));
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
        let seconds = Duration::from_secs_f32(1.12);
        assert_eq!(format_duration(&seconds), "00:01.12");

        let seconds_leftover = Duration::from_secs_f32(1.1233);
        assert_eq!(format_duration(&seconds_leftover), "00:01.12");
    }

    #[test]
    fn test_megabytes() {
        let megabytes_per_sec = 12_000_000;
        assert_eq!(format_bit_rate(megabytes_per_sec), "12.00 MB/s")
    }

    #[test]
    fn test_kilobytes() {
        let kilobytes_per_sec = 12_000;
        assert_eq!(format_bit_rate(kilobytes_per_sec), "12.00 KB/s")
    }

    #[test]
    fn test_bytes() {
        let bytes_per_sec = 12;
        assert_eq!(format_bit_rate(bytes_per_sec), "12 B/s")
    }
}