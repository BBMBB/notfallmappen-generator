use crate::model::{Entry, SECTIONS, is_bullet_field};
use anyhow::{Context, Result};
use genpdf::{Element, elements, fonts, style};
use lopdf::{Object, dictionary};
use std::{
    cell::{Cell, RefCell},
    fs::OpenOptions,
    io::Write,
    path::Path,
    rc::Rc,
};

type Position = (usize, f64);
type Locations = Rc<RefCell<Vec<Position>>>;
type Rendered = (Vec<u8>, Vec<Position>);
// Capture the actual page and position after layout, including automatic page breaks.
struct Located<E> {
    inner: E,
    index: usize,
    page: Rc<Cell<usize>>,
    locations: Locations,
}
impl<E: Element> Element for Located<E> {
    fn render(
        &mut self,
        context: &genpdf::Context,
        area: genpdf::render::Area<'_>,
        style: style::Style,
    ) -> std::result::Result<genpdf::RenderResult, genpdf::error::Error> {
        let height: f64 = area.size().height.into();
        let top = 277.0 - height;
        let result = self.inner.render(context, area, style)?;
        if result.size.height > genpdf::Mm::from(0) && self.locations.borrow()[self.index].0 == 0 {
            self.locations.borrow_mut()[self.index] = (self.page.get(), top);
        }
        Ok(result)
    }
}
struct PageDecorator {
    inner: genpdf::SimplePageDecorator,
    page: usize,
    total: usize,
}
impl genpdf::PageDecorator for PageDecorator {
    fn decorate_page<'a>(
        &mut self,
        context: &genpdf::Context,
        area: genpdf::render::Area<'a>,
        style: style::Style,
    ) -> std::result::Result<genpdf::render::Area<'a>, genpdf::error::Error> {
        self.page += 1;
        let mut footer_area = area.clone();
        footer_area.add_margins(genpdf::Margins::trbl(282, 20, 5, 20));
        let mut footer =
            elements::Paragraph::new(format!("Seite {:02}/{:02}", self.page, self.total))
                .aligned(genpdf::Alignment::Center)
                .styled(
                    style::Style::new()
                        .with_font_size(9)
                        .with_color(style::Color::Rgb(100, 100, 100)),
                );
        footer.render(context, footer_area, style)?;
        self.inner.decorate_page(context, area, style)
    }
}

