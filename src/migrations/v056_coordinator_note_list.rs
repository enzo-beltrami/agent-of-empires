//! Migration v056: the coordinator's notes move from one Markdown file (`NOTES.md`) to a list
//! of individual notes (`notes.json`). Existing text becomes a single note.

use anyhow::{Context, Result};
use std::{fs, io::ErrorKind, path::Path};

use crate::session::coordinator::{Note, NoteList, NOTES};

const LEGACY: &str = "NOTES.md";

pub fn run() -> Result<()> {
    run_in(&crate::session::get_app_dir()?.join("coordinator"))
}

fn run_in(dir: &Path) -> Result<()> {
    let legacy = dir.join(LEGACY);
    let text = match fs::read_to_string(&legacy) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error).with_context(|| format!("Read {}", legacy.display())),
    };
    let text = text.trim();
    if !text.is_empty() {
        let list_path = dir.join(NOTES);
        let mut list: NoteList = match fs::read(&list_path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .with_context(|| format!("Parse {}", list_path.display()))?,
            Err(error) if error.kind() == ErrorKind::NotFound => NoteList::default(),
            Err(error) => {
                return Err(error).with_context(|| format!("Read {}", list_path.display()))
            }
        };
        // A run interrupted after writing the list must not add the note twice.
        if !list.notes.iter().any(|note| note.text == text) {
            let at = fs::metadata(&legacy)
                .and_then(|m| m.modified())
                .map(chrono::DateTime::from)
                .unwrap_or_else(|_| chrono::Utc::now());
            list.notes.push(Note::new(text.to_string(), at));
            crate::session::atomic_write(&list_path, &serde_json::to_vec_pretty(&list)?)?;
        }
        tracing::info!(target: "migration", "moved the coordinator notes into notes.json");
    }
    fs::remove_file(&legacy).with_context(|| format!("Remove {}", legacy.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(dir: &Path) -> Vec<String> {
        let list: NoteList = serde_json::from_slice(&fs::read(dir.join(NOTES)).unwrap()).unwrap();
        list.notes.into_iter().map(|n| n.text).collect()
    }

    #[test]
    fn legacy_text_becomes_one_note_once() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        run_in(dir).unwrap();
        assert!(
            !dir.join(NOTES).exists(),
            "nothing to migrate leaves no list"
        );

        fs::write(dir.join(LEGACY), "- ship Friday\n- ask about tests\n").unwrap();
        run_in(dir).unwrap();
        assert_eq!(listed(dir), ["- ship Friday\n- ask about tests"]);
        assert!(!dir.join(LEGACY).exists());

        // Interrupted after the write: the file is back, the note is already listed.
        fs::write(dir.join(LEGACY), "- ship Friday\n- ask about tests\n").unwrap();
        run_in(dir).unwrap();
        assert_eq!(listed(dir).len(), 1);

        fs::write(dir.join(LEGACY), "  \n").unwrap();
        run_in(dir).unwrap();
        assert!(!dir.join(LEGACY).exists(), "an empty file is just removed");
        assert_eq!(listed(dir).len(), 1);
    }
}
