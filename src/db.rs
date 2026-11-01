use crate::model::{Entry, SECTIONS};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, params};
use std::path::Path;
pub struct Db(Connection);
impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .open(path)?;
        }
        Self::initialize(Connection::open(path).context("Datenbank konnte nicht geöffnet werden")?)
    }
    pub fn memory() -> Result<Self> {
        Self::initialize(Connection::open_in_memory()?)
    }
    fn initialize(mut conn: Connection) -> Result<Self> {
        conn.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE IF NOT EXISTS entries (id INTEGER PRIMARY KEY, section INTEGER NOT NULL, data TEXT NOT NULL, updated TEXT NOT NULL);")?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        ensure!(
            version <= 2,
            "Diese Datenbank stammt aus einer neueren Anwendungsversion."
        );
        if version < 2 {
            let tx = conn.transaction()?;
            let old = {
                let mut stmt = tx.prepare("SELECT id, data FROM entries WHERE section=1")?;
                stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
                    .collect::<std::result::Result<Vec<_>, _>>()?
            };
            for (id, data) in old {
                let mut values: Vec<String> = serde_json::from_str(&data)?;
                if values.len() == 4 {
                    // Old combined field: classify only obvious e-mails; otherwise keep as address.
                    let combined = std::mem::take(&mut values[3]);
                    if combined.contains('@') {
                        values[3] = combined;
                        values.push(String::new());
                    } else {
                        values.push(combined);
                    }
                    tx.execute(
                        "UPDATE entries SET data=?1 WHERE id=?2",
                        params![serde_json::to_string(&values)?, id],
                    )?;
                }
            }
            tx.pragma_update(None, "user_version", 2)?;
            tx.commit()?;
        }
        Ok(Self(conn))
    }
    pub fn all(&self) -> Result<Vec<Entry>> {
        let mut stmt = self
            .0
            .prepare("SELECT id, section, data, updated FROM entries ORDER BY section, id")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, usize>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        let mut entries = Vec::new();
        for row in rows {
            let (id, section, data, updated) = row?;
            ensure!(section < SECTIONS.len(), "Unbekannter Datenbereich");
            entries.push(Entry {
                id,
                section,
                values: serde_json::from_str(&data)?,
                updated,
            });
        }
        Ok(entries)
    }
    pub fn save(&self, entry: &Entry) -> Result<()> {
        ensure!(entry.section < SECTIONS.len(), "Ungültiger Bereich");
        let section = &SECTIONS[entry.section];
        ensure!(
            entry.values.len() == section.fields.len(),
            "Ungültiges Formular"
        );
        ensure!(
            entry.values.iter().any(|v| !v.trim().is_empty()),
            "Bitte mindestens ein Feld ausfüllen."
        );
        crate::model::validate(entry)?;
        let data = serde_json::to_string(&entry.values)?;
        let now = chrono::Local::now().format("%d.%m.%Y %H:%M").to_string();
        if entry.id == 0 {
            if section.single {
                ensure!(
                    !self.all()?.iter().any(|e| e.section == entry.section),
                    "Dieser Bereich hat bereits einen Eintrag."
                );
            }
            self.0.execute(
                "INSERT INTO entries(section,data,updated) VALUES(?1,?2,?3)",
                params![entry.section, data, now],
            )?;
        } else {
            ensure!(
                self.0.execute(
                    "UPDATE entries SET data=?1,updated=?2 WHERE id=?3 AND section=?4",
                    params![data, now, entry.id, entry.section]
                )? == 1,
                "Eintrag nicht gefunden"
            );
        }
        Ok(())
    }
    pub fn delete(&self, id: i64) -> Result<()> {
        self.0.execute("DELETE FROM entries WHERE id=?1", [id])?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persistence_and_singleton() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("test.db");
        let db = Db::open(&path)?;
        let mut e = Entry {
            id: 0,
            section: 0,
            values: vec!["Änne Müller".into(), "".into(), "".into(), "".into()],
            updated: String::new(),
        };
        db.save(&e)?;
        assert!(db.save(&e).is_err());
        e = db.all()?.remove(0);
        e.values[0] = "Neu".into();
        db.save(&e)?;
        drop(db);
        let db = Db::open(&path)?;
        assert_eq!(db.all()?[0].values[0], "Neu");
        db.delete(e.id)?;
        assert!(db.all()?.is_empty());
        Ok(())
    }
}

#[cfg(test)]
mod migration_tests {
    use super::*;
    #[test]
    fn migrate_combined_contact_without_losing_data() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("old.db");
        let conn = Connection::open(&path)?;
        conn.execute_batch("CREATE TABLE entries(id INTEGER PRIMARY KEY, section INTEGER NOT NULL, data TEXT NOT NULL, updated TEXT NOT NULL); PRAGMA user_version=1;")?;
        for value in ["name@beispiel.de", "Musterstraße 3"] {
            let data = serde_json::to_string(&vec!["Name", "Familie", "123", value])?;
            conn.execute(
                "INSERT INTO entries(section,data,updated) VALUES(1,?1,'alt')",
                [data],
            )?;
        }
        drop(conn);
        let db = Db::open(&path)?;
        let rows = db.all()?;
        assert_eq!(rows[0].values[3], "name@beispiel.de");
        assert_eq!(rows[0].values[4], "");
        assert_eq!(rows[1].values[3], "");
        assert_eq!(rows[1].values[4], "Musterstraße 3");
        drop(db);
        assert_eq!(Db::open(&path)?.all()?[1].values.len(), 5);
        Ok(())
    }
}
