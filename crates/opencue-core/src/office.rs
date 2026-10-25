//! Documents an agent can hand over as artifacts: Word (`.docx`), Excel (`.xlsx`), PowerPoint
//! (`.pptx`) and PDF. They are written here, without any office suite, from the plain text the
//! agent sends: Markdown for documents and PDFs, JSON (or CSV) for spreadsheets and Markdown
//! with `---` between slides for presentations.

use std::io::{Cursor, Write};

use serde_json::Value;

// ---------- shared ----------

/// Escape text for an XML element or attribute.
fn xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            // XML 1.0 can't hold most control characters.
            c if (c as u32) < 0x20 && c != '\n' && c != '\t' => {}
            c => out.push(c),
        }
    }
    out
}

fn zip_parts(parts: Vec<(String, String)>) -> anyhow::Result<Vec<u8>> {
    let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, body) in parts {
        zw.start_file(name, opts)?;
        zw.write_all(body.as_bytes())?;
    }
    Ok(zw.finish()?.into_inner())
}

const XML_HEAD: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;

/// A styled piece of a line: plain, **bold**, *italic* or `code`.
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
}

/// Split inline Markdown into spans. Links keep their text; images are dropped to their alt text.
pub fn inline_spans(line: &str) -> Vec<Span> {
    let chars: Vec<char> = line.chars().collect();
    let mut spans: Vec<Span> = Vec::new();
    let (mut bold, mut italic) = (false, false);
    let mut buf = String::new();
    let flush = |buf: &mut String, spans: &mut Vec<Span>, bold: bool, italic: bool, code: bool| {
        if !buf.is_empty() {
            spans.push(Span {
                text: std::mem::take(buf),
                bold,
                italic,
                code,
            });
        }
    };
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '`' {
            if let Some(end) = chars[i + 1..].iter().position(|x| *x == '`') {
                flush(&mut buf, &mut spans, bold, italic, false);
                let code: String = chars[i + 1..i + 1 + end].iter().collect();
                spans.push(Span {
                    text: code,
                    bold,
                    italic,
                    code: true,
                });
                i += end + 2;
                continue;
            }
        }
        if (c == '*' || c == '_') && chars.get(i + 1) == Some(&c) {
            // `__` inside a word is not emphasis.
            let word_inner = c == '_' && i > 0 && chars[i - 1].is_alphanumeric();
            if !word_inner {
                flush(&mut buf, &mut spans, bold, italic, false);
                bold = !bold;
                i += 2;
                continue;
            }
        }
        if c == '*'
            && chars
                .get(i + 1)
                .is_some_and(|n| !n.is_whitespace() || italic)
        {
            flush(&mut buf, &mut spans, bold, italic, false);
            italic = !italic;
            i += 1;
            continue;
        }
        if (c == '[' || (c == '!' && chars.get(i + 1) == Some(&'['))) && i + 1 < chars.len() {
            let start = if c == '!' { i + 2 } else { i + 1 };
            if let Some(close) = chars[start..].iter().position(|x| *x == ']') {
                let after = start + close + 1;
                if chars.get(after) == Some(&'(') {
                    if let Some(paren) = chars[after..].iter().position(|x| *x == ')') {
                        buf.extend(&chars[start..start + close]);
                        i = after + paren + 1;
                        continue;
                    }
                }
            }
        }
        buf.push(c);
        i += 1;
    }
    flush(&mut buf, &mut spans, bold, italic, false);
    spans
}

/// One block of a Markdown document.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading(u8, String),
    Paragraph(String),
    Bullet(usize, String),
    Numbered(usize, String),
    Code(String),
    Table(Vec<Vec<String>>),
    Rule,
}

fn table_cells(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|c| c.trim().to_string())
        .collect()
}

fn is_table_divider(line: &str) -> bool {
    let t = line.trim();
    t.contains('-') && t.contains('|') && t.chars().all(|c| "|-: ".contains(c))
}

/// A small Markdown reader: headings, paragraphs, bullet and numbered lists, code blocks, tables
/// and rules. That is all a document needs.
pub fn parse_markdown(src: &str) -> Vec<Block> {
    let lines: Vec<&str> = src.lines().collect();
    let mut blocks = Vec::new();
    let mut para: Vec<String> = Vec::new();
    let flush = |para: &mut Vec<String>, blocks: &mut Vec<Block>| {
        if !para.is_empty() {
            blocks.push(Block::Paragraph(para.join(" ")));
            para.clear();
        }
    };
    let mut i = 0;
    while i < lines.len() {
        let raw = lines[i];
        let line = raw.trim_end();
        let trimmed = line.trim_start();
        let indent = (line.len() - trimmed.len()) / 2;
        if trimmed.starts_with("```") {
            flush(&mut para, &mut blocks);
            let mut code = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with("```") {
                code.push(lines[i]);
                i += 1;
            }
            blocks.push(Block::Code(code.join("\n")));
            i += 1;
            continue;
        }
        if trimmed.is_empty() {
            flush(&mut para, &mut blocks);
        } else if let Some(level) = heading_level(trimmed) {
            flush(&mut para, &mut blocks);
            blocks.push(Block::Heading(
                level,
                trimmed[level as usize..]
                    .trim()
                    .trim_matches('#')
                    .trim()
                    .to_string(),
            ));
        } else if trimmed.len() >= 3 && trimmed.chars().all(|c| c == '-' || c == '*' || c == '_') {
            flush(&mut para, &mut blocks);
            blocks.push(Block::Rule);
        } else if let Some(rest) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| trimmed.strip_prefix("+ "))
        {
            flush(&mut para, &mut blocks);
            blocks.push(Block::Bullet(indent, rest.trim().to_string()));
        } else if let Some(rest) = numbered(trimmed) {
            flush(&mut para, &mut blocks);
            blocks.push(Block::Numbered(indent, rest.to_string()));
        } else if trimmed.contains('|')
            && lines.get(i + 1).is_some_and(|next| is_table_divider(next))
        {
            flush(&mut para, &mut blocks);
            let mut rows = vec![table_cells(trimmed)];
            i += 2;
            while i < lines.len() && lines[i].contains('|') && !lines[i].trim().is_empty() {
                rows.push(table_cells(lines[i]));
                i += 1;
            }
            blocks.push(Block::Table(rows));
            continue;
        } else {
            para.push(trimmed.to_string());
        }
        i += 1;
    }
    flush(&mut para, &mut blocks);
    blocks
}

