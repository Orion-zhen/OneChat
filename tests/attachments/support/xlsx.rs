use crate::attachments::*;

pub(crate) fn xlsx_fixture() -> Vec<u8> {
    let overview = r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
           xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheetData>
    <row r="1">
      <c r="A1" t="inlineStr"><is><t>Group A</t></is></c>
      <c r="C1" t="inlineStr"><is><t>Metadata</t></is></c>
    </row>
    <row r="2">
      <c r="A2" t="inlineStr"><is><t>Name</t></is></c>
      <c r="B2" t="inlineStr"><is><t>Value</t></is></c>
      <c r="D2" t="inlineStr"><is><t>Sparse</t></is></c>
    </row>
    <row r="3">
      <c r="A3" t="inlineStr"><is><t>中文项目</t></is></c>
      <c r="B3"><v>42</v></c>
      <c r="C3" s="1"><v>45292</v></c>
    </row>
    <row r="4">
      <c r="A4" t="inlineStr"><is><t>Website</t></is></c>
      <c r="B4" t="inlineStr"><is><t>OneChat site</t></is></c>
    </row>
    <row r="5">
      <c r="A5" t="inlineStr"><is><t>Cached formula</t></is></c>
      <c r="B5"><f>SUM(B3,1)</f><v>43</v></c>
      <c r="C5" t="inlineStr"><is><t>Needs review</t></is></c>
    </row>
    <row r="6">
      <c r="A6" t="inlineStr"><is><t>In-cell image</t></is></c>
      <c r="B6" t="e" vm="1"><v>#VALUE!</v></c>
    </row>
  </sheetData>
  <mergeCells count="2"><mergeCell ref="A1:B1"/><mergeCell ref="C1:D1"/></mergeCells>
  <hyperlinks><hyperlink ref="B4" r:id="rIdLink"/></hyperlinks>
  <drawing r:id="rIdDrawing"/>
</worksheet>"#
        .to_string();
    let details = r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Name</t></is></c><c r="B1" t="inlineStr"><is><t>Value</t></is></c></row>
    <row r="2"><c r="A2" t="inlineStr"><is><t>Second sheet / 第二页</t></is></c><c r="B2"><v>7</v></c></row>
  </sheetData>
</worksheet>"#
        .to_string();
    let extras = vec![
        (
            "xl/styles.xml".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <cellXfs count="2"><xf numFmtId="0"/><xf numFmtId="14"/></cellXfs>
</styleSheet>"#
                .to_vec(),
        ),
        (
            "xl/worksheets/_rels/sheet1.xml.rels".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdLink" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com" TargetMode="External"/>
  <Relationship Id="rIdComment" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="../comments1.xml"/>
  <Relationship Id="rIdDrawing" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing" Target="../drawings/drawing1.xml"/>
</Relationships>"#
                .to_vec(),
        ),
        (
            "xl/comments1.xml".into(),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<comments xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <authors><author>OneChat</author></authors>
  <commentList><comment ref="C5" authorId="0"><text><t>重要批注</t></text></comment></commentList>
</comments>"#
                .as_bytes()
                .to_vec(),
        ),
        (
            "xl/drawings/drawing1.xml".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing"
          xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
          xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <xdr:twoCellAnchor><xdr:pic>
    <xdr:nvPicPr><xdr:cNvPr id="1" name="Sales chart"/></xdr:nvPicPr>
    <xdr:blipFill><a:blip r:embed="rIdImage"/></xdr:blipFill>
    <xdr:spPr><a:xfrm><a:ext cx="914400" cy="914400"/></a:xfrm></xdr:spPr>
  </xdr:pic><xdr:clientData/></xdr:twoCellAnchor>
</xdr:wsDr>"#
                .to_vec(),
        ),
        (
            "xl/drawings/_rels/drawing1.xml.rels".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdImage" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/chart.png"/>
</Relationships>"#
                .to_vec(),
        ),
        (
            "xl/metadata.xml".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<metadata xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
          xmlns:xlrd="http://schemas.microsoft.com/office/spreadsheetml/2017/richdata">
  <metadataTypes count="1"><metadataType name="XLRICHVALUE"/></metadataTypes>
  <futureMetadata name="XLRICHVALUE" count="1"><bk><extLst><ext><xlrd:rvb i="0"/></ext></extLst></bk></futureMetadata>
  <valueMetadata count="1"><bk><rc t="1" v="0"/></bk></valueMetadata>
</metadata>"#
                .to_vec(),
        ),
        (
            "xl/richData/rdrichvaluestructure.xml".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<rvStructures xmlns="http://schemas.microsoft.com/office/spreadsheetml/2017/richdata" count="1">
  <s t="_localImage"><k n="_rvRel:LocalImageIdentifier" t="i"/></s>
</rvStructures>"#
                .to_vec(),
        ),
        (
            "xl/richData/rdrichvalue.xml".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<rvData xmlns="http://schemas.microsoft.com/office/spreadsheetml/2017/richdata" count="1">
  <rv s="0"><v>0</v></rv>
</rvData>"#
                .to_vec(),
        ),
        (
            "xl/richData/richValueRel.xml".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<richValueRels xmlns="http://schemas.microsoft.com/office/spreadsheetml/2022/richvaluerel"
               xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <rel r:id="rIdCellImage"/>
</richValueRels>"#
                .to_vec(),
        ),
        (
            "xl/richData/_rels/richValueRel.xml.rels".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdCellImage" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/cell.png"/>
</Relationships>"#
                .to_vec(),
        ),
        ("xl/media/cell.png".into(), png_bytes()),
        ("xl/media/chart.png".into(), png_bytes()),
    ];
    let rich_value_relationships = r#"
  <Relationship Id="rIdMetadata" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sheetMetadata" Target="metadata.xml"/>
  <Relationship Id="rIdRichValue" Type="http://schemas.microsoft.com/office/2017/06/relationships/rdRichValue" Target="richData/rdrichvalue.xml"/>
  <Relationship Id="rIdRichStructure" Type="http://schemas.microsoft.com/office/2017/06/relationships/rdRichValueStructure" Target="richData/rdrichvaluestructure.xml"/>
  <Relationship Id="rIdRichRel" Type="http://schemas.microsoft.com/office/2022/10/relationships/richValueRel" Target="richData/richValueRel.xml"/>"#;

    xlsx(
        &[("概览", overview), ("Data Sheet", details)],
        rich_value_relationships,
        extras,
    )
}

