//! Minimal XLSX (OOXML) writer: sheets of typed cells, inline strings, no shared strings, no styles
//! beyond a bold header. Built on the stored ZIP writer.

use crate::zip::{ZipError, ZipWriter};

#[derive(Clone, Debug, PartialEq)]
pub enum Cell {
    Empty,
    Text(String),
    /// Pre-formatted decimal text (written as a number cell; the same text goes into the CSV).
    Num(String),
}

pub struct Sheet {
    pub name: String,
    pub rows: Vec<Vec<Cell>>,
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            c if (c as u32) < 0x20 && c != '\t' && c != '\n' && c != '\r' => {}
            c => o.push(c),
        }
    }
    o
}

fn col_name(mut i: usize) -> String {
    let mut s = Vec::new();
    loop {
        s.push(b'A' + (i % 26) as u8);
        if i < 26 {
            break;
        }
        i = i / 26 - 1;
    }
    s.reverse();
    String::from_utf8(s).unwrap_or_default()
}

fn sheet_name(raw: &str, idx: usize) -> String {
    let n: String = raw
        .chars()
        .filter(|c| !matches!(c, '[' | ']' | ':' | '*' | '?' | '/' | '\\'))
        .take(31)
        .collect();
    if n.is_empty() {
        format!("Sheet{}", idx + 1)
    } else {
        n
    }
}

fn sheet_xml(s: &Sheet) -> String {
    let mut x = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData>",
    );
    for (r, row) in s.rows.iter().enumerate() {
        x.push_str(&format!("<row r=\"{}\">", r + 1));
        for (c, cell) in row.iter().enumerate() {
            let at = format!("{}{}", col_name(c), r + 1);
            let st = if r == 0 { " s=\"1\"" } else { "" };
            match cell {
                Cell::Empty => {}
                Cell::Text(t) => x.push_str(&format!(
                    "<c r=\"{at}\"{st} t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>",
                    esc(t)
                )),
                Cell::Num(n) => x.push_str(&format!("<c r=\"{at}\"{st}><v>{}</v></c>", esc(n))),
            }
        }
        x.push_str("</row>");
    }
    x.push_str("</sheetData></worksheet>");
    x
}

/// Build an XLSX file from sheets. Deterministic.
pub fn write_xlsx(sheets: &[Sheet]) -> Result<Vec<u8>, ZipError> {
    let empty = [Sheet {
        name: "Sheet1".into(),
        rows: vec![],
    }];
    let sheets = if sheets.is_empty() {
        &empty[..]
    } else {
        sheets
    };
    let mut z = ZipWriter::new();
    let mut ct = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/><Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/>",
    );
    let mut wb = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheets>",
    );
    let mut rels = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
    );
    for (i, s) in sheets.iter().enumerate() {
        let n = i + 1;
        ct.push_str(&format!("<Override PartName=\"/xl/worksheets/sheet{n}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>"));
        wb.push_str(&format!(
            "<sheet name=\"{}\" sheetId=\"{n}\" r:id=\"rId{n}\"/>",
            esc(&sheet_name(&s.name, i))
        ));
        rels.push_str(&format!("<Relationship Id=\"rId{n}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet{n}.xml\"/>"));
    }
    let sn = sheets.len() + 1;
    rels.push_str(&format!("<Relationship Id=\"rId{sn}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/></Relationships>"));
    ct.push_str("</Types>");
    wb.push_str("</sheets></workbook>");
    z.add("[Content_Types].xml", ct.as_bytes())?;
    z.add(
        "_rels/.rels",
        b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>",
    )?;
    z.add("xl/workbook.xml", wb.as_bytes())?;
    z.add("xl/_rels/workbook.xml.rels", rels.as_bytes())?;
    z.add(
        "xl/styles.xml",
        b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<styleSheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><fonts count=\"2\"><font><sz val=\"11\"/><name val=\"Calibri\"/></font><font><b/><sz val=\"11\"/><name val=\"Calibri\"/></font></fonts><fills count=\"2\"><fill><patternFill patternType=\"none\"/></fill><fill><patternFill patternType=\"gray125\"/></fill></fills><borders count=\"1\"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs><cellXfs count=\"2\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/><xf numFmtId=\"0\" fontId=\"1\" fillId=\"0\" borderId=\"0\" xfId=\"0\" applyFont=\"1\"/></cellXfs></styleSheet>",
    )?;
    for (i, s) in sheets.iter().enumerate() {
        z.add(
            &format!("xl/worksheets/sheet{}.xml", i + 1),
            sheet_xml(s).as_bytes(),
        )?;
    }
    z.finish()
}
