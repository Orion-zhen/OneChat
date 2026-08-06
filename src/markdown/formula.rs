use std::{
    collections::HashMap,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Mutex, OnceLock},
};

use ratex_layout::{LayoutOptions, layout, to_display_list};
use ratex_parser::parse;
use ratex_svg::{SvgColorSyntax, SvgOptions, render_to_svg_with_color_syntax};
use ratex_types::{color::Color, math_style::MathStyle};

const MIN_FORMULA_SCALE: f32 = 2.0;

#[derive(Clone, Debug)]
pub struct FormulaImage {
    pub svg: Vec<u8>,
    pub width: f32,
    pub height: f32,
}

pub fn render_formula_cached(
    source: &str,
    display: bool,
    dark: bool,
    scale_factor: f32,
) -> Result<FormulaImage, String> {
    type FormulaKey = (String, bool, bool, u32);
    static CACHE: OnceLock<Mutex<HashMap<FormulaKey, Result<FormulaImage, String>>>> =
        OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let raster_scale = if scale_factor.is_finite() {
        scale_factor.max(MIN_FORMULA_SCALE)
    } else {
        MIN_FORMULA_SCALE
    };
    let key = (source.to_string(), display, dark, raster_scale.to_bits());
    if let Some(cached) = cache
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get(&key)
    {
        return cached.clone();
    }

    let rendered = catch_unwind(AssertUnwindSafe(|| {
        render_formula_svg(source, display, dark, raster_scale)
    }))
    .map_err(|_| "Formula renderer stopped unexpectedly".to_string())
    .and_then(|result| result);
    cache
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .insert(key, rendered.clone());
    rendered
}

fn render_formula_svg(
    source: &str,
    display: bool,
    dark: bool,
    raster_scale: f32,
) -> Result<FormulaImage, String> {
    if source.is_empty() {
        return Err("Formula is empty".into());
    }
    let ast = parse(source).map_err(|error| error.to_string())?;
    let layout_options = LayoutOptions {
        style: if display {
            MathStyle::Display
        } else {
            MathStyle::Text
        },
        color: if dark { Color::WHITE } else { Color::BLACK },
        ..LayoutOptions::default()
    };
    let layout = layout(&ast, &layout_options);
    let display_list = to_display_list(&layout);
    let font_size = if display { 22.0 } else { 17.0 };
    let padding = if display { 6.0 } else { 2.0 };
    let raster_scale = f64::from(raster_scale);
    let options = SvgOptions {
        font_size: font_size * raster_scale,
        padding: padding * raster_scale,
        stroke_width: raster_scale,
        embed_glyphs: true,
        font_dir: String::new(),
    };
    let svg = render_to_svg_with_color_syntax(&display_list, &options, SvgColorSyntax::Rgb)
        .replace("pt\"", "px\"");
    let width = (display_list.width * font_size + padding * 2.0).max(font_size) as f32;
    let height = ((display_list.height + display_list.depth) * font_size + padding * 2.0)
        .max(font_size) as f32;
    Ok(FormulaImage {
        svg: svg.into_bytes(),
        width,
        height,
    })
}