fn render(entries: &[Entry], page_numbers: &[usize], total: usize) -> Result<Rendered> {
    let font = |data: &[u8]| fonts::FontData::new(data.to_vec(), None);
    let family = fonts::FontFamily {
        regular: font(include_bytes!("../assets/Regular.ttf"))?,
        bold: font(include_bytes!("../assets/Bold.ttf"))?,
        italic: font(include_bytes!("../assets/Italic.ttf"))?,
        bold_italic: font(include_bytes!("../assets/BoldItalic.ttf"))?,
    };
    let mut doc = genpdf::Document::new(family);
    doc.set_title("Notfallmappe");
    doc.set_paper_size(genpdf::PaperSize::A4);
    doc.set_font_size(11);
    doc.set_line_spacing(1.3);
    doc.set_minimal_conformance();
    let page = Rc::new(Cell::new(0));
    let locations: Locations = Rc::new(RefCell::new(vec![(0, 0.0); SECTIONS.len() * 2]));
    let mut decorator = genpdf::SimplePageDecorator::new();
    decorator.set_margins(20);
    let counter = page.clone();
    decorator.set_header(move |number| {
        counter.set(number);
        let mut header = elements::LinearLayout::vertical();
        if number > 1 {
            header.push(
                elements::Paragraph::new("NOTFALLMAPPE  /  VERTRAULICH").styled(
                    style::Style::new()
                        .with_font_size(9)
                        .with_color(style::Color::Rgb(100, 100, 100)),
                ),
            );
        }
        header.push(elements::Break::new(1));
        header
    });
    doc.set_page_decorator(PageDecorator {
        inner: decorator,
        page: 0,
        total,
    });
    doc.push(elements::Break::new(7));
    doc.push(
        elements::Paragraph::new("NOTFALLMAPPE").styled(
            style::Style::new()
                .bold()
                .with_font_size(34)
                .with_color(style::Color::Rgb(35, 55, 70)),
        ),
    );
    doc.push(elements::Break::new(1));
    let name = entries
        .iter()
        .find(|e| e.section == 0)
        .and_then(|e| e.values.first())
        .filter(|v| !v.is_empty())
        .map(String::as_str)
        .unwrap_or("Persönliche Unterlagen für den Notfall");
    doc.push(elements::Paragraph::new(name).styled(style::Style::new().with_font_size(19)));
    doc.push(elements::Break::new(2));
    doc.push(elements::Paragraph::new(format!(
        "Erstellt am {}",
        chrono::Local::now().format("%d.%m.%Y")
    )));
    doc.push(elements::Break::new(8));
    doc.push(
        elements::Paragraph::new("VERTRAULICH")
            .styled(style::Style::new().bold().with_font_size(10)),
    );
    doc.push(elements::Paragraph::new("Diese Mappe enthält persönliche Angaben. Bitte an einem geschützten, im Notfall zugänglichen Ort aufbewahren.").styled(style::Style::new().with_font_size(10)));
    doc.push(elements::PageBreak::new());
    doc.push(
        elements::Paragraph::new("Inhaltsverzeichnis")
            .styled(style::Style::new().bold().with_font_size(25)),
    );
    doc.push(elements::Break::new(1));
    doc.push(
        elements::Paragraph::new(
            "Ein Klick auf einen Eintrag führt direkt zum jeweiligen Bereich.",
        )
        .styled(style::Style::new().with_font_size(10)),
    );
    doc.push(elements::Break::new(2));
    for (i, section) in SECTIONS.iter().enumerate() {
        let color = style::Style::new()
            .with_font_size(13)
            .with_color(style::Color::Rgb(35, 65, 90));
        let mut row = elements::TableLayout::new(vec![1, 8, 2]);
        row.row()
            .element(elements::Paragraph::new(format!("{:02}", i + 1)).styled(color))
            .element(elements::Paragraph::new(section.title).styled(color))
            .element(
                elements::Paragraph::new(format!(
                    "Seite {}",
                    page_numbers.get(i).copied().unwrap_or(0)
                ))
                .aligned(genpdf::Alignment::Right)
                .styled(color),
            )
            .push()?;
        doc.push(Located {
            inner: row,
            index: SECTIONS.len() + i,
            page: page.clone(),
            locations: locations.clone(),
        });
        doc.push(elements::Break::new(1));
    }
    for (i, section) in SECTIONS.iter().enumerate() {
        doc.push(elements::PageBreak::new());
        doc.push(Located {
            inner: elements::Paragraph::new(format!("{:02}  {}", i + 1, section.title))
                .styled(style::Style::new().bold().with_font_size(18)),
            index: i,
            page: page.clone(),
            locations: locations.clone(),
        });
        doc.push(elements::Break::new(0.8));
        let selected: Vec<_> = entries.iter().filter(|e| e.section == i).collect();
        if selected.is_empty() {
            doc.push(elements::Paragraph::new("Noch keine Angaben erfasst."));
        }
        for (n, entry) in selected.iter().enumerate() {
            let title = if section.single {
                format!("Stand: {}", entry.updated)
            } else {
                format!("Eintrag {}  |  Stand: {}", n + 1, entry.updated)
            };
            doc.push(
                elements::Paragraph::new(title)
                    .styled(style::Style::new().bold().with_font_size(10)),
            );
            for (j, (field, value)) in section.fields.iter().zip(&entry.values).enumerate() {
                if is_bullet_field(i, j) && !value.trim().is_empty() {
                    doc.push(
                        elements::Paragraph::new(format!("{field}:"))
                            .styled(style::Style::new().bold()),
                    );
                    for item in value.lines().map(str::trim).filter(|v| !v.is_empty()) {
                        doc.push(
                            elements::Paragraph::new(format!("• {item}"))
                                .padded(genpdf::Margins::trbl(0, 0, 0, 3)),
                        );
                    }
                } else {
                    let mut p = elements::Paragraph::default();
                    p.push_styled(format!("{field}: "), style::Style::new().bold());
                    p.push(if value.trim().is_empty() {
                        "Keine Angabe"
                    } else {
                        value
                    });
                    doc.push(p);
                }
            }
            doc.push(elements::Break::new(1));
        }
    }
    let mut bytes = Vec::new();
    doc.render(&mut bytes)
        .context("PDF konnte nicht erstellt werden (z. B. nicht unterstützte Schriftzeichen)")?;
    let positions = locations.borrow().clone();
    Ok((bytes, positions))
}

