//! The track library at runtime: import (scan → tags → database), a background analysis queue
//! (BPM/beatgrid and waveform peaks) and lookups for loading a library track onto a deck.
//!
//! Analysis runs on `cores − 2` worker threads at macOS QoS "utility", so the audio thread and
//! the UI keep their cores. The database sits behind one mutex; every access is a short query,
//! the long analysis never holds it.

use std::collections::{HashSet, VecDeque};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;

use rille_library::analysis::analyze_file;
use rille_library::db::{AnalysisState, DbError, FileStamp, Library, TrackRow};
use rille_library::scan::{is_audio_file, scan};
use rille_library::tags::{info_from_file_name, read_info};
use serde::Serialize;

/// Tracks per `library-tracks` event while importing: few events, none huge.
const IMPORT_BATCH: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Analysis {
    Pending,
    Done,
    Failed,
}

impl From<AnalysisState> for Analysis {
    fn from(s: AnalysisState) -> Analysis {
        match s {
            AnalysisState::Pending => Analysis::Pending,
            AnalysisState::Done => Analysis::Done,
            AnalysisState::Failed => Analysis::Failed,
        }
    }
}

/// A track as the frontend lists it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryTrack {
    pub id: i64,
    pub path: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<f64>,
    pub bpm: Option<f64>,
    pub analysis: Analysis,
    pub error: Option<String>,
}

