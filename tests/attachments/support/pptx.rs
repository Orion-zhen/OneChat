use crate::attachments::*;

pub(crate) fn pptx_fixture() -> Vec<u8> {
    let slide1 = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
       xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
       xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart">
  <p:cSld><p:spTree>
    <p:sp>
      <p:nvSpPr><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr>
      <p:txBody><a:p><a:r><a:t>Quarterly Review / 季度回顾</a:t></a:r></a:p></p:txBody>
    </p:sp>
    <p:sp>
      <p:txBody>
        <a:p><a:r><a:t>English and 中文 presentation text.</a:t></a:r></a:p>
        <a:p><a:pPr><a:buChar char="•"/></a:pPr><a:r><a:t>First bullet</a:t></a:r></a:p>
        <a:p><a:pPr><a:buChar char="•"/></a:pPr><a:r><a:t>Second bullet</a:t></a:r></a:p>
        <a:p><a:r><a:rPr><a:hlinkClick r:id="rIdLink"/></a:rPr><a:t>Project site</a:t></a:r></a:p>
      </p:txBody>
    </p:sp>
    <p:sp><p:nvSpPr><p:nvPr><p:ph idx="7"/></p:nvPr></p:nvSpPr></p:sp>
    <p:graphicFrame><a:graphic><a:graphicData><a:tbl>
      <a:tr><a:tc><a:txBody><a:p><a:r><a:t>Metric</a:t></a:r></a:p></a:txBody></a:tc><a:tc><a:txBody><a:p><a:r><a:t>Value</a:t></a:r></a:p></a:txBody></a:tc></a:tr>
      <a:tr><a:tc><a:txBody><a:p><a:r><a:t>Revenue</a:t></a:r></a:p></a:txBody></a:tc><a:tc><a:txBody><a:p><a:r><a:t>42</a:t></a:r></a:p></a:txBody></a:tc></a:tr>
    </a:tbl></a:graphicData></a:graphic></p:graphicFrame>
    <p:graphicFrame><a:graphic><a:graphicData><c:chart r:id="rIdChart"/></a:graphicData></a:graphic></p:graphicFrame>
    <p:pic>
      <p:nvPicPr><p:cNvPr id="1" name="Product screenshot"/></p:nvPicPr>
      <p:blipFill><a:blip r:embed="rIdImage"/></p:blipFill>
      <p:spPr><a:xfrm><a:ext cx="914400" cy="914400"/></a:xfrm></p:spPr>
    </p:pic>
  </p:spTree></p:cSld>
</p:sld>"#
        .to_string();
    let slide2 = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
       xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>Second slide / 第二页</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld>
</p:sld>"#
        .to_string();
    let slide1_relationships = r#"
  <Relationship Id="rIdLink" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/ppt" TargetMode="External"/>
  <Relationship Id="rIdChart" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart" Target="../charts/chart1.xml"/>
  <Relationship Id="rIdImage" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/screenshot.png"/>
  <Relationship Id="rIdLayout" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>"#
        .to_string();
    let notes = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:notes xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
         xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>Speaker notes: verify revenue assumptions.</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld>
</p:notes>"#;
    let layout = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sldLayout xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
             xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <p:cSld><p:spTree><p:sp>
    <p:nvSpPr><p:nvPr><p:ph idx="7"/></p:nvPr></p:nvSpPr>
    <p:txBody><a:p><a:r><a:t>Inherited layout text</a:t></a:r></a:p></p:txBody>
  </p:sp></p:spTree></p:cSld>
</p:sldLayout>"#;
    let chart = r#"<?xml version="1.0" encoding="UTF-8"?>
<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"
              xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <c:chart>
    <c:title><c:tx><c:rich><a:p><a:r><a:t>Revenue Growth</a:t></a:r></a:p></c:rich></c:tx></c:title>
    <c:plotArea><c:barChart>
      <c:ser>
        <c:tx><c:strRef><c:strCache><c:pt idx="0"><c:v>2024</c:v></c:pt></c:strCache></c:strRef></c:tx>
        <c:cat><c:strRef><c:strCache><c:pt idx="0"><c:v>Q1</c:v></c:pt><c:pt idx="1"><c:v>Q2</c:v></c:pt></c:strCache></c:strRef></c:cat>
        <c:val><c:numRef><c:numCache><c:pt idx="0"><c:v>100</c:v></c:pt><c:pt idx="1"><c:v>150</c:v></c:pt></c:numCache></c:numRef></c:val>
      </c:ser>
      <c:ser>
        <c:tx><c:strRef><c:strCache><c:pt idx="0"><c:v>2025</c:v></c:pt></c:strCache></c:strRef></c:tx>
        <c:cat><c:strRef><c:strCache><c:pt idx="0"><c:v>Q1</c:v></c:pt><c:pt idx="1"><c:v>Q2</c:v></c:pt></c:strCache></c:strRef></c:cat>
        <c:val><c:numRef><c:numCache><c:pt idx="0"><c:v>120</c:v></c:pt><c:pt idx="1"><c:v>180</c:v></c:pt></c:numCache></c:numRef></c:val>
      </c:ser>
    </c:barChart></c:plotArea>
  </c:chart>
</c:chartSpace>"#;
    let extras = vec![
        (
            "ppt/notesSlides/notesSlide1.xml".into(),
            notes.as_bytes().to_vec(),
        ),
        (
            "ppt/slideLayouts/slideLayout1.xml".into(),
            layout.as_bytes().to_vec(),
        ),
        ("ppt/charts/chart1.xml".into(), chart.as_bytes().to_vec()),
        ("ppt/media/screenshot.png".into(), png_bytes()),
    ];

    pptx(
        &[slide1, slide2],
        vec![slide1_relationships, String::new()],
        extras,
    )
}

