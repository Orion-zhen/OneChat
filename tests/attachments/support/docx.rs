use crate::*;

pub(crate) fn drawing(resource_id: &str, alt: &str) -> String {
    format!(
        r#"<w:p><w:r><w:drawing><wp:inline><wp:docPr descr="{alt}"/><a:graphic><a:graphicData><a:blip r:embed="{resource_id}"/></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"#
    )
}

pub(crate) fn docx(
    body: &str,
    media: Vec<(String, Vec<u8>)>,
    extras: Vec<(String, Vec<u8>)>,
) -> Vec<u8> {
    let relationships = media
        .iter()
        .enumerate()
        .map(|(index, (name, _))| {
            format!(
                r#"<Relationship Id="rId{:03}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/{name}"/>"#,
                index + 1
            )
        })
        .collect::<String>();
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"><w:body>{body}</w:body></w:document>"#
    );
    let document_relationships = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{relationships}</Relationships>"#
    );
    let mut entries = vec![
        (
            "[Content_Types].xml".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Default Extension="jpg" ContentType="image/jpeg"/><Default Extension="svg" ContentType="image/svg+xml"/><Default Extension="bmp" ContentType="image/bmp"/><Default Extension="tiff" ContentType="image/tiff"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#.to_vec(),
        ),
        (
            "_rels/.rels".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#.to_vec(),
        ),
        (
            "word/_rels/document.xml.rels".into(),
            document_relationships.into_bytes(),
        ),
        (
            "word/numbering.xml".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?><w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num></w:numbering>"#.to_vec(),
        ),
        (
            "word/styles.xml".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?><w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="Heading 1"/><w:pPr><w:outlineLvl w:val="0"/></w:pPr></w:style></w:styles>"#.to_vec(),
        ),
        ("word/document.xml".into(), document.into_bytes()),
    ];
    entries.extend(
        media
            .into_iter()
            .map(|(name, bytes)| (format!("word/media/{name}"), bytes)),
    );
    entries.extend(extras);
    zip_entries(entries)
}
