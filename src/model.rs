use serde::{Deserialize, Serialize};

pub struct Section {
    pub title: &'static str,
    pub fields: &'static [&'static str],
    pub single: bool,
}
pub const SECTIONS: &[Section] = &[
    Section {
        title: "Persönliche Daten",
        fields: &["Name", "Geburtsdatum", "Adresse", "Telefon"],
        single: true,
    },
    Section {
        title: "Notfallkontakte",
        fields: &["Name", "Beziehung", "Telefon", "E-Mail", "Adresse"],
        single: false,
    },
    Section {
        title: "Medizinische Basisdaten",
        fields: &[
            "Blutgruppe (falls bekannt)",
            "Allergien / Unverträglichkeiten",
            "Wichtige Erkrankungen / Hinweise",
        ],
        single: true,
    },
    Section {
        title: "Medikamentenplan",
        fields: &[
            "Medikament / Wirkstoff",
            "Stärke",
            "Dosierung / Einnahmezeit",
            "Hinweis",
        ],
        single: false,
    },
    Section {
        title: "Bankkonten & Depots",
        fields: &[
            "Bank",
            "Art (Konto / Depot)",
            "IBAN / Konto- oder Depotnummer",
            "Inhaber",
            "Bevollmächtigte / Vollmacht liegt bei",
        ],
        single: false,
    },
    Section {
        title: "Versicherungen",
        fields: &[
            "Versicherer",
            "Versicherungsart",
            "Versicherungsnummer",
            "Kontakt / Unterlagen liegen bei",
        ],
        single: false,
    },
    Section {
        title: "Laufende Verträge",
        fields: &[
            "Anbieter",
            "Art (Miete / Strom / Kredit / Abo)",
            "Vertrags- / Kundennummer (optional)",
            "Kontakt / Unterlagen liegen bei",
        ],
        single: false,
    },
];
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub id: i64,
    pub section: usize,
    pub values: Vec<String>,
    pub updated: String,
}
impl Entry {
    pub fn label(&self) -> String {
        self.values
            .iter()
            .filter(|v| !v.is_empty())
            .take(2)
            .cloned()
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

/// Medical lists are stored as newline-separated items, preserving the existing schema.
pub fn is_bullet_field(section: usize, field: usize) -> bool {
    section == 2 && matches!(field, 1 | 2)
}

pub fn validate(entry: &Entry) -> anyhow::Result<()> {
    use anyhow::ensure;
    if entry.section == 0 {
        let date = entry.values[1].trim();
        if !date.is_empty() {
            let format_ok = date.len() == 10
                && date.as_bytes()[2] == b'.'
                && date.as_bytes()[5] == b'.'
                && date
                    .chars()
                    .enumerate()
                    .all(|(i, c)| i == 2 || i == 5 || c.is_ascii_digit());
            ensure!(format_ok, "Geburtsdatum bitte als TT.MM.JJJJ eingeben.");
            let parsed = chrono::NaiveDate::parse_from_str(date, "%d.%m.%Y")
                .map_err(|_| anyhow::anyhow!("Geburtsdatum ist kein gültiges Kalenderdatum."))?;
            ensure!(
                parsed <= chrono::Local::now().date_naive(),
                "Geburtsdatum darf nicht in der Zukunft liegen."
            );
            ensure!(
                chrono::Datelike::year(&parsed) > 0,
                "Geburtsjahr muss größer als 0 sein."
            );
        }
    }
    if entry.section == 1 {
        let email = entry.values[3].trim();
        if !email.is_empty() {
            let parts: Vec<_> = email.split('@').collect();
            ensure!(
                parts.len() == 2 && !email.chars().any(char::is_whitespace),
                "Bitte eine gültige E-Mail-Adresse eingeben (z. B. name@beispiel.de)."
            );
            let (local, domain) = (parts[0], parts[1]);
            ensure!(
                !local.is_empty()
                    && !local.starts_with('.')
                    && !local.ends_with('.')
                    && !local.contains("..")
                    && local
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || ".!#$%&'*+-/=?^_`{|}~".contains(c))
                    && domain.contains('.')
                    && domain.split('.').all(|part| !part.is_empty()
                        && !part.starts_with('-')
                        && !part.ends_with('-')
                        && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')),
                "Bitte eine gültige E-Mail-Adresse eingeben (z. B. name@beispiel.de)."
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn date_and_email_validation() {
        let mut person = Entry {
            id: 0,
            section: 0,
            values: vec!["Name".into(), String::new(), String::new(), String::new()],
            updated: String::new(),
        };
        for date in [
            "09.111.12",
            "31.02.2020",
            "29.02.2023",
            "1.1.1980",
            "01.01.2999",
        ] {
            person.values[1] = date.into();
            assert!(validate(&person).is_err(), "{date}");
        }
        for date in ["", "29.02.2024", "09.11.2012"] {
            person.values[1] = date.into();
            assert!(validate(&person).is_ok(), "{date}");
        }
        person.section = 1;
        person.values.push(String::new());
        for email in [
            "ohne-at",
            "@beispiel.de",
            "a@@b.de",
            "a@",
            "a @b.de",
            "a@.de",
            "a@b..de",
        ] {
            person.values[3] = email.into();
            assert!(validate(&person).is_err(), "{email}");
        }
        for email in [
            "",
            "name@beispiel.de",
            "vorname.nachname+notfall@praxis.example",
        ] {
            person.values[3] = email.into();
            assert!(validate(&person).is_ok(), "{email}");
        }
    }
}
