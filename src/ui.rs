use crate::{
    db::Db,
    model::{Entry, SECTIONS},
    pdf,
};
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode as K, KeyEventKind, KeyModifiers as M},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{prelude::*, widgets::*};
use std::{io, path::PathBuf};
struct Editor {
    entry: Entry,
    field: usize,
    cursor: usize,
}
enum Mode {
    Browse,
    Edit(Editor),
    Delete(i64),
    Help,
}
struct App {
    db: Db,
    path: PathBuf,
    entries: Vec<Entry>,
    section: usize,
    row: usize,
    entries_focus: bool,
    mode: Mode,
    message: String,
}
impl App {
    fn rows(&self) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|e| e.section == self.section)
            .collect()
    }
    fn refresh(&mut self) -> Result<()> {
        self.entries = self.db.all()?;
        self.row = self.row.min(self.rows().len().saturating_sub(1));
        Ok(())
    }
    fn edit(&mut self, new: bool) {
        let section = &SECTIONS[self.section];
        if new && section.single {
            return;
        }
        let existing = if new && !section.single {
            None
        } else {
            self.rows().get(self.row).map(|e| (*e).clone())
        };
        let entry = existing.unwrap_or(Entry {
            id: 0,
            section: self.section,
            values: vec![String::new(); section.fields.len()],
            updated: String::new(),
        });
        let cursor = entry.values[0].len();
        self.message = "F2 / Strg+S speichert. Esc Abbrechen.".into();
        self.mode = Mode::Edit(Editor {
            entry,
            field: 0,
            cursor,
        });
    }
    fn key(&mut self, key: event::KeyEvent) -> Result<bool> {
        if key.code == K::Char('c') && key.modifiers.contains(M::CONTROL) {
            self.mode = Mode::Browse;
            self.message = "Bearbeitung abgebrochen. Zum Beenden: Q".into();
            return Ok(false);
        }
        match &mut self.mode {
            Mode::Help => {
                if matches!(key.code, K::Esc | K::F(1) | K::Enter) {
                    self.mode = Mode::Browse;
                }
            }
            Mode::Delete(id) => {
                let id = *id;
                match key.code {
                    K::Char('j') | K::Char('J') => {
                        self.db.delete(id)?;
                        self.refresh()?;
                        self.mode = Mode::Browse;
                        self.message = "Eintrag gelöscht.".into();
                    }
                    K::Esc | K::Char('n' | 'N') => self.mode = Mode::Browse,
                    _ => {}
                }
            }
            Mode::Edit(editor) => {
                let count = editor.entry.values.len();
                if key.code == K::Esc {
                    self.mode = Mode::Browse;
                    self.message = "Änderungen verworfen.".into();
                } else if key.code == K::F(2)
                    || (key.code == K::Char('s') && key.modifiers.contains(M::CONTROL))
                {
                    let mut entry = editor.entry.clone();
                    for value in &mut entry.values {
                        *value = value.trim().to_owned();
                    }
                    match self.db.save(&entry) {
                        Ok(()) => {
                            self.refresh()?;
                            self.mode = Mode::Browse;
                            self.message = "Gespeichert.".into();
                        }
                        Err(e) => self.message = e.to_string(),
                    }
                } else if key.code == K::Enter
                    && crate::model::is_bullet_field(editor.entry.section, editor.field)
                {
                    editor.entry.values[editor.field].insert(editor.cursor, '\n');
                    editor.cursor += 1;
                } else if matches!(key.code, K::Tab | K::Down | K::Enter | K::BackTab | K::Up) {
                    editor.field = if matches!(key.code, K::BackTab | K::Up) {
                        (editor.field + count - 1) % count
                    } else {
                        (editor.field + 1) % count
                    };
                    editor.cursor = editor.entry.values[editor.field].len();
                } else {
                    let text = &mut editor.entry.values[editor.field];
                    let pos = editor.cursor;
                    match key.code {
                        K::Left => {
                            editor.cursor = text[..pos].char_indices().last().map_or(0, |(i, _)| i)
                        }
                        K::Right => {
                            editor.cursor =
                                pos + text[pos..].chars().next().map_or(0, char::len_utf8)
                        }
                        K::Home => editor.cursor = 0,
                        K::End => editor.cursor = text.len(),
                        K::Backspace if pos > 0 => {
                            let prev = text[..pos].char_indices().last().unwrap().0;
                            text.drain(prev..pos);
                            editor.cursor = prev;
                        }
                        K::Delete if pos < text.len() => {
                            let end = pos + text[pos..].chars().next().unwrap().len_utf8();
                            text.drain(pos..end);
                        }
                        K::Char(c)
                            if !key.modifiers.intersects(M::CONTROL | M::ALT)
                                && !c.is_control() =>
                        {
                            text.insert(pos, c);
                            editor.cursor += c.len_utf8();
                        }
                        _ => {}
                    }
                }
            }
            Mode::Browse => match key.code {
                K::Char('q' | 'Q') | K::Esc => return Ok(true),
                K::F(1) => self.mode = Mode::Help,
                K::Tab | K::BackTab => self.entries_focus = !self.entries_focus,
                K::Left => self.entries_focus = false,
                K::Right => self.entries_focus = true,
                K::Up if !self.entries_focus => {
                    self.section = (self.section + SECTIONS.len() - 1) % SECTIONS.len();
                    self.row = 0;
                }
                K::Down if !self.entries_focus => {
                    self.section = (self.section + 1) % SECTIONS.len();
                    self.row = 0;
                }
                K::Up => self.row = self.row.saturating_sub(1),
                K::Down => self.row = (self.row + 1).min(self.rows().len().saturating_sub(1)),
                K::Char('n' | 'N') => self.edit(true),
                K::Enter | K::Char('e' | 'E') => self.edit(false),
                K::Char('d' | 'D') => {
                    if let Some(entry) = self.rows().get(self.row) {
                        self.mode = Mode::Delete(entry.id);
                    }
                }
                K::Char('p' | 'P') => {
                    let path = std::env::current_dir()?.join(format!(
                        "notfallmappe-{}.pdf",
                        chrono::Local::now().format("%Y%m%d-%H%M%S-%f")
                    ));
                    self.message = match pdf::export(&self.entries, &path) {
                        Ok(()) => format!("PDF: {}", path.display()),
                        Err(e) => format!("Export fehlgeschlagen: {e:#}"),
                    };
                }
                _ => {}
            },
        }
        Ok(false)
    }
    fn draw(&self, f: &mut Frame) {
        let area = f.area();
        if area.width < 70 || area.height < 20 {
            f.render_widget(Paragraph::new("Bitte Terminal auf mindestens 70 × 20 Zeichen vergrößern.\nQ beendet die Anwendung."),area);
            return;
        }
        let layout = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
            Constraint::Length(2),
        ])
        .split(area);
        f.render_widget(
            Paragraph::new("NOTFALLMAPPE")
                .style(Style::default().fg(Color::Cyan).bold())
                .block(Block::bordered()),
            layout[0],
        );
        let cols =
            Layout::horizontal([Constraint::Length(30), Constraint::Min(20)]).split(layout[1]);
        let sections = SECTIONS
            .iter()
            .enumerate()
            .map(|(i, s)| {
                ListItem::new(format!(
                    "{}  {}",
                    if i == self.section { "›" } else { " " },
                    s.title
                ))
                .style(if i == self.section {
                    Style::default().fg(Color::Cyan).bold()
                } else {
                    Style::default()
                })
            })
            .collect::<Vec<_>>();
        f.render_widget(
            List::new(sections).block(Block::bordered().title("Bereiche · ↑ ↓").border_style(
                Style::default().fg(if self.entries_focus {
                    Color::DarkGray
                } else {
                    Color::Cyan
                }),
            )),
            cols[0],
        );
        let right = Layout::vertical([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(cols[1]);
        let rows = self.rows();
        let items = if rows.is_empty() {
            vec![ListItem::new(if SECTIONS[self.section].single {
                "Noch keine Angaben. Enter zum Bearbeiten."
            } else {
                "Noch keine Angaben. Mit N anlegen."
            })]
        } else {
            rows.iter().map(|e| ListItem::new(e.label())).collect()
        };
        let mut state = ListState::default().with_selected(if rows.is_empty() {
            None
        } else {
            Some(self.row)
        });
        f.render_stateful_widget(
            List::new(items)
                .block(
                    Block::bordered()
                        .title(SECTIONS[self.section].title)
                        .border_style(Style::default().fg(if self.entries_focus {
                            Color::Cyan
                        } else {
                            Color::DarkGray
                        })),
                )
                .highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
                .highlight_symbol("› "),
            right[0],
            &mut state,
        );
        let details = rows
            .get(self.row)
            .map(|e| {
                SECTIONS[self.section]
                    .fields
                    .iter()
                    .zip(&e.values)
                    .enumerate()
                    .map(|(i, (label, value))| {
                        if crate::model::is_bullet_field(self.section, i) && !value.is_empty() {
                            format!(
                                "{label}:\n{}",
                                value
                                    .lines()
                                    .filter(|v| !v.trim().is_empty())
                                    .map(|v| format!("  • {v}"))
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            )
                        } else {
                            format!("{label}: {}", if value.is_empty() { "–" } else { value })
                        }
                    })
                    .chain(std::iter::once(format!("\nStand: {}", e.updated)))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_else(|| "Noch keine Angaben hinterlegt.".into());
        f.render_widget(
            Paragraph::new(details)
                .wrap(Wrap { trim: false })
                .block(Block::bordered().title("Details")),
            right[1],
        );
        f.render_widget(
            Paragraph::new(self.message.as_str())
                .wrap(Wrap { trim: false })
                .style(Style::default().fg(Color::Yellow)),
            layout[2],
        );
        f.render_widget(
            Paragraph::new(match &self.mode {
                Mode::Edit(_) => "Tab Feld wechseln  F2 / Strg+S Speichern  Esc Abbrechen",
                Mode::Delete(_) => "J Löschen  N / Esc Abbrechen",
                Mode::Help => "Esc Abbrechen",
                Mode::Browse if SECTIONS[self.section].single => "Enter Bearbeiten  D Löschen  P PDF exportieren\nTab Fokus wechseln  F1 Hilfe  Q Ende",
                Mode::Browse => "N Neu  Enter Bearbeiten  D Löschen  P PDF exportieren\nTab Fokus wechseln  F1 Hilfe  Q Ende",
            }),
            layout[3],
        );
        match &self.mode {
            Mode::Browse => {}
            Mode::Help => popup(
                f,
                "Hilfe",
                &format!(
                    "↑ / ↓: Bereich wählen (links) oder Eintrag (rechts)\nTab / ← / →: Fokus wechseln\nN: Neuer Eintrag (nur Listen) · Enter: Bearbeiten\nFormular: Tab/↑/↓ wechselt das Feld\n←/→, Pos1/Ende, Entf: Text bearbeiten\nAllergien/Erkrankungen: Enter für neuen Stichpunkt\nF2 oder Strg+S: Speichern · Esc: Abbrechen\nD: Löschen mit Rückfrage · P: PDF exportieren\nPDF-Ziel: aktuelles Arbeitsverzeichnis\n\nDatenbank: {}\nDatenbank und PDFs sind unverschlüsselt.\nSicherung: Anwendung schließen, Datenbank kopieren.\n\nEsc schließt diese Hilfe.",
                    if self.path == std::path::Path::new(":memory:") {
                        "Demomodus: nur im Arbeitsspeicher. Änderungen werden beim Beenden verworfen.".into()
                    } else {
                        self.path.display().to_string()
                    }
                ),
            ),
            Mode::Delete(_) => popup(
                f,
                "Eintrag löschen?",
                "Diesen Eintrag endgültig löschen?\n\nJ: Löschen   N / Esc: Abbrechen",
            ),
            Mode::Edit(editor) => {
                let rect = center(area, 90, 85);
                f.render_widget(Clear, rect);
                let items = SECTIONS[editor.entry.section]
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(i, label)| {
                        let mut value = editor.entry.values[i].clone();
                        if i == editor.field {
                            value.insert(editor.cursor, '│');
                        }
                        let bullets = crate::model::is_bullet_field(editor.entry.section, i);
                        let mut lines = vec![Line::from(Span::styled(
                            if bullets {
                                format!("{label} · Enter: weiterer Stichpunkt")
                            } else {
                                label.to_string()
                            },
                            Style::default().bold(),
                        ))];
                        let width = rect.width.saturating_sub(7) as usize;
                        for line in value.split('\n') {
                            let chars: Vec<_> = line.chars().collect();
                            let cursor = chars.iter().position(|c| *c == '│').unwrap_or(0);
                            let start = cursor.saturating_sub(width.saturating_sub(1));
                            let shown: String = chars.into_iter().skip(start).take(width).collect();
                            lines.push(Line::from(format!(
                                "{}{}",
                                if bullets { "• " } else { "" },
                                if shown.is_empty() { "(leer)" } else { &shown }
                            )));
                        }
                        lines.push(Line::from(""));
                        ListItem::new(lines)
                    })
                    .collect::<Vec<_>>();
                let mut state = ListState::default().with_selected(Some(editor.field));
                f.render_stateful_widget(
                    List::new(items)
                        .block(
                            Block::bordered()
                                .title(format!(
                                    "{} · F2 Speichern · Esc Abbrechen · Tab Feld",
                                    SECTIONS[editor.entry.section].title
                                ))
                                .title_bottom(self.message.as_str()),
                        )
                        .highlight_style(Style::default().bg(Color::DarkGray))
                        .highlight_symbol("› "),
                    rect,
                    &mut state,
                );
            }
        }
    }
}
fn center(area: Rect, width: u16, height: u16) -> Rect {
    let w = area.width * width / 100;
    let h = area.height * height / 100;
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}
fn popup(f: &mut Frame, title: &str, text: &str) {
    let rect = center(f.area(), 90, 85);
    f.render_widget(Clear, rect);
    f.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .block(Block::bordered().title(title)),
        rect,
    );
}
struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}
pub fn run(db: Db, path: PathBuf) -> Result<()> {
    let entries = db.all()?;
    let demo = path == std::path::Path::new(":memory:");
    enable_raw_mode()?;
    let _restore = Restore;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut app = App {
        db,
        path,
        entries,
        section: 0,
        row: 0,
        entries_focus: false,
        mode: Mode::Browse,
        message: if demo {
            "DEMO: Fiktive Beispiele. Änderungen bleiben nur bis zum Beenden erhalten."
        } else {
            "Alle Daten werden lokal gespeichert. F1 zeigt Hilfe und Speicherort."
        }
        .into(),
    };
    loop {
        terminal.draw(|f| app.draw(f))?;
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Release {
                continue;
            }
            match app.key(key) {
                Ok(true) => break,
                Ok(false) => {}
                Err(e) => app.message = format!("Fehler: {e:#}"),
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edit_unicode_save_cancel_and_delete() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("mappe.db");
        let mut app = App {
            db: Db::open(&path)?,
            path,
            entries: vec![],
            section: 0,
            row: 0,
            entries_focus: false,
            mode: Mode::Browse,
            message: String::new(),
        };
        let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(100, 30))?;
        terminal.draw(|frame| app.draw(frame))?;
        let header: String = (0..100)
            .map(|x| terminal.backend().buffer()[(x, 1)].symbol())
            .collect();
        assert!(header.contains("NOTFALLMAPPE"));
        assert!(!header.contains("Persönliches"));
        app.path = PathBuf::from(":memory:");
        terminal.draw(|frame| app.draw(frame))?;
        let demo_header: String = (0..100)
            .map(|x| terminal.backend().buffer()[(x, 1)].symbol())
            .collect();
        assert_eq!(header, demo_header);
        let key = |code| event::KeyEvent::new(code, M::NONE);
        app.key(key(K::Down))?;
        assert_eq!(app.section, 1);
        app.key(key(K::Right))?;
        app.key(key(K::Down))?;
        assert_eq!(app.section, 1);
        app.key(key(K::Left))?;
        app.key(key(K::Up))?;
        assert_eq!(app.section, 0);

        app.key(key(K::Char('n')))?;
        assert!(matches!(app.mode, Mode::Browse));
        app.key(key(K::Enter))?;
        for c in "Müller".chars() {
            app.key(key(K::Char(c)))?;
        }
        app.key(key(K::Left))?;
        app.key(key(K::Backspace))?;
        app.key(key(K::Char('e')))?;
        app.key(key(K::F(2)))?;
        assert_eq!(app.entries[0].values[0], "Müller");
        app.key(key(K::Enter))?;
        app.key(key(K::Char('X')))?;
        app.key(key(K::Esc))?;
        assert_eq!(app.entries[0].values[0], "Müller");
        app.key(key(K::Char('d')))?;
        app.key(key(K::Char('n')))?;
        assert_eq!(app.entries.len(), 1);
        app.key(key(K::Char('d')))?;
        app.key(key(K::Char('j')))?;
        assert!(app.entries.is_empty());
        app.section = 2;
        app.key(key(K::Enter))?;
        app.key(key(K::Tab))?;
        for c in "Penicillin".chars() {
            app.key(key(K::Char(c)))?;
        }
        app.key(key(K::Enter))?;
        for c in "Hausstaub".chars() {
            app.key(key(K::Char(c)))?;
        }
        app.key(key(K::F(2)))?;
        assert_eq!(app.entries[0].values[1], "Penicillin\nHausstaub");
        Ok(())
    }
}
