//! The library database (SQLite): tracks, their tags and analysis results, imported folders.
//!
//! Schema versions are tracked in `PRAGMA user_version` and migrated step by step on open. A
//! database written by a newer rille is refused instead of being damaged.

use std::path::{Path, PathBuf};

use rille_core::BeatGrid;
use rusqlite::{Connection, OptionalExtension, params};

use crate::tags::TrackInfo;

/// The schema this build writes.
pub const SCHEMA_VERSION: i64 = 1;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("Library-Datenbank: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error(
        "Die Library stammt von einer neueren rille-Version (Schema {0}) und wird nicht verändert"
    )]
    TooNew(i64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisState {
    Pending,
    Done,
    Failed,
}

impl AnalysisState {
    fn as_str(self) -> &'static str {
        match self {
            AnalysisState::Pending => "pending",
            AnalysisState::Done => "done",
            AnalysisState::Failed => "failed",
        }
    }

    fn parse(s: &str) -> AnalysisState {
        match s {
            "done" => AnalysisState::Done,
            "failed" => AnalysisState::Failed,
            _ => AnalysisState::Pending,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrackRow {
    pub id: i64,
    pub path: PathBuf,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<f64>,
    pub grid: Option<BeatGrid>,
    pub analysis: AnalysisState,
    pub error: Option<String>,
}

/// What a file looks like on disk; a change means the analysis must run again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileStamp {
    pub size: u64,
    /// Modification time in whole seconds since the Unix epoch.
    pub mtime: i64,
}

impl FileStamp {
    pub fn of(path: &Path) -> std::io::Result<FileStamp> {
        let meta = std::fs::metadata(path)?;
        let mtime = meta
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        Ok(FileStamp {
            size: meta.len(),
            mtime,
        })
    }
}

pub struct Library {
    conn: Connection,
}

const COLUMNS: &str = "id, path, title, artist, album, duration, bpm, first_beat, analysis, error";

impl Library {
    pub fn open(path: &Path) -> Result<Library, DbError> {
        let conn = Connection::open(path)?;
        // WAL: the UI can read while an analysis worker writes.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Library::init(conn)
    }

    pub fn open_in_memory() -> Result<Library, DbError> {
        Library::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Library, DbError> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let mut lib = Library { conn };
        lib.migrate()?;
        Ok(lib)
    }

    pub fn schema_version(&self) -> Result<i64, DbError> {
        Ok(self
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))?)
    }

    fn migrate(&mut self) -> Result<(), DbError> {
        let version = self.schema_version()?;
        if version > SCHEMA_VERSION {
            return Err(DbError::TooNew(version));
        }
        let tx = self.conn.transaction()?;
        if version < 1 {
            tx.execute_batch(
                "CREATE TABLE tracks (
                    id INTEGER PRIMARY KEY,
                    path TEXT NOT NULL UNIQUE,
                    size INTEGER NOT NULL,
                    mtime INTEGER NOT NULL,
                    title TEXT NOT NULL,
                    artist TEXT,
                    album TEXT,
                    duration REAL,
                    bpm REAL,
                    first_beat REAL,
                    analysis TEXT NOT NULL DEFAULT 'pending',
                    error TEXT,
                    added_at INTEGER NOT NULL DEFAULT (unixepoch())
                );
                CREATE INDEX tracks_analysis ON tracks (analysis);
                CREATE TABLE roots (path TEXT PRIMARY KEY);
                PRAGMA user_version = 1;",
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn add_root(&self, path: &Path) -> Result<(), DbError> {
        self.conn.execute(
            "INSERT OR IGNORE INTO roots (path) VALUES (?1)",
            params![path.to_string_lossy()],
        )?;
        Ok(())
    }

    pub fn roots(&self) -> Result<Vec<PathBuf>, DbError> {
        let mut stmt = self.conn.prepare("SELECT path FROM roots ORDER BY path")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(PathBuf::from)
            .collect())
    }

    /// Add a track or refresh it. A changed file (size or mtime) gets its analysis redone; an
    /// unchanged one keeps it. Returns the id and whether analysis is (now) pending.
    pub fn upsert(
        &self,
        path: &Path,
        stamp: FileStamp,
        info: &TrackInfo,
    ) -> Result<(i64, bool), DbError> {
        let key = path.to_string_lossy();
        let existing: Option<(i64, i64, i64, String)> = self
            .conn
            .query_row(
                "SELECT id, size, mtime, analysis FROM tracks WHERE path = ?1",
                params![key],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;
        match existing {
            // A failed analysis gets another try on import: the cause may have been passing
            // (drive unplugged, permissions); a really broken file fails again quickly.
            Some((id, size, mtime, analysis))
                if size == stamp.size as i64
                    && mtime == stamp.mtime
                    && AnalysisState::parse(&analysis) == AnalysisState::Failed =>
            {
                self.conn.execute(
                    "UPDATE tracks SET analysis = 'pending', error = NULL WHERE id = ?1",
                    params![id],
                )?;
                Ok((id, true))
            }
            Some((id, size, mtime, analysis))
                if size == stamp.size as i64 && mtime == stamp.mtime =>
            {
                Ok((
                    id,
                    AnalysisState::parse(&analysis) == AnalysisState::Pending,
                ))
            }
            Some((id, ..)) => {
                self.conn.execute(
                    "UPDATE tracks SET size = ?2, mtime = ?3, title = ?4, artist = ?5, album = ?6,
                         duration = ?7, bpm = NULL, first_beat = NULL, analysis = 'pending',
                         error = NULL
                     WHERE id = ?1",
                    params![
                        id,
                        stamp.size as i64,
                        stamp.mtime,
                        info.title,
                        info.artist,
                        info.album,
                        info.duration_secs
                    ],
                )?;
                Ok((id, true))
            }
            None => {
                self.conn.execute(
                    "INSERT INTO tracks (path, size, mtime, title, artist, album, duration)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        key,
                        stamp.size as i64,
                        stamp.mtime,
                        info.title,
                        info.artist,
                        info.album,
                        info.duration_secs
                    ],
                )?;
                Ok((self.conn.last_insert_rowid(), true))
            }
        }
    }

    pub fn tracks(&self) -> Result<Vec<TrackRow>, DbError> {
        let mut stmt = self
            .conn
            .prepare(&format!("SELECT {COLUMNS} FROM tracks ORDER BY id"))?;
        let rows = stmt.query_map([], row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn track(&self, id: i64) -> Result<Option<TrackRow>, DbError> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM tracks WHERE id = ?1"),
                params![id],
                row,
            )
            .optional()?)
    }

    pub fn track_by_path(&self, path: &Path) -> Result<Option<TrackRow>, DbError> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM tracks WHERE path = ?1"),
                params![path.to_string_lossy()],
                row,
            )
            .optional()?)
    }

    /// Tracks still to analyse, oldest first.
    pub fn pending(&self) -> Result<Vec<(i64, PathBuf)>, DbError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, path FROM tracks WHERE analysis = 'pending' ORDER BY id")?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, PathBuf::from(r.get::<_, String>(1)?)))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn set_analyzed(
        &self,
        id: i64,
        grid: Option<BeatGrid>,
        duration: f64,
    ) -> Result<(), DbError> {
        self.conn.execute(
            "UPDATE tracks SET bpm = ?2, first_beat = ?3, duration = ?4, analysis = ?5,
                 error = NULL
             WHERE id = ?1",
            params![
                id,
                grid.map(|g| g.bpm),
                grid.map(|g| g.first_beat),
                duration,
                AnalysisState::Done.as_str()
            ],
        )?;
        Ok(())
    }

    pub fn set_failed(&self, id: i64, error: &str) -> Result<(), DbError> {
        self.conn.execute(
            "UPDATE tracks SET analysis = ?2, error = ?3 WHERE id = ?1",
            params![id, AnalysisState::Failed.as_str(), error],
        )?;
        Ok(())
    }
}

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<TrackRow> {
    let bpm: Option<f64> = r.get(6)?;
    let first_beat: Option<f64> = r.get(7)?;
    Ok(TrackRow {
        id: r.get(0)?,
        path: PathBuf::from(r.get::<_, String>(1)?),
        title: r.get(2)?,
        artist: r.get(3)?,
        album: r.get(4)?,
        duration: r.get(5)?,
        grid: bpm
            .zip(first_beat)
            .map(|(bpm, first_beat)| BeatGrid { bpm, first_beat }),
        analysis: AnalysisState::parse(&r.get::<_, String>(8)?),
        error: r.get(9)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(title: &str) -> TrackInfo {
        TrackInfo {
            title: title.to_string(),
            artist: Some("Artist".into()),
            album: None,
            duration_secs: Some(200.0),
        }
    }

    const STAMP: FileStamp = FileStamp {
        size: 1000,
        mtime: 1_700_000_000,
    };

    #[test]
    fn fresh_database_has_the_current_schema() {
        let lib = Library::open_in_memory().unwrap();
        assert_eq!(lib.schema_version().unwrap(), SCHEMA_VERSION);
        assert!(lib.tracks().unwrap().is_empty());
    }

    #[test]
    fn reopening_keeps_data_and_schema() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("library.db");
        {
            let lib = Library::open(&path).unwrap();
            lib.upsert(Path::new("/m/a.mp3"), STAMP, &info("A"))
                .unwrap();
            lib.add_root(Path::new("/m")).unwrap();
        }
        let lib = Library::open(&path).unwrap();
        assert_eq!(lib.schema_version().unwrap(), SCHEMA_VERSION);
        assert_eq!(lib.tracks().unwrap().len(), 1);
        assert_eq!(lib.roots().unwrap(), [PathBuf::from("/m")]);
    }

    #[test]
    fn a_database_from_a_newer_version_is_refused_untouched() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("library.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.pragma_update(None, "user_version", 99).unwrap();
        }
        assert!(matches!(Library::open(&path), Err(DbError::TooNew(99))));
        let conn = Connection::open(&path).unwrap();
        let v: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v, 99, "not migrated down or overwritten");
    }

    #[test]
    fn upsert_keeps_analysis_of_unchanged_files_and_redoes_changed_ones() {
        let lib = Library::open_in_memory().unwrap();
        let (id, pending) = lib
            .upsert(Path::new("/m/a.mp3"), STAMP, &info("A"))
            .unwrap();
        assert!(pending);
        let grid = BeatGrid {
            bpm: 128.0,
            first_beat: 0.1,
        };
        lib.set_analyzed(id, Some(grid), 201.5).unwrap();

        // Import again, file unchanged: analysis stays.
        let (same, pending) = lib
            .upsert(Path::new("/m/a.mp3"), STAMP, &info("A"))
            .unwrap();
        assert_eq!((same, pending), (id, false));
        let t = lib.track(id).unwrap().unwrap();
        assert_eq!(
            (t.grid, t.analysis, t.duration),
            (Some(grid), AnalysisState::Done, Some(201.5))
        );

        // File edited (new mtime): analysis again, tags refreshed.
        let edited = FileStamp {
            mtime: STAMP.mtime + 60,
            ..STAMP
        };
        let (same, pending) = lib
            .upsert(Path::new("/m/a.mp3"), edited, &info("A2"))
            .unwrap();
        assert_eq!((same, pending), (id, true));
        let t = lib.track(id).unwrap().unwrap();
        assert_eq!(
            (t.title.as_str(), t.grid, t.analysis),
            ("A2", None, AnalysisState::Pending)
        );
        assert_eq!(lib.pending().unwrap(), [(id, PathBuf::from("/m/a.mp3"))]);
        assert_eq!(
            lib.track_by_path(Path::new("/m/a.mp3"))
                .unwrap()
                .map(|t| t.id),
            Some(id)
        );
        assert!(lib.track_by_path(Path::new("/m/b.mp3")).unwrap().is_none());
    }

    #[test]
    fn failed_analysis_is_kept_with_its_message() {
        let lib = Library::open_in_memory().unwrap();
        let (id, _) = lib
            .upsert(Path::new("/m/broken.mp3"), STAMP, &info("B"))
            .unwrap();
        lib.set_failed(id, "Datei ist beschädigt").unwrap();
        let t = lib.track(id).unwrap().unwrap();
        assert_eq!(t.analysis, AnalysisState::Failed);
        assert_eq!(t.error.as_deref(), Some("Datei ist beschädigt"));
        assert!(lib.pending().unwrap().is_empty());

        // Importing again, unchanged, gives it another try.
        let (same, pending) = lib
            .upsert(Path::new("/m/broken.mp3"), STAMP, &info("B"))
            .unwrap();
        assert_eq!((same, pending), (id, true));
        let t = lib.track(id).unwrap().unwrap();
        assert_eq!((t.analysis, t.error), (AnalysisState::Pending, None));
    }

    #[test]
    fn a_track_without_tempo_is_done_with_no_grid() {
        let lib = Library::open_in_memory().unwrap();
        let (id, _) = lib
            .upsert(Path::new("/m/ambient.flac"), STAMP, &info("C"))
            .unwrap();
        lib.set_analyzed(id, None, 300.0).unwrap();
        let t = lib.track(id).unwrap().unwrap();
        assert_eq!((t.analysis, t.grid), (AnalysisState::Done, None));
    }

    #[test]
    fn unusual_paths_round_trip() {
        let lib = Library::open_in_memory().unwrap();
        let path = Path::new("/m/Ünïcödé 'quote' \"dq\" ; DROP TABLE tracks;.mp3");
        let (id, _) = lib.upsert(path, STAMP, &info("D")).unwrap();
        assert_eq!(lib.track(id).unwrap().unwrap().path, path);
    }
}
