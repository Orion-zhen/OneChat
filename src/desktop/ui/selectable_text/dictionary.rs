use objc2::{AnyThread as _, MainThreadMarker, rc::Retained};
use objc2_app_kit::{
    NSFont, NSFontAttributeName, NSFontManager, NSFontTraitMask, NSFontWeightBlack,
    NSFontWeightBold, NSFontWeightHeavy, NSFontWeightLight, NSFontWeightMedium,
    NSFontWeightRegular, NSFontWeightSemibold, NSFontWeightThin, NSView,
};
use objc2_foundation::{NSAttributedString, NSDictionary, NSPoint, NSString};
use objc2_natural_language::{NLTokenUnit, NLTokenizer};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use super::geometry::nearest_index;
use super::*;

pub(super) fn show_at(selection: &TextSelection, position: Point<Pixels>, window: &Window) -> bool {
    let registry = selection.registry.borrow();
    let runtime = registry.groups.values().find_map(|entry| {
        let runtime = entry.runtime.borrow();
        runtime
            .regions
            .iter()
            .any(|region| region.bounds.contains(&position))
            .then(|| entry.runtime.clone())
    });
    drop(registry);
    let Some(runtime) = runtime else {
        return false;
    };
    let runtime = runtime.borrow();
    let Some(region) = runtime
        .regions
        .iter()
        .find(|region| region.bounds.contains(&position))
    else {
        return false;
    };
    let source = region.source.clone();
    let source_offset = (region.source_range.start + nearest_index(&region.layout, position))
        .min(region.source_range.end);
    let Some(word_range) = native_word_range(&source, source_offset) else {
        return false;
    };
    let word = &source[word_range.clone()];
    let word_region = runtime
        .regions
        .iter()
        .filter(|candidate| candidate.source == source)
        .find(|candidate| region_contains_offset(candidate, word_range.start))
        .unwrap_or(region);
    let origin = baseline_for_offset(word_region, word_range.start)
        .or_else(|| baseline_for_offset(region, source_offset))
        .unwrap_or(position);

    let view = native_view(window);
    let view_height = f32::from(window.viewport_size().height) as f64;
    let origin = appkit_point(origin, view_height, view.isFlipped());
    let attributed_string = attributed_word(word, word_region, word_range.start, window);
    view.showDefinitionForAttributedString_atPoint(Some(&attributed_string), origin);
    true
}

fn native_view(window: &Window) -> &NSView {
    let window_handle =
        HasWindowHandle::window_handle(window).expect("macOS window must expose an AppKit handle");
    let RawWindowHandle::AppKit(handle) = window_handle.as_raw() else {
        unreachable!("macOS window must use an AppKit handle");
    };
    // SAFETY: GPUI documents the AppKit raw handle as its live backing NSView.
    unsafe { handle.ns_view.cast::<NSView>().as_ref() }
}

fn native_word_range(source: &str, source_offset: usize) -> Option<Range<usize>> {
    if source.is_empty() {
        return None;
    }
    let string = NSString::from_str(source);
    let tokenizer = unsafe { NLTokenizer::initWithUnit(NLTokenizer::alloc(), NLTokenUnit::Word) };
    unsafe {
        tokenizer.setString(Some(&string));
    }
    let utf16_offset = utf16_offset_for_utf8(source, source_offset).min(string.len_utf16() - 1);
    let range = unsafe { tokenizer.tokenRangeAtIndex(utf16_offset) };
    let utf16_end = range.location.checked_add(range.length)?;
    if range.length == 0 || utf16_end > string.len_utf16() {
        return None;
    }
    let start = utf8_offset_for_utf16(source, range.location);
    let end = utf8_offset_for_utf16(source, utf16_end);
    (start < end && !source[start..end].trim().is_empty()).then_some(start..end)
}

fn region_contains_offset(region: &TextRegion, source_offset: usize) -> bool {
    source_offset >= region.source_range.start && source_offset <= region.source_range.end
}

fn baseline_for_offset(region: &TextRegion, source_offset: usize) -> Option<Point<Pixels>> {
    if !region_contains_offset(region, source_offset) {
        return None;
    }
    let local_offset = source_offset - region.source_range.start;
    let line = region.layout.line_layout_for_index(local_offset)?;
    let mut origin = region.layout.position_for_index(local_offset)?;
    origin.y +=
        (region.layout.line_height() - line.ascent() - line.descent()) / 2.0 + line.ascent();
    Some(origin)
}

fn attributed_word(
    word: &str,
    region: &TextRegion,
    source_offset: usize,
    window: &Window,
) -> Retained<NSAttributedString> {
    let string = NSString::from_str(word);
    let Some(font) = native_font(word, region, source_offset, window) else {
        return NSAttributedString::from_nsstring(&string);
    };
    // SAFETY: AppKit exports NSFontAttributeName for the lifetime of the process.
    let font_attribute = unsafe { NSFontAttributeName };
    let attributes = NSDictionary::from_slices(
        &[font_attribute],
        &[font.as_ref() as &objc2::runtime::AnyObject],
    );
    // SAFETY: NSFontAttributeName requires an NSFont value, which the dictionary contains.
    unsafe {
        NSAttributedString::initWithString_attributes(
            NSAttributedString::alloc(),
            &string,
            Some(&attributes),
        )
    }
}

