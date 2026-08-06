#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TableAlignment {
    None,
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug)]
pub struct MarkdownDocument {
    pub blocks: Vec<Block>,
}

#[derive(Clone, Debug)]
pub enum Block {
    Paragraph(Vec<Inline>),
    Heading(u8, Vec<Inline>),
    Quote(Vec<Block>),
    List {
        ordered: bool,
        start: usize,
        items: Vec<Vec<Block>>,
    },
    Code {
        language: String,
        content: String,
    },
    Formula(Formula),
    Table {
        alignments: Vec<TableAlignment>,
        header: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
    },
    Rule,
}

#[derive(Clone, Debug)]
pub enum Inline {
    Text { content: String, style: InlineStyle },
    Formula(Formula),
    Break,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct InlineStyle {
    pub emphasis: bool,
    pub strong: bool,
    pub strike: bool,
    pub code: bool,
    pub link: bool,
}

#[derive(Clone, Debug)]
pub struct Formula {
    pub source: String,
    pub display: bool,
}