pub(crate) fn pptx(
    slides: &[String],
    slide_relationships: Vec<String>,
    extras: Vec<(String, Vec<u8>)>,
) -> Vec<u8> {
    assert_eq!(slides.len(), slide_relationships.len());
    let slide_types = (1..=slides.len())
        .map(|index| {
            format!(
                r#"<Override PartName="/ppt/slides/slide{index}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#
            )
        })
        .collect::<String>();
    let content_types = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  {slide_types}
</Types>"#
    );
    let slide_ids = (1..=slides.len())
        .map(|index| format!(r#"<p:sldId id="{}" r:id="rId{index}"/>"#, 255 + index))
        .collect::<String>();
    let presentation = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
                xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:sldIdLst>{slide_ids}</p:sldIdLst>
</p:presentation>"#
    );
    let presentation_relationships = (1..=slides.len())
        .map(|index| {
            format!(
                r#"<Relationship Id="rId{index}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{index}.xml"/>"#
            )
        })
        .collect::<String>();
    let presentation_relationships = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  {presentation_relationships}
</Relationships>"#
    );

    let mut entries = vec![
        ("[Content_Types].xml".into(), content_types.into_bytes()),
        (
            "_rels/.rels".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#
                .to_vec(),
        ),
        ("ppt/presentation.xml".into(), presentation.into_bytes()),
        (
            "ppt/_rels/presentation.xml.rels".into(),
            presentation_relationships.into_bytes(),
        ),
    ];
    for (index, (slide, relationships)) in slides.iter().zip(slide_relationships).enumerate() {
        let number = index + 1;
        entries.push((
            format!("ppt/slides/slide{number}.xml"),
            slide.as_bytes().to_vec(),
        ));
        entries.push((
            format!("ppt/slides/_rels/slide{number}.xml.rels"),
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  {relationships}
</Relationships>"#
            )
            .into_bytes(),
        ));
    }
    entries.extend(extras);
    zip_entries(entries)
}
