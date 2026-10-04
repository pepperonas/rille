//! Plausibility run on real files: BPM, first beat and timing per track.
//! `cargo run --release -p rille-library --example analyze -- <folder or files…>`
use std::path::PathBuf;
use std::time::Instant;

use rille_library::analysis::analyze_file;
use rille_library::scan::scan;
use rille_library::tags::read_info;

fn main() {
    let mut files: Vec<PathBuf> = Vec::new();
    for arg in std::env::args().skip(1) {
        let p = PathBuf::from(arg);
        if p.is_dir() {
            files.extend(scan(&p));
        } else {
            files.push(p);
        }
    }
    let started = Instant::now();
    for path in &files {
        let name = read_info(path)
            .map(|i| match i.artist {
                Some(a) => format!("{a} – {}", i.title),
                None => i.title,
            })
            .unwrap_or_else(|_| path.display().to_string());
        let t = Instant::now();
        match analyze_file(path, || true) {
            Ok(a) => {
                let reading = a
                    .reading
                    .map(|r| {
                        format!(
                            "coarse {:5.1}, sharpness {:4.1}, consistency {:3.0} %",
                            r.coarse,
                            r.sharpness,
                            r.consistency * 100.0
                        )
                    })
                    .unwrap_or_default();
                println!(
                    "{:>7}  {reading}  {:>5.0} ms  {name}",
                    a.grid
                        .map(|g| format!("{:.2}", g.bpm))
                        .unwrap_or_else(|| "–".into()),
                    t.elapsed().as_secs_f64() * 1000.0,
                )
            }
            Err(e) => println!("  error  {name}: {e}"),
        }
    }
    println!(
        "{} files in {:.1} s",
        files.len(),
        started.elapsed().as_secs_f64()
    );
}
