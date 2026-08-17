#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LayoutClass {
    Narrow,
    Compact,
    Wide,
}

impl LayoutClass {
    pub(crate) fn from_width(width: f32) -> Self {
        if width < 640.0 {
            Self::Narrow
        } else if width < 960.0 {
            Self::Compact
        } else {
            Self::Wide
        }
    }

    pub(crate) fn is_narrow(self) -> bool {
        self == Self::Narrow
    }

    pub(crate) fn is_wide(self) -> bool {
        self == Self::Wide
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_content_widths_at_shared_breakpoints() {
        assert_eq!(LayoutClass::from_width(520.0), LayoutClass::Narrow);
        assert_eq!(LayoutClass::from_width(640.0), LayoutClass::Compact);
        assert_eq!(LayoutClass::from_width(959.0), LayoutClass::Compact);
        assert_eq!(LayoutClass::from_width(960.0), LayoutClass::Wide);
    }
}
