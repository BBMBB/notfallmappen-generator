Erste Alpha-Version des Notfallmappen-Generators.

Die Anwendung erfasst persönliche und medizinische Daten, Notfallkontakte, Konten, Versicherungen und laufende Verträge in einer lokalen SQLite-Datenbank. Der A4-PDF-Export enthält ein Deckblatt, ein verlinktes Inhaltsverzeichnis und einen eigenen Seitenbeginn für jeden Bereich.

**Ausprobieren:** Archiv entpacken und im Terminal `notfallmappe --demo` bzw. unter PowerShell `.\notfallmappe.exe --demo` starten. Die Demo verändert keine gespeicherten Daten.

- Linux: x86_64, glibc ab 2.35 (gebaut auf Ubuntu 22.04).
- Windows: x86_64, MSVC-Build.
- SQLite und Schriften sind eingebunden. Keine LaTeX-Installation nötig.
- `SHA256SUMS.txt` enthält die Prüfsummen beider Archive.

Dies ist eine Vorabversion. Datenbank und exportierte PDFs sind unverschlüsselt. Vor einem Update die Datenbank sichern. Rechtliche Vorsorge ist noch nicht enthalten.

Das Projekt wurde mit Unterstützung von KI erstellt. Anwendungscode: MIT; eingebettete Schriften: SIL Open Font License.
