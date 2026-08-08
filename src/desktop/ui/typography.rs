use crate::domain::{MAX_MESSAGE_FONT_SIZE, MIN_MESSAGE_FONT_SIZE};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MessageTypography {
    pub(crate) body_size: f32,
    pub(crate) body_line_height: f32,
    pub(crate) secondary_size: f32,
    pub(crate) secondary_line_height: f32,
    pub(crate) code_size: f32,
    pub(crate) code_line_height: f32,
    pub(crate) metadata_size: f32,
    pub(crate) metadata_line_height: f32,
    pub(crate) micro_size: f32,
    pub(crate) micro_line_height: f32,
}

impl MessageTypography {
    pub(crate) fn new(body_size: f32) -> Self {
        let body_size = body_size.clamp(MIN_MESSAGE_FONT_SIZE, MAX_MESSAGE_FONT_SIZE);
        let secondary_size = body_size - 1.0;
        let code_size = body_size - 1.0;
        let metadata_size = (body_size - 3.0).max(11.0);
        let micro_size = (body_size - 4.0).max(10.0);
        Self {
            body_size,
            body_line_height: body_size + 9.0,
            secondary_size,
            secondary_line_height: secondary_size + 8.0,
            code_size,
            code_line_height: code_size + 8.0,
            metadata_size,
            metadata_line_height: metadata_size + 6.0,
            micro_size,
            micro_line_height: micro_size + 5.0,
        }
    }

    pub(crate) fn heading_size(self, level: u8) -> f32 {
        self.body_size
            + match level {
                1 => 9.0,
                2 => 6.0,
                3 => 3.0,
                _ => 1.0,
            }
    }

    pub(crate) fn heading_line_height(self, level: u8) -> f32 {
        self.heading_size(level) + 6.0
    }

    pub(crate) fn table_line_height(self) -> f32 {
        self.body_size + 6.0
    }
}