fn native_font(
    word: &str,
    region: &TextRegion,
    source_offset: usize,
    window: &Window,
) -> Option<Retained<NSFont>> {
    let local_offset = source_offset.checked_sub(region.source_range.start)?;
    let (line, line_offset) = line_for_offset(&region.layout, local_offset)?;
    let size = f32::from(line.font_size()) as f64;
    let manager = NSFontManager::sharedFontManager(MainThreadMarker::new()?);

    if let Some(font) = line
        .unwrapped_layout
        .font_id_for_index(line_offset)
        .and_then(|font_id| window.text_system().get_font_for_id(font_id))
        && let Some(native) = native_font_for_family(&font, &font.family, size, &manager)
    {
        return Some(native);
    }

    Some(native_font_from_stack(&region.font, word, size, &manager))
}

fn native_font_from_stack(
    font: &Font,
    word: &str,
    size: f64,
    manager: &NSFontManager,
) -> Retained<NSFont> {
    let mut families = vec![font.family.to_string()];
    if let Some(fallbacks) = &font.fallbacks {
        families.extend(fallbacks.fallback_list().iter().cloned());
    }

    let mut first = None;
    for family in families {
        let Some(native) = native_font_for_family(font, &family, size, manager) else {
            continue;
        };
        if first.is_none() {
            first = Some(native.clone());
        }
        if font_covers_word(&native, word) {
            return native;
        }
    }

    first.unwrap_or_else(|| NSFont::systemFontOfSize_weight(size, native_font_weight(font.weight)))
}

fn native_font_for_family(
    font: &Font,
    family: &str,
    size: f64,
    manager: &NSFontManager,
) -> Option<Retained<NSFont>> {
    let traits = if font.style == gpui::FontStyle::Normal {
        NSFontTraitMask::empty()
    } else {
        NSFontTraitMask::ItalicFontMask
    };
    if family == ".SystemUIFont" {
        let native = NSFont::systemFontOfSize_weight(size, native_font_weight(font.weight));
        return Some(if traits.is_empty() {
            native
        } else {
            manager.convertFont_toHaveTrait(&native, traits)
        });
    }

    let family = NSString::from_str(family);
    manager.fontWithFamily_traits_weight_size(
        &family,
        traits,
        native_font_manager_weight(font.weight),
        size,
    )
}

fn font_covers_word(font: &NSFont, word: &str) -> bool {
    let characters = font.coveredCharacterSet();
    word.chars()
        .filter(|character| !character.is_whitespace())
        .all(|character| characters.longCharacterIsMember(character as u32))
}

fn line_for_offset(
    layout: &gpui::TextLayout,
    offset: usize,
) -> Option<(std::sync::Arc<gpui::WrappedLineLayout>, usize)> {
    let mut line_start = 0;
    for line in layout.line_layouts() {
        let line_end = line_start + line.len();
        if offset <= line_end {
            return Some((line, offset - line_start));
        }
        line_start = line_end + 1;
    }
    None
}

fn native_font_manager_weight(weight: FontWeight) -> isize {
    if weight <= FontWeight::THIN {
        2
    } else if weight <= FontWeight::EXTRA_LIGHT {
        3
    } else if weight <= FontWeight::LIGHT {
        4
    } else if weight < FontWeight::MEDIUM {
        5
    } else if weight < FontWeight::SEMIBOLD {
        6
    } else if weight < FontWeight::BOLD {
        8
    } else if weight < FontWeight::EXTRA_BOLD {
        9
    } else if weight < FontWeight::BLACK {
        10
    } else {
        12
    }
}

fn native_font_weight(weight: FontWeight) -> f64 {
    if weight <= FontWeight::THIN {
        unsafe { NSFontWeightThin }
    } else if weight <= FontWeight::LIGHT {
        unsafe { NSFontWeightLight }
    } else if weight < FontWeight::MEDIUM {
        unsafe { NSFontWeightRegular }
    } else if weight < FontWeight::SEMIBOLD {
        unsafe { NSFontWeightMedium }
    } else if weight < FontWeight::BOLD {
        unsafe { NSFontWeightSemibold }
    } else if weight < FontWeight::EXTRA_BOLD {
        unsafe { NSFontWeightBold }
    } else if weight < FontWeight::BLACK {
        unsafe { NSFontWeightHeavy }
    } else {
        unsafe { NSFontWeightBlack }
    }
}

fn appkit_point(point: Point<Pixels>, view_height: f64, view_is_flipped: bool) -> NSPoint {
    let y = f32::from(point.y) as f64;
    NSPoint::new(
        f32::from(point.x) as f64,
        if view_is_flipped { y } else { view_height - y },
    )
}

fn utf16_offset_for_utf8(text: &str, utf8_offset: usize) -> usize {
    text[..utf8_offset.min(text.len())].encode_utf16().count()
}

fn utf8_offset_for_utf16(text: &str, utf16_offset: usize) -> usize {
    let mut utf16_index = 0;
    for (utf8_index, character) in text.char_indices() {
        if utf16_index >= utf16_offset {
            return utf8_index;
        }
        let next = utf16_index + character.len_utf16();
        if utf16_offset < next {
            return utf8_index;
        }
        utf16_index = next;
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::{native_word_range, utf8_offset_for_utf16, utf16_offset_for_utf8};

    #[test]
    fn native_tokenizer_selects_word_at_position() {
        let text = "Open the dictionary overlay";
        let range = native_word_range(text, 12).unwrap();
        assert_eq!(&text[range], "dictionary");
    }

    #[test]
    fn converts_offsets_between_utf8_and_utf16() {
        let text = "A词😀é";
        for offset in text
            .char_indices()
            .map(|(offset, _)| offset)
            .chain([text.len()])
        {
            let utf16 = utf16_offset_for_utf8(text, offset);
            assert_eq!(utf8_offset_for_utf16(text, utf16), offset);
        }
    }

    #[test]
    fn utf16_offset_inside_surrogate_pair_uses_character_start() {
        assert_eq!(utf8_offset_for_utf16("a😀b", 2), 1);
    }
}
