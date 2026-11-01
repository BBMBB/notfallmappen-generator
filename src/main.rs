mod db;
mod demo;
mod model;
mod pdf;
mod ui;
use anyhow::{Context, Result, bail};
use std::path::PathBuf;
fn main() -> Result<()> {
    let mut database = std::env::current_exe()?
        .parent()
        .context("Anwendungsordner konnte nicht ermittelt werden")?
        .join("notfallmappe.sqlite3");
    let mut export = None;
    let mut demo = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--db" => database = PathBuf::from(args.next().context("--db benötigt einen Pfad")?),
            "--export" => {
                export = Some(PathBuf::from(
                    args.next().context("--export benötigt einen PDF-Pfad")?,
                ))
            }
            "--demo" => demo = true,
            "--demo-pdf" => {
                demo = true;
                export = Some(PathBuf::from(
                    args.next().context("--demo-pdf benötigt einen PDF-Pfad")?,
                ));
            }
            "--help" | "-h" => {
                println!(
                    "Notfallmappe\n\ncargo run -- [--db DATEI] [--export DATEI.pdf]\n  --demo                TUI mit fiktiven Beispieldaten (nur im Arbeitsspeicher)\n  --demo-pdf DATEI.pdf   PDF mit fiktiven Beispieldaten, ohne Datenbank\n\nOhne --export startet die TUI. Hilfe: F1."
                );
                return Ok(());
            }
            _ => bail!("Unbekannte Option: {arg}"),
        }
    }
    let db = if demo {
        database = PathBuf::from(":memory:");
        demo::database()?
    } else {
        db::Db::open(&database)?
    };
    if let Some(path) = export {
        pdf::export(&db.all()?, &path)?;
        println!("PDF gespeichert: {}", path.display());
        Ok(())
    } else {
        ui::run(db, database)
    }
}