fn heading_level(line: &str) -> Option<u8> {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    (1..=6)
        .contains(&hashes)
        .then_some(())
        .filter(|_| line[hashes..].starts_with(' '))
        .map(|_| hashes as u8)
}

fn numbered(line: &str) -> Option<&str> {
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 || digits > 3 {
        return None;
    }
    line[digits..]
        .strip_prefix(". ")
        .or_else(|| line[digits..].strip_prefix(") "))
        .map(str::trim)
}

// ---------- docx ----------

fn docx_runs(text: &str, base_bold: bool, size: Option<u32>) -> String {
    let mut out = String::new();
    for s in inline_spans(text) {
        let mut props = String::new();
        if s.bold || base_bold {
            props.push_str("<w:b/>");
        }
        if s.italic {
            props.push_str("<w:i/>");
        }
        if s.code {
            props.push_str(r#"<w:rFonts w:ascii="Consolas" w:hAnsi="Consolas" w:cs="Consolas"/><w:shd w:val="clear" w:color="auto" w:fill="EEEEEE"/>"#);
        }
        if let Some(sz) = size {
            props.push_str(&format!(r#"<w:sz w:val="{sz}"/>"#));
        }
        out.push_str(&format!(
            r#"<w:r><w:rPr>{props}</w:rPr><w:t xml:space="preserve">{}</w:t></w:r>"#,
            xml(&s.text)
        ));
    }
    out
}

/// A Word document from Markdown.
pub fn docx(markdown: &str, title: &str) -> anyhow::Result<Vec<u8>> {
    let mut body = String::new();
    let mut counter: std::collections::HashMap<usize, u32> = Default::default();
    for block in parse_markdown(markdown) {
        if !matches!(block, Block::Numbered(..)) {
            counter.clear();
        }
        match block {
            Block::Heading(level, text) => {
                let style = format!("Heading{}", level.min(3));
                body.push_str(&format!(
                    r#"<w:p><w:pPr><w:pStyle w:val="{style}"/></w:pPr>{}</w:p>"#,
                    docx_runs(&text, false, None)
                ));
            }
            Block::Paragraph(text) => body.push_str(&format!(
                "<w:p>{}</w:p>",
                docx_runs(&text, false, None)
            )),
            Block::Bullet(depth, text) => body.push_str(&format!(
                r#"<w:p><w:pPr><w:pStyle w:val="ListParagraph"/><w:ind w:left="{}" w:hanging="260"/></w:pPr><w:r><w:t xml:space="preserve">•  </w:t></w:r>{}</w:p>"#,
                540 + depth * 360,
                docx_runs(&text, false, None)
            )),
            Block::Numbered(depth, text) => {
                let n = counter.entry(depth).or_insert(0);
                *n += 1;
                body.push_str(&format!(
                    r#"<w:p><w:pPr><w:pStyle w:val="ListParagraph"/><w:ind w:left="{}" w:hanging="360"/></w:pPr><w:r><w:t xml:space="preserve">{}.  </w:t></w:r>{}</w:p>"#,
                    540 + depth * 360,
                    n,
                    docx_runs(&text, false, None)
                ));
            }
            Block::Code(code) => {
                for line in code.lines().chain(code.is_empty().then_some("")) {
                    body.push_str(&format!(
                        r#"<w:p><w:pPr><w:pStyle w:val="Code"/></w:pPr><w:r><w:t xml:space="preserve">{}</w:t></w:r></w:p>"#,
                        xml(line)
                    ));
                }
            }
            Block::Table(rows) => {
                let cols = rows.iter().map(Vec::len).max().unwrap_or(1).max(1);
                let mut t = String::from(
                    r#"<w:tbl><w:tblPr><w:tblStyle w:val="TableGrid"/><w:tblW w:w="0" w:type="auto"/><w:tblBorders><w:top w:val="single" w:sz="4" w:space="0" w:color="999999"/><w:left w:val="single" w:sz="4" w:space="0" w:color="999999"/><w:bottom w:val="single" w:sz="4" w:space="0" w:color="999999"/><w:right w:val="single" w:sz="4" w:space="0" w:color="999999"/><w:insideH w:val="single" w:sz="4" w:space="0" w:color="999999"/><w:insideV w:val="single" w:sz="4" w:space="0" w:color="999999"/></w:tblBorders></w:tblPr><w:tblGrid>"#,
                );
                for _ in 0..cols {
                    t.push_str(&format!(r#"<w:gridCol w:w="{}"/>"#, 9000 / cols));
                }
                t.push_str("</w:tblGrid>");
                for (r, row) in rows.iter().enumerate() {
                    t.push_str("<w:tr>");
                    for c in 0..cols {
                        let cell = row.get(c).map(String::as_str).unwrap_or("");
                        let shade = if r == 0 {
                            r#"<w:shd w:val="clear" w:color="auto" w:fill="E8E8E8"/>"#
                        } else {
                            ""
                        };
                        t.push_str(&format!(
                            r#"<w:tc><w:tcPr><w:tcW w:w="{}" w:type="dxa"/>{shade}</w:tcPr><w:p>{}</w:p></w:tc>"#,
                            9000 / cols,
                            docx_runs(cell, r == 0, None)
                        ));
                    }
                    t.push_str("</w:tr>");
                }
                t.push_str("</w:tbl><w:p/>");
                body.push_str(&t);
            }
            Block::Rule => body.push_str(
                r#"<w:p><w:pPr><w:pBdr><w:bottom w:val="single" w:sz="6" w:space="1" w:color="999999"/></w:pBdr></w:pPr></w:p>"#,
            ),
        }
    }
    if body.is_empty() {
        body.push_str("<w:p/>");
    }
    let document = format!(
        r#"{XML_HEAD}<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1300" w:bottom="1440" w:left="1300" w:header="708" w:footer="708" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    );
    let styles = format!(
        r#"{XML_HEAD}<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Calibri" w:hAnsi="Calibri" w:cs="Calibri"/><w:sz w:val="22"/><w:szCs w:val="22"/><w:lang w:val="en-US"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after="140" w:line="276" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="360" w:after="120"/><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/><w:sz w:val="40"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="280" w:after="100"/><w:outlineLvl w:val="1"/></w:pPr><w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading3"><w:name w:val="heading 3"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="240" w:after="80"/><w:outlineLvl w:val="2"/></w:pPr><w:rPr><w:b/><w:sz w:val="26"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="ListParagraph"><w:name w:val="List Paragraph"/><w:basedOn w:val="Normal"/><w:qFormat/><w:pPr><w:spacing w:after="60"/></w:pPr></w:style><w:style w:type="paragraph" w:styleId="Code"><w:name w:val="Code"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/><w:shd w:val="clear" w:color="auto" w:fill="F2F2F2"/></w:pPr><w:rPr><w:rFonts w:ascii="Consolas" w:hAnsi="Consolas" w:cs="Consolas"/><w:sz w:val="19"/></w:rPr></w:style><w:style w:type="table" w:styleId="TableGrid"><w:name w:val="Table Grid"/><w:basedOn w:val="TableNormal"/><w:tblPr><w:tblCellMar><w:left w:w="108" w:type="dxa"/><w:right w:w="108" w:type="dxa"/></w:tblCellMar></w:tblPr></w:style><w:style w:type="table" w:default="1" w:styleId="TableNormal"><w:name w:val="Normal Table"/><w:tblPr><w:tblInd w:w="0" w:type="dxa"/><w:tblCellMar><w:top w:w="0" w:type="dxa"/><w:left w:w="108" w:type="dxa"/><w:bottom w:w="0" w:type="dxa"/><w:right w:w="108" w:type="dxa"/></w:tblCellMar></w:tblPr></w:style></w:styles>"#
    );
    let content_types = format!(
        r#"{XML_HEAD}<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/></Types>"#
    );
    let rels = format!(
        r#"{XML_HEAD}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/></Relationships>"#
    );
    let doc_rels = format!(
        r#"{XML_HEAD}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#
    );
    zip_parts(vec![
        ("[Content_Types].xml".into(), content_types),
        ("_rels/.rels".into(), rels),
        ("word/document.xml".into(), document),
        ("word/styles.xml".into(), styles),
        ("word/_rels/document.xml.rels".into(), doc_rels),
        ("docProps/core.xml".into(), core_props(title)),
    ])
}

fn core_props(title: &str) -> String {
    format!(
        r#"{XML_HEAD}<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:title>{}</dc:title><dc:creator>OpenCue</dc:creator><dcterms:created xsi:type="dcterms:W3CDTF">{}</dcterms:created></cp:coreProperties>"#,
        xml(title),
        chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ")
    )
}

// ---------- xlsx ----------

/// One sheet of a workbook.
#[derive(Debug, Clone, PartialEq)]
pub struct Sheet {
    pub name: String,
    pub rows: Vec<Vec<Value>>,
    pub header: bool,
}

/// Read a spreadsheet from what the agent sent: `{"sheets":[{"name","rows"}]}`, a single
/// `{"rows":[…]}` / `[[…],[…]]`, or CSV text.
pub fn parse_sheets(src: &str) -> anyhow::Result<Vec<Sheet>> {
    let trimmed = src.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        let v: Value = serde_json::from_str(trimmed)
            .map_err(|e| anyhow::anyhow!("the spreadsheet content is not valid JSON: {e}"))?;
        let sheet = |v: &Value, n: usize| -> anyhow::Result<Sheet> {
            let rows = v
                .get("rows")
                .unwrap_or(v)
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("a sheet needs a `rows` array of arrays"))?
                .iter()
                .map(|r| match r {
                    Value::Array(cells) => cells.clone(),
                    other => vec![other.clone()],
                })
                .collect();
            Ok(Sheet {
                name: sheet_name(v.get("name").and_then(Value::as_str).unwrap_or(""), n),
                rows,
                header: v.get("header").and_then(Value::as_bool).unwrap_or(true),
            })
        };
        let sheets = match v.get("sheets").and_then(Value::as_array) {
            Some(list) => list
                .iter()
                .enumerate()
                .map(|(i, s)| sheet(s, i + 1))
                .collect::<anyhow::Result<Vec<_>>>()?,
            None => vec![sheet(&v, 1)?],
        };
        anyhow::ensure!(!sheets.is_empty(), "the workbook has no sheets");
        return Ok(sheets);
    }
    Ok(vec![Sheet {
        name: "Sheet1".into(),
        rows: parse_csv(trimmed)
            .into_iter()
            .map(|r| r.into_iter().map(Value::String).collect())
            .collect(),
        header: true,
    }])
}

fn sheet_name(name: &str, n: usize) -> String {
    let clean: String = name
        .chars()
        .filter(|c| !r"[]:*?/\".contains(*c))
        .take(31)
        .collect();
    if clean.trim().is_empty() {
        format!("Sheet{n}")
    } else {
        clean
    }
}

/// CSV with quoted fields (`"a,b"`, `""`), commas or tabs.
pub fn parse_csv(src: &str) -> Vec<Vec<String>> {
    let delim = if src
        .lines()
        .next()
        .is_some_and(|l| l.contains('\t') && !l.contains(','))
    {
        '\t'
    } else {
        ','
    };
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cell.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            c if c == delim && !quoted => row.push(std::mem::take(&mut cell)),
            '\r' if !quoted => {}
            '\n' if !quoted => {
                row.push(std::mem::take(&mut cell));
                rows.push(std::mem::take(&mut row));
            }
            c => cell.push(c),
        }
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    rows
}

fn col_letters(mut n: usize) -> String {
    let mut s = String::new();
    loop {
        s.insert(0, (b'A' + (n % 26) as u8) as char);
        if n < 26 {
            break;
        }
        n = n / 26 - 1;
    }
    s
}

fn cell_text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// An Excel workbook. Text starting with `=` becomes a formula; numbers stay numbers.
pub fn xlsx(sheets: &[Sheet]) -> anyhow::Result<Vec<u8>> {
    let mut parts: Vec<(String, String)> = Vec::new();
    let mut overrides = String::new();
    let mut wb_sheets = String::new();
    let mut wb_rels = String::new();
    for (n, sheet) in sheets.iter().enumerate() {
        let idx = n + 1;
        let widths: Vec<usize> = (0..sheet.rows.iter().map(Vec::len).max().unwrap_or(0))
            .map(|c| {
                sheet
                    .rows
                    .iter()
                    .filter_map(|r| r.get(c))
                    .map(|v| cell_text(v).chars().count())
                    .max()
                    .unwrap_or(8)
                    .clamp(8, 60)
            })
            .collect();
        let mut data = String::new();
        for (r, row) in sheet.rows.iter().enumerate() {
            data.push_str(&format!(r#"<row r="{}">"#, r + 1));
            for (c, v) in row.iter().enumerate() {
                let at = format!("{}{}", col_letters(c), r + 1);
                let style = if sheet.header && r == 0 {
                    r#" s="1""#
                } else {
                    ""
                };
                match v {
                    Value::Null => {}
                    Value::Number(num) => {
                        data.push_str(&format!(r#"<c r="{at}"{style}><v>{num}</v></c>"#))
                    }
                    Value::Bool(b) => data.push_str(&format!(
                        r#"<c r="{at}"{style} t="b"><v>{}</v></c>"#,
                        *b as u8
                    )),
                    other => {
                        let text = cell_text(other);
                        if let Some(f) = text.strip_prefix('=').filter(|f| !f.trim().is_empty()) {
                            data.push_str(&format!(r#"<c r="{at}"{style}><f>{}</f></c>"#, xml(f)));
                        } else {
                            data.push_str(&format!(
                                r#"<c r="{at}"{style} t="inlineStr"><is><t xml:space="preserve">{}</t></is></c>"#,
                                xml(&text)
                            ));
                        }
                    }
                }
            }
            data.push_str("</row>");
        }
        let cols: String = widths
            .iter()
            .enumerate()
            .map(|(c, w)| {
                format!(
                    r#"<col min="{0}" max="{0}" width="{1}" customWidth="1"/>"#,
                    c + 1,
                    w + 2
                )
            })
            .collect();
        let pane = if sheet.header && sheet.rows.len() > 1 {
            r#"<sheetViews><sheetView workbookViewId="0"><pane ySplit="1" topLeftCell="A2" activePane="bottomLeft" state="frozen"/></sheetView></sheetViews>"#
        } else {
            ""
        };
        parts.push((
            format!("xl/worksheets/sheet{idx}.xml"),
            format!(
                r#"{XML_HEAD}<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">{pane}{}<sheetData>{data}</sheetData></worksheet>"#,
                if cols.is_empty() {
                    String::new()
                } else {
                    format!("<cols>{cols}</cols>")
                }
            ),
        ));
        overrides.push_str(&format!(
            r#"<Override PartName="/xl/worksheets/sheet{idx}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#
        ));
        wb_sheets.push_str(&format!(
            r#"<sheet name="{}" sheetId="{idx}" r:id="rId{idx}"/>"#,
            xml(&sheet.name)
        ));
        wb_rels.push_str(&format!(
            r#"<Relationship Id="rId{idx}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{idx}.xml"/>"#
        ));
    }
    let styles_id = sheets.len() + 1;
    wb_rels.push_str(&format!(
        r#"<Relationship Id="rId{styles_id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#
    ));
    parts.insert(
        0,
        (
            "[Content_Types].xml".into(),
            format!(
                r#"{XML_HEAD}<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>{overrides}</Types>"#
            ),
        ),
    );
    parts.push((
        "_rels/.rels".into(),
        format!(
            r#"{XML_HEAD}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#
        ),
    ));
    parts.push((
        "xl/workbook.xml".into(),
        format!(
            r#"{XML_HEAD}<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets>{wb_sheets}</sheets></workbook>"#
        ),
    ));
    parts.push((
        "xl/_rels/workbook.xml.rels".into(),
        format!(
            r#"{XML_HEAD}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{wb_rels}</Relationships>"#
        ),
    ));
    parts.push((
        "xl/styles.xml".into(),
        format!(
            r#"{XML_HEAD}<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="2"><font><sz val="11"/><name val="Calibri"/></font><font><b/><sz val="11"/><name val="Calibri"/></font></fonts><fills count="3"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill><fill><patternFill patternType="solid"><fgColor rgb="FFE8E8E8"/><bgColor indexed="64"/></patternFill></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="2"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/><xf numFmtId="0" fontId="1" fillId="2" borderId="0" xfId="0" applyFont="1" applyFill="1"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>"#
        ),
    ));
    zip_parts(parts)
}

// ---------- pptx ----------

/// One slide: a title, bullets (or paragraphs) and speaker notes.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Slide {
    pub title: String,
    pub lines: Vec<String>,
    pub notes: String,
}

/// Slides from Markdown: `---` between slides, `#`/`##` for the title, `-` for bullets, and
/// `Notes:` lines for the speaker. A JSON `{"slides":[{"title","bullets","notes"}]}` also works.
pub fn parse_slides(src: &str) -> Vec<Slide> {
    let trimmed = src.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
            let list = v.get("slides").unwrap_or(&v);
            if let Some(list) = list.as_array() {
                let slides: Vec<Slide> = list
                    .iter()
                    .map(|s| Slide {
                        title: s["title"].as_str().unwrap_or_default().to_string(),
                        lines: s
                            .get("bullets")
                            .or_else(|| s.get("content"))
                            .and_then(Value::as_array)
                            .map(|a| a.iter().map(cell_text).collect())
                            .unwrap_or_default(),
                        notes: s["notes"].as_str().unwrap_or_default().to_string(),
                    })
                    .collect();
                if !slides.is_empty() {
                    return slides;
                }
            }
        }
    }
    let mut slides = Vec::new();
    let mut cur = Slide::default();
    let mut started = false;
    let push = |cur: &mut Slide, slides: &mut Vec<Slide>, started: &mut bool| {
        if *started {
            slides.push(std::mem::take(cur));
            *started = false;
        }
    };
    for line in src.lines() {
        let t = line.trim();
        if t.len() >= 3 && t.chars().all(|c| c == '-') {
            push(&mut cur, &mut slides, &mut started);
            continue;
        }
        if t.is_empty() {
            continue;
        }
        started = true;
        if let Some(level) = heading_level(t) {
            let text = t[level as usize..].trim().to_string();
            if cur.title.is_empty() {
                cur.title = text;
            } else {
                cur.lines.push(text);
            }
        } else if let Some(n) = t
            .strip_prefix("Notes:")
            .or_else(|| t.strip_prefix("Speaker notes:"))
        {
            cur.notes.push_str(n.trim());
        } else {
            let text = t
                .strip_prefix("- ")
                .or_else(|| t.strip_prefix("* "))
                .unwrap_or(t);
            cur.lines.push(text.to_string());
        }
    }
    push(&mut cur, &mut slides, &mut started);
    slides
}

fn pptx_paragraphs(lines: &[String]) -> String {
    let mut out = String::new();
    for l in lines {
        let runs: String = inline_spans(l)
            .into_iter()
            .map(|s| {
                format!(
                    r#"<a:r><a:rPr lang="en-US" sz="2200"{}{} dirty="0"/><a:t>{}</a:t></a:r>"#,
                    if s.bold { r#" b="1""# } else { "" },
                    if s.italic { r#" i="1""# } else { "" },
                    xml(&s.text)
                )
            })
            .collect();
        out.push_str(&format!(
            r#"<a:p><a:pPr marL="342900" indent="-342900"><a:spcBef><a:spcPts val="800"/></a:spcBef><a:buFont typeface="Arial"/><a:buChar char="•"/></a:pPr>{runs}</a:p>"#
        ));
    }
    if out.is_empty() {
        out.push_str(r#"<a:p><a:endParaRPr lang="en-US"/></a:p>"#);
    }
    out
}

/// A PowerPoint deck (16:9) with a title and a bullet box on each slide.
pub fn pptx(slides: &[Slide], title: &str) -> anyhow::Result<Vec<u8>> {
    anyhow::ensure!(!slides.is_empty(), "the presentation has no slides");
    const NS: &str = r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#;
    let mut parts: Vec<(String, String)> = Vec::new();
    let mut overrides = String::new();
    let mut pres_rels = String::from(
        r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>"#,
    );
    let mut slide_ids = String::new();
    for (n, s) in slides.iter().enumerate() {
        let idx = n + 1;
        let only_title = s.lines.is_empty();
        let (ty, ty_h) = if only_title {
            (2_600_000, 1_600_000)
        } else {
            (400_000, 1_100_000)
        };
        let title_box = format!(
            r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="Title"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="609600" y="{ty}"/><a:ext cx="10972800" cy="{ty_h}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr wrap="square" anchor="{}"><a:normAutofit/></a:bodyPr><a:lstStyle/><a:p><a:pPr algn="{}"/><a:r><a:rPr lang="en-US" sz="{}" b="1" dirty="0"/><a:t>{}</a:t></a:r></a:p></p:txBody></p:sp>"#,
            if only_title { "ctr" } else { "b" },
            if only_title { "ctr" } else { "l" },
            if only_title { 4800 } else { 3600 },
            xml(&s.title)
        );
        let body_box = if only_title {
            String::new()
        } else {
            format!(
                r#"<p:sp><p:nvSpPr><p:cNvPr id="3" name="Content"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="609600" y="1700000"/><a:ext cx="10972800" cy="4600000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr wrap="square"><a:normAutofit/></a:bodyPr><a:lstStyle/>{}</p:txBody></p:sp>"#,
                pptx_paragraphs(&s.lines)
            )
        };
        parts.push((
            format!("ppt/slides/slide{idx}.xml"),
            format!(
                r#"{XML_HEAD}<p:sld {NS}><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>{title_box}{body_box}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#
            ),
        ));
        parts.push((
            format!("ppt/slides/_rels/slide{idx}.xml.rels"),
            format!(
                r#"{XML_HEAD}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/></Relationships>"#
            ),
        ));
        overrides.push_str(&format!(
            r#"<Override PartName="/ppt/slides/slide{idx}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#
        ));
        pres_rels.push_str(&format!(
            r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{idx}.xml"/>"#,
            idx + 1
        ));
        slide_ids.push_str(&format!(
            r#"<p:sldId id="{}" r:id="rId{}"/>"#,
            255 + idx,
            idx + 1
        ));
    }
    let theme_rid = slides.len() + 2;
    pres_rels.push_str(&format!(
        r#"<Relationship Id="rId{theme_rid}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/>"#
    ));
    let group = r#"<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>"#;
    parts.insert(
        0,
        (
            "[Content_Types].xml".into(),
            format!(
                r#"{XML_HEAD}<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/><Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/><Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/><Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>{overrides}</Types>"#
            ),
        ),
    );
    parts.push((
        "_rels/.rels".into(),
        format!(
            r#"{XML_HEAD}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/></Relationships>"#
        ),
    ));
    parts.push(("docProps/core.xml".into(), core_props(title)));
    parts.push((
        "ppt/presentation.xml".into(),
        format!(
            r#"{XML_HEAD}<p:presentation {NS}><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst><p:sldIdLst>{slide_ids}</p:sldIdLst><p:sldSz cx="12192000" cy="6858000"/><p:notesSz cx="6858000" cy="9144000"/></p:presentation>"#
        ),
    ));
    parts.push((
        "ppt/_rels/presentation.xml.rels".into(),
        format!(
            r#"{XML_HEAD}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{pres_rels}</Relationships>"#
        ),
    ));
    parts.push((
        "ppt/slideMasters/slideMaster1.xml".into(),
        format!(
            r#"{XML_HEAD}<p:sldMaster {NS}><p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg><p:spTree>{group}</p:spTree></p:cSld><p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/><p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst><p:txStyles><p:titleStyle><a:lvl1pPr><a:defRPr sz="4400"/></a:lvl1pPr></p:titleStyle><p:bodyStyle><a:lvl1pPr><a:defRPr sz="2800"/></a:lvl1pPr></p:bodyStyle><p:otherStyle><a:lvl1pPr><a:defRPr sz="1800"/></a:lvl1pPr></p:otherStyle></p:txStyles></p:sldMaster>"#
        ),
    ));
    parts.push((
        "ppt/slideMasters/_rels/slideMaster1.xml.rels".into(),
        format!(
            r#"{XML_HEAD}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/></Relationships>"#
        ),
    ));
    parts.push((
        "ppt/slideLayouts/slideLayout1.xml".into(),
        format!(
            r#"{XML_HEAD}<p:sldLayout {NS} type="blank" preserve="1"><p:cSld name="Blank"><p:spTree>{group}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"#
        ),
    ));
    parts.push((
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels".into(),
        format!(
            r#"{XML_HEAD}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/></Relationships>"#
        ),
    ));
    parts.push((
        "ppt/theme/theme1.xml".into(),
        format!(
            r#"{XML_HEAD}<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="OpenCue"><a:themeElements><a:clrScheme name="OpenCue"><a:dk1><a:srgbClr val="1F2328"/></a:dk1><a:lt1><a:srgbClr val="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="44546A"/></a:dk2><a:lt2><a:srgbClr val="E7E6E6"/></a:lt2><a:accent1><a:srgbClr val="4F6BED"/></a:accent1><a:accent2><a:srgbClr val="ED7D31"/></a:accent2><a:accent3><a:srgbClr val="2DA44E"/></a:accent3><a:accent4><a:srgbClr val="FFC000"/></a:accent4><a:accent5><a:srgbClr val="8250DF"/></a:accent5><a:accent6><a:srgbClr val="CF222E"/></a:accent6><a:hlink><a:srgbClr val="0563C1"/></a:hlink><a:folHlink><a:srgbClr val="954F72"/></a:folHlink></a:clrScheme><a:fontScheme name="OpenCue"><a:majorFont><a:latin typeface="Calibri"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="Calibri"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme><a:fmtScheme name="OpenCue"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="12700"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="19050"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>"#
        ),
    ));
    zip_parts(parts)
}

// ---------- pdf ----------

const PAGE_W: f64 = 595.0;
const PAGE_H: f64 = 842.0;
const MARGIN: f64 = 56.0;

#[derive(Clone, Copy, PartialEq)]
enum Face {
    Regular,
    Bold,
    Italic,
    Mono,
}

impl Face {
    fn name(self) -> &'static str {
        match self {
            Face::Regular => "F1",
            Face::Bold => "F2",
            Face::Italic => "F3",
            Face::Mono => "F4",
        }
    }
}

/// Helvetica's character widths (1/1000 em) for ASCII 32..=126, regular and bold.
const HELV: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667,
    611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500,
    222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];
const HELV_BOLD: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584, 611, 975, 722, 722, 722, 722, 667,
    611, 778, 722, 278, 556, 722, 611, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 333, 278, 333, 584, 556, 333, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556,
    278, 889, 611, 611, 611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584,
];

/// Width of a character in 1/1000 em (Courier is fixed at 600).
fn char_width(c: char, face: Face) -> f64 {
    if face == Face::Mono {
        return 600.0;
    }
    let table = if face == Face::Bold {
        &HELV_BOLD
    } else {
        &HELV
    };
    let n = c as u32;
    if (32..=126).contains(&n) {
        table[(n - 32) as usize] as f64
    } else if face == Face::Bold {
        611.0
    } else {
        556.0
    }
}

/// Text for a PDF string: Latin-1 bytes (the standard fonts), `?` for anything else.
fn pdf_text(s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for c in s.chars() {
        let b = match c {
            '\u{2018}' | '\u{2019}' => b'\'',
            '\u{201c}' | '\u{201d}' => b'"',
            '\u{2013}' | '\u{2014}' => b'-',
            '\u{2022}' => 0x95,
            '\u{2026}' => 0x85,
            c if (c as u32) < 256 => c as u32 as u8,
            _ => b'?',
        };
        match b {
            b'(' | b')' | b'\\' => {
                out.push(b'\\');
                out.push(b);
            }
            b if b < 0x20 => out.push(b' '),
            b => out.push(b),
        }
    }
    out
}

struct PdfWriter {
    pages: Vec<Vec<u8>>,
    cur: Vec<u8>,
    y: f64,
}

impl PdfWriter {
    fn new() -> Self {
        Self {
            pages: vec![],
            cur: Vec::new(),
            y: PAGE_H - MARGIN,
        }
    }

    fn page_break(&mut self) {
        self.pages.push(std::mem::take(&mut self.cur));
        self.y = PAGE_H - MARGIN;
    }

    fn ensure(&mut self, height: f64) {
        if self.y - height < MARGIN {
            self.page_break();
        }
    }

    /// Draw a line made of (text, face) runs starting at x.
    fn line(&mut self, runs: &[(String, Face)], x: f64, size: f64) {
        let mut at = x;
        for (text, face) in runs {
            if text.is_empty() {
                continue;
            }
            self.cur.extend_from_slice(
                format!(
                    "BT /{} {size:.1} Tf {at:.1} {:.1} Td (",
                    face.name(),
                    self.y
                )
                .as_bytes(),
            );
            self.cur.extend(pdf_text(text));
            self.cur.extend_from_slice(b") Tj ET\n");
            at += text.chars().map(|c| char_width(c, *face)).sum::<f64>() * size / 1000.0;
        }
    }

    fn rule(&mut self) {
        self.ensure(10.0);
        self.cur.extend_from_slice(
            format!(
                "0.6 G 0.5 w {MARGIN:.1} {:.1} m {:.1} {:.1} l S 0 G\n",
                self.y - 4.0,
                PAGE_W - MARGIN,
                self.y - 4.0
            )
            .as_bytes(),
        );
        self.y -= 12.0;
    }

    /// Word-wrapped styled text.
    fn paragraph(
        &mut self,
        text: &str,
        size: f64,
        base: Face,
        indent: f64,
        first: Option<&str>,
        gap: f64,
    ) {
        let width = PAGE_W - 2.0 * MARGIN - indent;
        // Words carry their face so bold and code spans survive wrapping; a word that touches the
        // previous one (no space between two styled spans) stays glued to it.
        let mut words: Vec<(String, Face, bool)> = Vec::new();
        let mut open = true;
        for s in inline_spans(text) {
            let face = if s.code {
                Face::Mono
            } else if s.bold || base == Face::Bold {
                Face::Bold
            } else if s.italic || base == Face::Italic {
                Face::Italic
            } else {
                base
            };
            if s.text.starts_with(char::is_whitespace) {
                open = true;
            }
            for w in s.text.split_whitespace() {
                words.push((w.to_string(), face, open));
                open = true;
            }
            // Only whitespace at the end of the span (or the next one's start) separates it
            // from what follows.
            if !s.text.trim().is_empty() {
                open = s.text.ends_with(char::is_whitespace);
            }
            if s.text.ends_with(char::is_whitespace) {
                open = true;
            }
        }
        let leading = size * 1.3;
        let mut lines: Vec<Vec<(String, Face)>> = vec![vec![]];
        let mut used = 0.0;
        for (w, face, spaced) in words {
            let w_width = w.chars().map(|c| char_width(c, face)).sum::<f64>() * size / 1000.0;
            let space = if spaced {
                char_width(' ', face) * size / 1000.0
            } else {
                0.0
            };
            let add = if used == 0.0 {
                w_width
            } else {
                space + w_width
            };
            if used > 0.0 && used + add > width && spaced {
                lines.push(vec![]);
                used = 0.0;
            }
            let line = lines.last_mut().unwrap();
            let lead = if line.is_empty() || !spaced { "" } else { " " };
            line.push((format!("{lead}{w}"), face));
            used += if lead.is_empty() {
                w_width
            } else {
                space + w_width
            };
        }
        for (n, runs) in lines.iter().enumerate() {
            self.ensure(leading);
            self.y -= leading;
            if n == 0 {
                if let Some(bullet) = first {
                    self.line(&[(bullet.to_string(), base)], MARGIN + indent - 14.0, size);
                }
            }
            self.line(runs, MARGIN + indent, size);
        }
        self.y -= gap;
    }

    fn finish(mut self, title: &str) -> Vec<u8> {
        if !self.cur.is_empty() || self.pages.is_empty() {
            self.pages.push(std::mem::take(&mut self.cur));
        }
        let n = self.pages.len();
        // Objects: 1 catalog, 2 pages, 3-6 fonts, 7 info, then a page and its content per page.
        let mut objs: Vec<Vec<u8>> = Vec::new();
        objs.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
        let kids: String = (0..n).map(|i| format!("{} 0 R ", 8 + i * 2)).collect();
        objs.push(format!("<< /Type /Pages /Kids [{kids}] /Count {n} >>").into_bytes());
        for font in [
            "Helvetica",
            "Helvetica-Bold",
            "Helvetica-Oblique",
            "Courier",
        ] {
            objs.push(
                format!("<< /Type /Font /Subtype /Type1 /BaseFont /{font} /Encoding /WinAnsiEncoding >>")
                    .into_bytes(),
            );
        }
        let mut info = b"<< /Title (".to_vec();
        info.extend(pdf_text(title));
        info.extend_from_slice(b") /Producer (OpenCue) >>");
        objs.push(info);
        for (i, content) in self.pages.iter().enumerate() {
            objs.push(
                format!(
                    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_W} {PAGE_H}] /Resources << /Font << /F1 3 0 R /F2 4 0 R /F3 5 0 R /F4 6 0 R >> >> /Contents {} 0 R >>",
                    9 + i * 2
                )
                .into_bytes(),
            );
            let mut stream = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
            stream.extend_from_slice(content);
            stream.extend_from_slice(b"endstream");
            objs.push(stream);
        }
        let mut out = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
        let mut offsets = Vec::new();
        for (i, o) in objs.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
            out.extend_from_slice(o);
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref = out.len();
        out.extend_from_slice(
            format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes(),
        );
        for off in offsets {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R /Info 7 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                objs.len() + 1
            )
            .as_bytes(),
        );
        out
    }
}

/// A PDF from Markdown (A4, standard fonts: Latin text).
pub fn pdf(markdown: &str, title: &str) -> anyhow::Result<Vec<u8>> {
    let mut w = PdfWriter::new();
    let mut counter: std::collections::HashMap<usize, u32> = Default::default();
    for block in parse_markdown(markdown) {
        if !matches!(block, Block::Numbered(..)) {
            counter.clear();
        }
        match block {
            Block::Heading(level, text) => {
                let size = match level {
                    1 => 22.0,
                    2 => 17.0,
                    3 => 14.0,
                    _ => 12.0,
                };
                w.y -= size * 0.5;
                w.paragraph(&text, size, Face::Bold, 0.0, None, 4.0);
            }
            Block::Paragraph(text) => w.paragraph(&text, 11.0, Face::Regular, 0.0, None, 6.0),
            Block::Bullet(depth, text) => w.paragraph(
                &text,
                11.0,
                Face::Regular,
                20.0 + depth as f64 * 16.0,
                Some("\u{2022}"),
                2.0,
            ),
            Block::Numbered(depth, text) => {
                let n = counter.entry(depth).or_insert(0);
                *n += 1;
                let label = format!("{n}.");
                w.paragraph(
                    &text,
                    11.0,
                    Face::Regular,
                    20.0 + depth as f64 * 16.0,
                    Some(&label),
                    2.0,
                );
            }
            Block::Code(code) => {
                for line in code.lines() {
                    // Code keeps its spacing and is cut, not wrapped.
                    let max = ((PAGE_W - 2.0 * MARGIN - 8.0) / (600.0 * 9.0 / 1000.0)) as usize;
                    let cut: String = line.chars().take(max).collect();
                    w.ensure(12.0);
                    w.y -= 11.5;
                    w.line(&[(cut, Face::Mono)], MARGIN + 8.0, 9.0);
                }
                w.y -= 6.0;
            }
            Block::Table(rows) => {
                let cols = rows.iter().map(Vec::len).max().unwrap_or(1).max(1);
                let col_w = (PAGE_W - 2.0 * MARGIN) / cols as f64;
                let max_chars = ((col_w - 6.0) / (540.0 * 10.0 / 1000.0)) as usize;
                for (r, row) in rows.iter().enumerate() {
                    w.ensure(16.0);
                    w.y -= 14.0;
                    for c in 0..cols {
                        let cell: String = row
                            .get(c)
                            .map(|s| s.chars().take(max_chars.max(4)).collect())
                            .unwrap_or_default();
                        let face = if r == 0 { Face::Bold } else { Face::Regular };
                        w.line(&[(cell, face)], MARGIN + c as f64 * col_w + 3.0, 10.0);
                    }
                    if r == 0 {
                        w.rule();
                        w.y += 6.0;
                    }
                }
                w.y -= 8.0;
            }
            Block::Rule => w.rule(),
        }
    }
    Ok(w.finish(title))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(bytes: &[u8]) -> Vec<String> {
        let mut z = zip::ZipArchive::new(Cursor::new(bytes.to_vec())).unwrap();
        (0..z.len())
            .map(|i| z.by_index(i).unwrap().name().to_string())
            .collect()
    }

    #[test]
    fn markdown_blocks() {
        let b = parse_markdown("# Title\n\nHello **world**\nsecond line\n\n- a\n  - b\n1. one\n\n| x | y |\n|---|---|\n| 1 | 2 |\n\n```\ncode\n```\n---");
        assert_eq!(b[0], Block::Heading(1, "Title".into()));
        assert_eq!(b[1], Block::Paragraph("Hello **world** second line".into()));
        assert_eq!(b[2], Block::Bullet(0, "a".into()));
        assert_eq!(b[3], Block::Bullet(1, "b".into()));
        assert_eq!(b[4], Block::Numbered(0, "one".into()));
        assert!(matches!(&b[5], Block::Table(r) if r.len() == 2));
        assert_eq!(b[6], Block::Code("code".into()));
        assert_eq!(b[7], Block::Rule);
    }

    #[test]
    fn inline_styles() {
        let s = inline_spans("a **b** *c* `d` [e](http://x)");
        assert_eq!(
            s.iter().map(|x| x.text.as_str()).collect::<String>(),
            "a b c d e"
        );
        assert!(s.iter().any(|x| x.bold && x.text == "b"));
        assert!(s.iter().any(|x| x.italic && x.text == "c"));
        assert!(s.iter().any(|x| x.code && x.text == "d"));
    }

    #[test]
    fn docx_has_its_parts() {
        let d = docx("# Hi\n\ntext <&>", "T").unwrap();
        let e = entries(&d);
        assert!(e.contains(&"word/document.xml".to_string()));
        assert!(e.contains(&"[Content_Types].xml".to_string()));
    }

    #[test]
    fn sheets_from_json_and_csv() {
        let s = parse_sheets(
            r#"{"sheets":[{"name":"A","rows":[["x","y"],[1,2.5],["=SUM(A2:B2)",null]]}]}"#,
        )
        .unwrap();
        assert_eq!(s[0].rows.len(), 3);
        let c = parse_sheets("a,b\n\"1,5\",\"say \"\"hi\"\"\"\n").unwrap();
        assert_eq!(c[0].rows[1][0], Value::String("1,5".into()));
        assert_eq!(c[0].rows[1][1], Value::String("say \"hi\"".into()));
        assert!(parse_sheets("{nope").is_err());
        let bytes = xlsx(&s).unwrap();
        assert!(entries(&bytes).contains(&"xl/worksheets/sheet1.xml".to_string()));
        assert_eq!(col_letters(0), "A");
        assert_eq!(col_letters(26), "AA");
    }

    #[test]
    fn slides_from_markdown() {
        let s = parse_slides("# One\n- a\n- b\nNotes: hello\n---\n# Two\ntext");
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].title, "One");
        assert_eq!(s[0].lines, vec!["a", "b"]);
        assert_eq!(s[0].notes, "hello");
        let bytes = pptx(&s, "Deck").unwrap();
        assert!(entries(&bytes).contains(&"ppt/slides/slide2.xml".to_string()));
        assert!(pptx(&[], "x").is_err());
    }

    #[test]
    fn pdf_is_a_pdf() {
        let p = pdf("# Title\n\nSome words (with) parens.\n\n- bullet", "T").unwrap();
        assert!(p.starts_with(b"%PDF-1.4"));
        assert!(p.ends_with(b"%%EOF\n"));
    }

    /// `OPENCUE_OFFICE_OUT=/some/dir cargo test -p opencue-core dump_samples -- --ignored` writes
    /// one of each file so they can be opened in a real office suite.
    #[test]
    #[ignore]
    fn dump_samples() {
        let Some(dir) = std::env::var_os("OPENCUE_OFFICE_OUT") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        let md = "# Quarterly report\n\nThis is **bold**, *italic* and `code` text with (parens).\n\n## List\n\n- first\n  - nested\n- second\n\n1. one\n2. two\n\n| Name | Qty |\n|---|---|\n| Apples | 3 |\n| Pears | 10 |\n\n```\nlet x = 1;\n```\n\n---\n\nThe end.";
        std::fs::write(dir.join("a.docx"), docx(md, "Report").unwrap()).unwrap();
        std::fs::write(dir.join("a.pdf"), pdf(md, "Report").unwrap()).unwrap();
        let sheets = parse_sheets(r#"{"sheets":[{"name":"Sales","rows":[["Item","Qty","Price"],["Apple",3,1.5],["Pear",10,2],["Total","=SUM(B2:B3)","=SUM(C2:C3)"]]},{"name":"Notes","rows":[["hello"]]}]}"#).unwrap();
        std::fs::write(dir.join("a.xlsx"), xlsx(&sheets).unwrap()).unwrap();
        let slides = parse_slides("# Launch plan\n---\n# Goals\n- Ship **fast**\n- Keep quality\n---\n## Next\n- Review\n- Release");
        std::fs::write(dir.join("a.pptx"), pptx(&slides, "Deck").unwrap()).unwrap();
    }
}