fn add_navigation(bytes: &[u8], positions: &[(usize, f64)]) -> Result<Vec<u8>> {
    let mut doc = lopdf::Document::load_mem(bytes)?;
    let pages = doc.get_pages();
    let outline_root = doc.new_object_id();
    let outlines: Vec<_> = SECTIONS.iter().map(|_| doc.new_object_id()).collect();
    let pt = 72.0 / 25.4;
    for (i, section) in SECTIONS.iter().enumerate() {
        let (number, top) = positions[i];
        let dest = Object::Array(vec![
            Object::Reference(pages[&(number as u32)]),
            Object::Name(b"XYZ".to_vec()),
            Object::Null,
            Object::Real((297.0 - top) * pt),
            Object::Null,
        ]);
        let (toc_page, toc_top) = positions[SECTIONS.len() + i];
        let annotation=doc.add_object(dictionary! {
            "Type"=>"Annot", "Subtype"=>"Link",
            "Rect"=> vec![Object::Real(20.0*pt),Object::Real((297.0-toc_top-7.0)*pt),Object::Real(190.0*pt),Object::Real((297.0-toc_top)*pt)],
            "Border"=> vec![0.into(),0.into(),0.into()], "Dest"=>dest.clone(),
        });
        let toc = doc
            .get_object_mut(pages[&(toc_page as u32)])?
            .as_dict_mut()?;
        if !toc.has(b"Annots") {
            toc.set("Annots", Object::Array(vec![]));
        }
        toc.get_mut(b"Annots")?
            .as_array_mut()?
            .push(annotation.into());
        let title: Vec<u8> = [
            vec![0xfe, 0xff],
            section
                .title
                .encode_utf16()
                .flat_map(u16::to_be_bytes)
                .collect(),
        ]
        .concat();
        let mut outline = dictionary! {"Title"=>Object::String(title,lopdf::StringFormat::Hexadecimal),"Parent"=>outline_root,"Dest"=>dest};
        if i > 0 {
            outline.set("Prev", outlines[i - 1]);
        }
        if i + 1 < outlines.len() {
            outline.set("Next", outlines[i + 1]);
        }
        doc.objects.insert(outlines[i], Object::Dictionary(outline));
    }
    doc.objects.insert(outline_root,Object::Dictionary(dictionary! {"Type"=>"Outlines","First"=>outlines[0],"Last"=>*outlines.last().unwrap(),"Count"=>SECTIONS.len() as i64}));
    let catalog = doc.trailer.get(b"Root")?.as_reference()?;
    doc.get_object_mut(catalog)?
        .as_dict_mut()?
        .set("Outlines", outline_root);
    let mut result = Vec::new();
    doc.save_to(&mut result)?;
    Ok(result)
}

pub fn export(entries: &[Entry], path: &Path) -> Result<()> {
    anyhow::ensure!(
        !path.exists(),
        "PDF-Datei existiert bereits; bitte ein anderes Ziel wählen."
    );
    // First pass measures actual destinations, second pass prints correct TOC page numbers.
    let (measured, locations) = render(entries, &[], 0)?;
    let total = lopdf::Document::load_mem(&measured)?.get_pages().len();
    let numbers: Vec<_> = locations[..SECTIONS.len()].iter().map(|p| p.0).collect();
    let (bytes, locations) = render(entries, &numbers, total)?;
    let bytes = add_navigation(&bytes, &locations)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .context("PDF-Ziel nicht verfügbar oder Datei existiert bereits")?;
    file.write_all(&bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn multipage_export_and_overwrite_protection() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("mappe.pdf");
        let entries = (0..60)
            .map(|i| Entry {
                id: i + 1,
                section: 1,
                values: vec![
                    format!("Kontakt {i} Müller"),
                    "Familie".into(),
                    "030 1234567".into(),
                    "kontakt@beispiel.example".into(),
                    "Langer Hinweis mit mehreren Wörtern. ".repeat(12),
                ],
                updated: "01.11.2026".into(),
            })
            .collect::<Vec<_>>();
        export(&entries, &path)?;
        let original = std::fs::read(&path)?;
        assert!(original.starts_with(b"%PDF-"));
        let parsed = lopdf::Document::load_mem(&original)?;
        let pages = parsed.get_pages();
        assert!(pages.len() > SECTIONS.len() + 2);
        let toc = parsed.get_object(pages[&2])?.as_dict()?;
        let links = toc.get(b"Annots")?.as_array()?;
        assert_eq!(links.len(), SECTIONS.len());
        let mut previous_page = 2;
        let mut chapter_top: Option<f64> = None;
        for link in links {
            let annot = parsed.get_object(link.as_reference()?)?.as_dict()?;
            let dest = annot.get(b"Dest")?.as_array()?;
            assert!(
                pages
                    .values()
                    .any(|id| *id == dest[0].as_reference().unwrap())
            );
            let target = dest[0].as_reference()?;
            let number = *pages.iter().find(|(_, id)| **id == target).unwrap().0;
            assert!(
                number > previous_page,
                "Jeder Bereich beginnt auf einer eigenen neuen Seite"
            );
            if previous_page == 2 {
                assert_eq!(number, 3);
            }
            previous_page = number;
            let top = dest[3].as_f64()?;
            if let Some(expected) = chapter_top {
                assert!((top - expected).abs() < 0.01);
            }
            chapter_top = Some(top);
            assert!(annot.get(b"Rect")?.as_array()?.len() == 4);
        }
        let catalog = parsed
            .get_object(parsed.trailer.get(b"Root")?.as_reference()?)?
            .as_dict()?;
        let outlines = parsed
            .get_object(catalog.get(b"Outlines")?.as_reference()?)?
            .as_dict()?;
        assert_eq!(outlines.get(b"Count")?.as_i64()?, 7);
        assert!(export(&[], &path).is_err());
        assert_eq!(std::fs::read(&path)?, original);
        Ok(())
    }
}