pub(crate) fn xlsx(
    sheets: &[(&str, String)],
    extra_workbook_relationships: &str,
    extras: Vec<(String, Vec<u8>)>,
) -> Vec<u8> {
    let sheet_types = (1..=sheets.len())
        .map(|index| {
            format!(
                r#"<Override PartName="/xl/worksheets/sheet{index}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#
            )
        })
        .collect::<String>();
    let content_types = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  {sheet_types}
</Types>"#
    );
    let workbook_sheets = sheets
        .iter()
        .enumerate()
        .map(|(index, (name, _))| {
            let id = index + 1;
            format!(r#"<sheet name="{name}" sheetId="{id}" r:id="rId{id}"/>"#)
        })
        .collect::<String>();
    let workbook = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
          xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>{workbook_sheets}</sheets>
</workbook>"#
    );
    let workbook_relationships = sheets
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let id = index + 1;
            format!(
                r#"<Relationship Id="rId{id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{id}.xml"/>"#
            )
        })
        .collect::<String>();
    let workbook_relationships = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  {workbook_relationships}
  {extra_workbook_relationships}
</Relationships>"#
    );

    let mut entries = vec![
        ("[Content_Types].xml".into(), content_types.into_bytes()),
        (
            "_rels/.rels".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#
                .to_vec(),
        ),
        ("xl/workbook.xml".into(), workbook.into_bytes()),
        (
            "xl/_rels/workbook.xml.rels".into(),
            workbook_relationships.into_bytes(),
        ),
    ];
    entries.extend(sheets.iter().enumerate().map(|(index, (_, xml))| {
        (
            format!("xl/worksheets/sheet{}.xml", index + 1),
            xml.as_bytes().to_vec(),
        )
    }));
    entries.extend(extras);
    zip_entries(entries)
}
