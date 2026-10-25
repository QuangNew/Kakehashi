//! Copy referenced legacy files before updating their database paths. Originals stay intact.
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

fn digest(path: &Path) -> Result<Vec<u8>, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65_536];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(hash.finalize().to_vec())
}

fn legacy_source(path: &str, legacy: &Path, kind: &str) -> Option<PathBuf> {
    let directory = legacy.join(kind).canonicalize().ok()?;
    let source = Path::new(path).canonicalize().ok()?;
    (source.is_file() && source.starts_with(directory)).then_some(source)
}

fn copy_verified(source: &Path, destination_dir: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(destination_dir).map_err(|e| e.to_string())?;
    let hash = digest(source)?;
    let name = source.file_name().ok_or("Missing legacy filename")?;
    let mut destination = destination_dir.join(name);
    if destination.exists() {
        if digest(&destination)? == hash {
            return Ok(destination);
        }
        let suffix: String = hash.iter().map(|b| format!("{b:02x}")).collect();
        destination = destination_dir.join(format!("{suffix}-{}", name.to_string_lossy()));
        if destination.exists() {
            return if digest(&destination)? == hash {
                Ok(destination)
            } else {
                Err("Migration destination contains different data".into())
            };
        }
    }
    let result = (|| {
        let mut input = File::open(source).map_err(|e| e.to_string())?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
            .map_err(|e| e.to_string())?;
        io::copy(&mut input, &mut output).map_err(|e| e.to_string())?;
        output.sync_all().map_err(|e| e.to_string())?;
        if digest(&destination)? != hash {
            return Err("Copied file checksum differs".into());
        }
        Ok(destination.clone())
    })();
    // A failed copy remains unreferenced. A future launch can retry without overwriting it.
    result
}

pub fn migrate_legacy_files(
    conn: &Connection,
    app_data: &Path,
    legacy: &Path,
) -> Result<usize, String> {
    let mut changes: Vec<(&str, &str, String, PathBuf, &str)> = Vec::new();
    for (table, column, kind) in [
        ("context_resources", "file_path", "context"),
        ("meetings", "recording_path", "recordings"),
        ("meetings", "waveform_path", "recordings"),
    ] {
        let mut statement = conn
            .prepare(&format!(
                "SELECT id, {column} FROM {table} WHERE {column} IS NOT NULL"
            ))
            .map_err(|e| e.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (id, path) = row.map_err(|e| e.to_string())?;
            if let Some(source) = legacy_source(&path, legacy, kind) {
                changes.push((table, column, id, source, kind));
            }
        }
    }
    if changes.is_empty() {
        return Ok(0);
    }
    let backups = app_data.join("backups");
    fs::create_dir_all(&backups).map_err(|e| e.to_string())?;
    let backup = backups.join(format!("before-path-migration-{}.db", uuid::Uuid::new_v4()));
    conn.execute("VACUUM INTO ?1", params![backup.to_string_lossy().as_ref()])
        .map_err(|e| format!("Could not back up database: {e}"))?;
    let mut copied = 0;
    for (table, column, id, source, kind) in changes {
        match copy_verified(&source, &app_data.join(kind)) {
            Ok(destination) => {
                conn.execute(
                    &format!("UPDATE {table} SET {column}=?1 WHERE id=?2"),
                    params![destination.to_string_lossy().as_ref(), id],
                )
                .map_err(|e| e.to_string())?;
                copied += 1;
            }
            Err(error) => log::warn!("Keeping legacy {kind} reference for {id}: {error}"),
        }
    }
    Ok(copied)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn copies_referenced_files_without_overwriting_or_changing_originals() {
        let root =
            std::env::temp_dir().join(format!("kakehashi-migration-{}", uuid::Uuid::new_v4()));
        let legacy = root.join("legacy");
        let current = root.join("current");
        fs::create_dir_all(legacy.join("context")).unwrap();
        fs::create_dir_all(legacy.join("recordings")).unwrap();
        fs::create_dir_all(current.join("context")).unwrap();
        let old = legacy.join("context/doc.txt");
        fs::write(&old, "日本語 tài liệu").unwrap();
        fs::write(
            current.join("context/doc.txt"),
            "Existing different document",
        )
        .unwrap();
        let recording = legacy.join("recordings/a.ogg");
        let waveform = legacy.join("recordings/a.json");
        fs::write(&recording, b"recording bytes").unwrap();
        fs::write(&waveform, b"{}").unwrap();
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE context_resources(id TEXT,file_path TEXT); CREATE TABLE meetings(id TEXT,recording_path TEXT,waveform_path TEXT);").unwrap();
        conn.execute(
            "INSERT INTO context_resources VALUES ('doc',?1)",
            params![old.to_string_lossy().as_ref()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO meetings VALUES ('m',?1,?2)",
            params![
                recording.to_string_lossy().as_ref(),
                waveform.to_string_lossy().as_ref()
            ],
        )
        .unwrap();
        assert_eq!(migrate_legacy_files(&conn, &current, &legacy).unwrap(), 3);
        let migrated: String = conn
            .query_row("SELECT file_path FROM context_resources", [], |r| r.get(0))
            .unwrap();
        assert!(Path::new(&migrated).starts_with(current.join("context")));
        assert_eq!(fs::read_to_string(migrated).unwrap(), "日本語 tài liệu");
        assert_eq!(
            fs::read_to_string(current.join("context/doc.txt")).unwrap(),
            "Existing different document"
        );
        assert!(old.is_file() && recording.is_file() && waveform.is_file());
        assert_eq!(migrate_legacy_files(&conn, &current, &legacy).unwrap(), 0);
        assert_eq!(fs::read_dir(current.join("backups")).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn ignores_missing_files_and_paths_outside_legacy_storage() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE context_resources(id TEXT,file_path TEXT); CREATE TABLE meetings(id TEXT,recording_path TEXT,waveform_path TEXT); INSERT INTO context_resources VALUES ('missing','does-not-exist.txt');").unwrap();
        let root =
            std::env::temp_dir().join(format!("kakehashi-migration-{}", uuid::Uuid::new_v4()));
        assert_eq!(
            migrate_legacy_files(&conn, &root, &root.join("legacy")).unwrap(),
            0
        );
        let path: String = conn
            .query_row("SELECT file_path FROM context_resources", [], |r| r.get(0))
            .unwrap();
        assert_eq!(path, "does-not-exist.txt");
        assert!(!root.exists());
    }
}
