use crate::{db::Db, model::Entry};
use anyhow::Result;

/// An independent, editable session. Never opens or changes the user's database.
pub fn database() -> Result<Db> {
    let db = Db::memory()?;
    let groups: &[&[&[&str]]] = &[
        &[&[
            "Erika Musterfrau",
            "12.03.1980",
            "Musterstraße 12, 12345 Beispielstadt",
            "030 000000",
        ]],
        &[
            &[
                "Max Muster",
                "Partner",
                "030 000001",
                "max@muster.example",
                "Musterstraße 12, 12345 Beispielstadt",
            ],
            &[
                "Dr. Anna Beispiel",
                "Hausärztin",
                "030 000002",
                "praxis@hausarzt.example",
                "Praxisweg 5, 12345 Beispielstadt",
            ],
            &[
                "Dr. Paul Demo",
                "Kardiologe",
                "030 000003",
                "praxis@kardiologie.example",
                "Ärzteweg 7, 12345 Beispielstadt",
            ],
        ],
        &[&[
            "A positiv",
            "Penicillin\nHausstaub\nBirkenpollen",
            "Asthma\nMigräne\nBluthochdruck",
        ]],
        &[
            &[
                "Demomedikament A",
                "10 mg",
                "morgens laut hinterlegtem Plan",
                "Fiktives Beispiel, keine Einnahmeempfehlung",
            ],
            &[
                "Demomedikament B",
                "20 mg",
                "abends laut hinterlegtem Plan",
                "Fiktives Beispiel, keine Einnahmeempfehlung",
            ],
            &[
                "Demomedikament C",
                "5 mg",
                "laut hinterlegtem Bedarfsplan",
                "Fiktives Beispiel, keine Einnahmeempfehlung",
            ],
        ],
        &[
            &[
                "Musterbank",
                "Girokonto",
                "DEMO-KONTO-001",
                "Erika Musterfrau",
                "Max Muster / Ordner Finanzen",
            ],
            &[
                "Beispielbank",
                "Tagesgeldkonto",
                "DEMO-KONTO-002",
                "Erika Musterfrau",
                "Max Muster / Ordner Finanzen",
            ],
            &[
                "Demobank",
                "Depot",
                "DEMO-DEPOT-003",
                "Erika Musterfrau",
                "Max Muster / Ordner Finanzen",
            ],
        ],
        &[
            &[
                "Muster-Versicherung",
                "Haftpflicht",
                "DEMO-V-001",
                "Ordner Versicherungen, Register 1",
            ],
            &[
                "Beispiel-Krankenkasse",
                "Krankenversicherung",
                "DEMO-V-002",
                "Ordner Versicherungen, Register 2",
            ],
            &[
                "Demo-Versicherung",
                "Berufsunfähigkeit",
                "DEMO-V-003",
                "Ordner Versicherungen, Register 3",
            ],
        ],
        &[
            &[
                "Beispiel-Energie",
                "Strom",
                "DEMO-VERTRAG-001",
                "Ordner Verträge, Register 1",
            ],
            &[
                "Muster-Vermietung",
                "Miete",
                "DEMO-VERTRAG-002",
                "Ordner Verträge, Register 2",
            ],
            &[
                "Netflix",
                "Streaming-Abo",
                "DEMO-ABO-003",
                "Ordner Verträge, Register 3",
            ],
        ],
    ];
    for (section, rows) in groups.iter().enumerate() {
        for row in *rows {
            db.save(&Entry {
                id: 0,
                section,
                values: row.iter().map(|value| value.to_string()).collect(),
                updated: String::new(),
            })?;
        }
    }
    Ok(db)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{SECTIONS, is_bullet_field};
    #[test]
    fn demo_counts_validity_and_isolation() -> Result<()> {
        let db = database()?;
        let entries = db.all()?;
        for (section, config) in SECTIONS.iter().enumerate() {
            let rows: Vec<_> = entries
                .iter()
                .filter(|entry| entry.section == section)
                .collect();
            assert_eq!(
                rows.len(),
                if config.single { 1 } else { 3 },
                "{}",
                config.title
            );
            for entry in rows {
                assert_eq!(entry.values.len(), config.fields.len());
                crate::model::validate(entry)?;
                for (field, value) in entry.values.iter().enumerate() {
                    assert!(!value.is_empty());
                    assert_eq!(
                        value.lines().count(),
                        if is_bullet_field(section, field) {
                            3
                        } else {
                            1
                        }
                    );
                }
            }
        }
        db.delete(entries[0].id)?;
        assert_eq!(db.all()?.len(), 16);
        assert_eq!(database()?.all()?.len(), 17);
        Ok(())
    }
}