impl From<TrackRow> for LibraryTrack {
    fn from(t: TrackRow) -> LibraryTrack {
        LibraryTrack {
            id: t.id,
            path: t.path.to_string_lossy().into_owned(),
            title: t.title,
            artist: t.artist,
            album: t.album,
            duration: t.duration,
            bpm: t.grid.map(|g| g.bpm),
            analysis: t.analysis.into(),
            error: t.error,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryProgress {
    /// An import (scan + tags) is running.
    pub importing: bool,
    /// Tracks queued for analysis since the queue was last idle.
    pub queued: u32,
    /// Of those, finished (done or failed).
    pub analyzed: u32,
}

impl LibraryProgress {
    #[cfg(test)]
    pub fn idle(&self) -> bool {
        !self.importing && self.analyzed == self.queued
    }
}

pub enum LibraryEvent {
    /// Added or changed tracks; the frontend merges them by id.
    Tracks(Vec<LibraryTrack>),
    Progress(LibraryProgress),
}

pub type LibraryNotify = Arc<dyn Fn(LibraryEvent) + Send + Sync>;

/// What to put on a deck for a library track.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadInfo {
    pub path: PathBuf,
    pub title: String,
    pub artist: Option<String>,
    pub bpm: Option<f64>,
}

impl From<&TrackRow> for LoadInfo {
    fn from(t: &TrackRow) -> LoadInfo {
        LoadInfo {
            path: t.path.clone(),
            title: t.title.clone(),
            artist: t.artist.clone(),
            bpm: t.grid.map(|g| g.bpm),
        }
    }
}

/// Analysis queue bookkeeping, without threads (tested on its own).
#[derive(Default)]
struct Queue {
    jobs: VecDeque<(i64, PathBuf)>,
    /// Queued or being analysed; a track is never in the queue twice.
    known: HashSet<i64>,
    in_flight: u32,
    imports: u32,
    queued: u32,
    analyzed: u32,
    shutdown: bool,
}

impl Queue {
    fn push(&mut self, id: i64, path: PathBuf) -> bool {
        if !self.known.insert(id) {
            return false;
        }
        self.jobs.push_back((id, path));
        self.queued += 1;
        true
    }

    fn take(&mut self) -> Option<(i64, PathBuf)> {
        let job = self.jobs.pop_front()?;
        self.in_flight += 1;
        Some(job)
    }

    fn finish(&mut self, id: i64) {
        self.known.remove(&id);
        self.in_flight = self.in_flight.saturating_sub(1);
        self.analyzed += 1;
    }

    fn progress(&self) -> LibraryProgress {
        LibraryProgress {
            importing: self.imports > 0,
            queued: self.queued,
            analyzed: self.analyzed,
        }
    }

    /// Start counting afresh once everything is through, so the next import shows 0 / n.
    fn reset_if_idle(&mut self) {
        if self.imports == 0 && self.jobs.is_empty() && self.in_flight == 0 {
            self.queued = 0;
            self.analyzed = 0;
        }
    }
}

struct Shared {
    db: Mutex<Library>,
    peaks_dir: PathBuf,
    queue: Mutex<Queue>,
    wake: Condvar,
    notify: LibraryNotify,
    stopping: AtomicBool,
}

#[derive(Clone)]
pub struct LibraryService {
    shared: Arc<Shared>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Worker threads for analysis: leave two cores for audio and UI, but at least one.
pub fn worker_count(cores: usize) -> usize {
    cores.saturating_sub(2).max(1)
}

impl LibraryService {
    /// Open the database and start the workers. Tracks whose analysis was interrupted (app
    /// quit mid-way) are queued again.
    pub fn start(
        db_path: &Path,
        peaks_dir: PathBuf,
        workers: usize,
        notify: LibraryNotify,
    ) -> Result<LibraryService, DbError> {
        if let Some(dir) = db_path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::create_dir_all(&peaks_dir);
        let db = Library::open(db_path)?;
        let pending = db.pending()?;
        let service = LibraryService {
            shared: Arc::new(Shared {
                db: Mutex::new(db),
                peaks_dir,
                queue: Mutex::new(Queue::default()),
                wake: Condvar::new(),
                notify,
                stopping: AtomicBool::new(false),
            }),
        };
        for n in 0..workers.max(1) {
            service.spawn_worker(n);
        }
        service.enqueue(pending);
        Ok(service)
    }

    pub fn tracks(&self) -> Result<Vec<LibraryTrack>, DbError> {
        Ok(lock(&self.shared.db)
            .tracks()?
            .into_iter()
            .map(LibraryTrack::from)
            .collect())
    }

    pub fn progress(&self) -> LibraryProgress {
        lock(&self.shared.queue).progress()
    }

    pub fn load_info(&self, id: i64) -> Option<LoadInfo> {
        let row = lock(&self.shared.db).track(id).ok()??;
        Some(LoadInfo::from(&row))
    }

    /// Library data for a file dropped from the Finder, if it was imported before.
    pub fn load_info_for_path(&self, path: &Path) -> Option<LoadInfo> {
        let row = lock(&self.shared.db).track_by_path(path).ok()??;
        Some(LoadInfo::from(&row))
    }

    pub fn peaks_path(&self, id: i64) -> PathBuf {
        self.shared.peaks_dir.join(format!("{id}.bin"))
    }

    /// Import folders and/or files in the background: scan, read tags, add to the database,
    /// queue the analysis. Folders are remembered as library roots.
    pub fn import(&self, paths: Vec<PathBuf>) {
        {
            let mut q = lock(&self.shared.queue);
            q.imports += 1;
            (self.shared.notify)(LibraryEvent::Progress(q.progress()));
        }
        let service = self.clone();
        let spawned = thread::Builder::new()
            .name("rille-import".into())
            .spawn(move || {
                // Whatever happens inside, the import counter must come down again.
                if catch_unwind(AssertUnwindSafe(|| service.run_import(&paths))).is_err() {
                    tracing::error!("import panicked");
                }
                let mut q = lock(&service.shared.queue);
                q.imports = q.imports.saturating_sub(1);
                q.reset_if_idle();
                (service.shared.notify)(LibraryEvent::Progress(q.progress()));
            });
        if let Err(e) = spawned {
            tracing::error!(%e, "could not start import");
            let mut q = lock(&self.shared.queue);
            q.imports = q.imports.saturating_sub(1);
        }
    }

    fn run_import(&self, paths: &[PathBuf]) {
        let mut batch = Vec::new();
        let mut jobs = Vec::new();
        for root in paths {
            let files = if root.is_dir() {
                if let Err(e) = lock(&self.shared.db).add_root(root) {
                    tracing::warn!(%e, "could not remember library folder");
                }
                scan(root)
            } else if is_audio_file(root) {
                vec![root.clone()]
            } else {
                Vec::new()
            };
            for file in files {
                if let Some((track, pending)) = self.add_file(&file) {
                    if pending {
                        jobs.push((track.id, file));
                    }
                    batch.push(track);
                }
                if batch.len() >= IMPORT_BATCH {
                    (self.shared.notify)(LibraryEvent::Tracks(std::mem::take(&mut batch)));
                    self.enqueue(std::mem::take(&mut jobs));
                }
            }
        }
        if !batch.is_empty() {
            (self.shared.notify)(LibraryEvent::Tracks(batch));
        }
        self.enqueue(jobs);
    }

    /// One file into the database. Unreadable headers still get a row (named after the file)
    /// with the analysis marked failed, so the user sees which files did not work.
    fn add_file(&self, file: &Path) -> Option<(LibraryTrack, bool)> {
        let stamp = match FileStamp::of(file) {
            Ok(stamp) => stamp,
            Err(e) => {
                tracing::warn!(path = %file.display(), %e, "skipping unreadable file");
                return None;
            }
        };
        let (info, failure) = match catch_unwind(AssertUnwindSafe(|| read_info(file))) {
            Ok(Ok(info)) => (info, None),
            Ok(Err(e)) => (info_from_file_name(file), Some(e.to_string())),
            Err(_) => (
                info_from_file_name(file),
                Some("Der Decoder ist an dieser Datei gescheitert".to_string()),
            ),
        };
        let db = lock(&self.shared.db);
        let result = (|| {
            let (id, pending) = db.upsert(file, stamp, &info)?;
            let pending = match &failure {
                Some(message) if pending => {
                    db.set_failed(id, message)?;
                    false
                }
                _ => pending,
            };
            Ok::<_, DbError>((db.track(id)?, pending))
        })();
        match result {
            Ok((Some(row), pending)) => Some((row.into(), pending)),
            Ok((None, _)) => None,
            Err(e) => {
                tracing::warn!(path = %file.display(), %e, "could not add track");
                None
            }
        }
    }

    fn enqueue(&self, jobs: Vec<(i64, PathBuf)>) {
        if jobs.is_empty() {
            return;
        }
        let mut q = lock(&self.shared.queue);
        for (id, path) in jobs {
            q.push(id, path);
        }
        (self.shared.notify)(LibraryEvent::Progress(q.progress()));
        drop(q);
        self.shared.wake.notify_all();
    }

    fn spawn_worker(&self, n: usize) {
        let service = self.clone();
        let spawned = thread::Builder::new()
            .name(format!("rille-analysis-{}", n + 1))
            .spawn(move || {
                lower_priority();
                while let Some((id, path)) = service.next_job() {
                    let track = service.analyze(id, &path);
                    let mut q = lock(&service.shared.queue);
                    q.finish(id);
                    let progress = q.progress();
                    q.reset_if_idle();
                    drop(q);
                    if let Some(track) = track {
                        (service.shared.notify)(LibraryEvent::Tracks(vec![track]));
                    }
                    (service.shared.notify)(LibraryEvent::Progress(progress));
                }
            });
        if let Err(e) = spawned {
            tracing::error!(%e, "could not start analysis worker");
        }
    }

    fn next_job(&self) -> Option<(i64, PathBuf)> {
        let mut q = lock(&self.shared.queue);
        loop {
            if q.shutdown {
                return None;
            }
            if let Some(job) = q.take() {
                return Some(job);
            }
            q = self
                .shared
                .wake
                .wait(q)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
    }

    /// Analyse one track and store the result. Returns the updated row.
    fn analyze(&self, id: i64, path: &Path) -> Option<LibraryTrack> {
        let stopping = &self.shared.stopping;
        let result = catch_unwind(AssertUnwindSafe(|| {
            analyze_file(path, || !stopping.load(Ordering::Relaxed))
        }));
        let db_result = match result {
            Ok(Ok(analysis)) => {
                let peaks = analysis.peaks.to_bytes();
                if let Err(e) = write_atomically(&self.peaks_path(id), &peaks) {
                    tracing::warn!(%e, "could not write peaks");
                }
                lock(&self.shared.db).set_analyzed(id, analysis.grid, analysis.duration_secs)
            }
            Ok(Err(e)) => {
                if self.shared.stopping.load(Ordering::Relaxed) {
                    return None; // stays pending, resumes next start
                }
                lock(&self.shared.db).set_failed(id, &e.to_string())
            }
            Err(_) => lock(&self.shared.db).set_failed(id, "Die Analyse ist abgestürzt"),
        };
        if let Err(e) = db_result {
            tracing::warn!(%e, "could not store analysis");
        }
        let row = lock(&self.shared.db).track(id).ok()??;
        Some(row.into())
    }

    /// Stop the workers (used by tests; the app just exits).
    #[cfg(test)]
    pub fn stop(&self) {
        self.shared.stopping.store(true, Ordering::Relaxed);
        lock(&self.shared.queue).shutdown = true;
        self.shared.wake.notify_all();
    }
}

fn write_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// Analysis is background work: macOS QoS "utility" keeps it off the cores the audio thread
/// and the UI need.
fn lower_priority() {
    #[cfg(target_os = "macos")]
    {
        // SAFETY: plain FFI call that only changes the calling thread's scheduling class.
        let rc =
            unsafe { libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_UTILITY, 0) };
        if rc != 0 {
            tracing::debug!(rc, "could not lower analysis thread priority");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    #[test]
    fn workers_leave_two_cores_free() {
        assert_eq!(worker_count(10), 8);
        assert_eq!(worker_count(2), 1);
        assert_eq!(worker_count(1), 1);
    }

    #[test]
    fn a_track_is_queued_once_and_progress_resets_when_idle() {
        let mut q = Queue::default();
        assert!(q.push(1, "/a".into()));
        assert!(!q.push(1, "/a".into()), "already queued");
        assert!(q.push(2, "/b".into()));
        let (id, _) = q.take().unwrap();
        assert!(!q.push(id, "/a".into()), "being analysed");
        q.finish(id);
        assert_eq!(
            q.progress(),
            LibraryProgress {
                importing: false,
                queued: 2,
                analyzed: 1
            }
        );
        q.reset_if_idle();
        assert_eq!(q.progress().queued, 2, "one job still waiting");
        let (id, _) = q.take().unwrap();
        q.finish(id);
        assert!(q.progress().idle());
        q.reset_if_idle();
        assert_eq!(q.progress(), LibraryProgress::default());
    }

    #[test]
    fn progress_is_not_idle_while_importing() {
        let mut q = Queue {
            imports: 1,
            ..Queue::default()
        };
        assert!(!q.progress().idle());
        q.reset_if_idle();
        q.imports = 0;
        assert!(q.progress().idle());
    }

    /// 16-bit stereo WAV with a kick every half second (120 BPM).
    fn write_kicks(path: &Path, seconds: f32) {
        let rate = 44_100u32;
        let frames = (rate as f32 * seconds) as usize;
        let mut data = Vec::with_capacity(frames * 4);
        for i in 0..frames {
            let t = (i % (rate as usize / 2)) as f32 / rate as f32;
            let s = (t * 60.0 * std::f32::consts::TAU).sin() * (-t * 30.0).exp() * 0.8;
            let v = (s * i16::MAX as f32) as i16;
            data.extend_from_slice(&v.to_le_bytes());
            data.extend_from_slice(&v.to_le_bytes());
        }
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&rate.to_le_bytes());
        out.extend_from_slice(&(rate * 4).to_le_bytes());
        out.extend_from_slice(&4u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&data);
        std::fs::write(path, out).unwrap();
    }

    fn start(
        dir: &Path,
    ) -> (
        LibraryService,
        mpsc::Receiver<LibraryProgress>,
        Arc<Mutex<Vec<LibraryTrack>>>,
    ) {
        let (tx, rx) = mpsc::channel();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        let tx = Mutex::new(tx);
        let notify: LibraryNotify = Arc::new(move |event| match event {
            LibraryEvent::Progress(p) => {
                let _ = lock(&tx).send(p);
            }
            LibraryEvent::Tracks(t) => lock(&sink).extend(t),
        });
        let service =
            LibraryService::start(&dir.join("db/library.db"), dir.join("peaks"), 2, notify)
                .unwrap();
        (service, rx, seen)
    }

    fn wait_idle(rx: &mpsc::Receiver<LibraryProgress>) {
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut started = false;
        while Instant::now() < deadline {
            if let Ok(p) = rx.recv_timeout(Duration::from_millis(100)) {
                started |= p.importing || p.queued > 0;
                if started && p.idle() {
                    return;
                }
            }
        }
        panic!("library did not become idle");
    }

    #[test]
    fn import_analyses_tracks_and_marks_broken_files() {
        let dir = tempfile::tempdir().unwrap();
        let music = dir.path().join("music");
        std::fs::create_dir_all(music.join("sub")).unwrap();
        write_kicks(&music.join("DJ - Kicks.wav"), 12.0);
        write_kicks(&music.join("sub/Second.wav"), 12.0);
        std::fs::write(music.join("broken.mp3"), b"not audio at all").unwrap();
        std::fs::write(music.join("notes.txt"), b"ignored").unwrap();

        let (service, rx, seen) = start(dir.path());
        service.import(vec![music.clone()]);
        wait_idle(&rx);

        let tracks = service.tracks().unwrap();
        assert_eq!(tracks.len(), 3, "{tracks:?}");
        let kicks = tracks.iter().find(|t| t.title == "Kicks").unwrap();
        assert_eq!(kicks.artist.as_deref(), Some("DJ"));
        assert_eq!(kicks.analysis, Analysis::Done);
        let bpm = kicks.bpm.unwrap();
        assert!((bpm - 120.0).abs() < 0.1, "bpm {bpm}");
        assert!(service.peaks_path(kicks.id).is_file());
        let broken = tracks.iter().find(|t| t.title == "broken").unwrap();
        assert_eq!(broken.analysis, Analysis::Failed);
        assert!(broken.error.is_some());
        // The frontend heard about every track, the analysed ones with their result.
        let seen = lock(&seen).clone();
        assert!(
            seen.iter()
                .any(|t| t.id == kicks.id && t.analysis == Analysis::Done)
        );
        assert!(seen.iter().any(|t| t.id == broken.id));

        let info = service.load_info(kicks.id).unwrap();
        assert_eq!(info.path, music.join("DJ - Kicks.wav"));
        assert_eq!(service.load_info_for_path(&info.path), Some(info));
        service.stop();
    }

    #[test]
    fn interrupted_analysis_resumes_on_the_next_start() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("Track.wav");
        write_kicks(&file, 8.0);
        {
            // Add the track without letting the workers run.
            std::fs::create_dir_all(dir.path().join("db")).unwrap();
            let db = Library::open(&dir.path().join("db/library.db")).unwrap();
            let info = read_info(&file).unwrap();
            db.upsert(&file, FileStamp::of(&file).unwrap(), &info)
                .unwrap();
        }
        let (service, rx, _) = start(dir.path());
        wait_idle(&rx);
        let tracks = service.tracks().unwrap();
        assert_eq!(tracks[0].analysis, Analysis::Done);
        service.stop();
    }
}
