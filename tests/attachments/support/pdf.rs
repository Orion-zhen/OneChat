use crate::attachments::*;

pub(crate) fn minimal_pdf() -> Vec<u8> {
    pdf_fixture(1, 0)
}

pub(crate) fn pdf_fixture(page_count: usize, padding_bytes: usize) -> Vec<u8> {
    let kids = (0..page_count)
        .map(|index| format!("{} 0 R", index + 3))
        .collect::<Vec<_>>()
        .join(" ");
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        format!("<< /Type /Pages /Kids [{kids}] /Count {page_count} >>"),
    ];
    for _ in 0..page_count {
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] /Resources << >> /Contents {} 0 R >>",
            page_count + 3
        ));
    }
    objects.push("<< /Length 0 >>\nstream\n\nendstream".into());
    let mut pdf = "%PDF-1.4\n".to_string();
    pdf.push_str(&" ".repeat(padding_bytes));
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        writeln!(&mut pdf, "{} 0 obj\n{object}\nendobj", index + 1).unwrap();
    }
    let xref = pdf.len();
    let size = objects.len() + 1;
    write!(&mut pdf, "xref\n0 {size}\n0000000000 65535 f \n").unwrap();
    for offset in offsets {
        writeln!(&mut pdf, "{offset:010} 00000 n ").unwrap();
    }
    write!(
        &mut pdf,
        "trailer\n<< /Size {size} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n"
    )
    .unwrap();
    pdf.into_bytes()
}
