//! File › New: Photoshop's New Document dialog. Category tabs (Recent, Saved, then the
//! blank-document categories) over a well of preset cards on the left, Preset Details on the
//! right. Values live in the dialog fields (`width`/`height` in pixels, `resolution` in ppi,
//! `mode`, `depth`, `background`, `name`, and `artboards`/`profile` only when set), so
//! `ui.dialog.set` drives it and Create runs `file.new` with them. `__advanced` opens Advanced
//! Options; `__pixelAspect` picks the view's Pixel Aspect Ratio, applied on Create.
//!
//! The dialog draws in its own palette ([`crate::theme::NewDocPalette`]); every glyph on it is
//! painted here from lines and shapes.

use egui::{Align2, Rect, RichText, Sense, Stroke, StrokeKind, pos2, vec2};
use serde_json::{Map, Value, json};

use photocraft_engine::prefs::UnitsAndRulers;

use crate::theme::Tokens;
use crate::{icons, widgets};
use photocraft_engine::document_preset_cmds::DocumentSettings;

/// A blank-document preset: (name, width px, height px, ppi).
pub type Preset = (&'static str, u32, u32, f32);

/// Photoshop's New Document categories and their blank-document presets.
pub const CATEGORIES: &[(&str, &[Preset])] = &[
    ("Recent", &[("Default PhotoCraft Size", 2100, 1500, 300.0), ("HDTV 1080p", 1920, 1080, 72.0)]),
    (
        "Photo",
        &[
            ("Landscape, 6 x 4", 1800, 1200, 300.0),
            ("Landscape, 7 x 5", 2100, 1500, 300.0),
            ("Landscape, 10 x 8", 3000, 2400, 300.0),
            ("Portrait, 4 x 6", 1200, 1800, 300.0),
            ("Portrait, 5 x 7", 1500, 2100, 300.0),
            ("Square, 5 x 5", 1500, 1500, 300.0),
        ],
    ),
    (
        "Print",
        &[
            ("Letter", 2550, 3300, 300.0),
            ("Legal", 2550, 4200, 300.0),
            ("Tabloid", 3300, 5100, 300.0),
            ("A4", 2480, 3508, 300.0),
            ("A3", 3508, 4961, 300.0),
            ("A5", 1748, 2480, 300.0),
            ("B3", 4169, 5906, 300.0),
            ("B4", 2953, 4169, 300.0),
        ],
    ),
    (
        "Art & Illustration",
        &[("Poster", 5400, 7200, 300.0), ("Postcard", 1800, 1200, 300.0), ("Comic Book", 1988, 3075, 300.0), ("Square, 12 x 12", 3600, 3600, 300.0)],
    ),
    (
        "Web",
        &[
            ("Web Most Common", 1366, 768, 72.0),
            ("Web Minimum", 1024, 768, 72.0),
            ("Web Large", 1920, 1080, 72.0),
            ("MacBook Pro 16\"", 3456, 2234, 72.0),
            ("iMac 24\"", 4480, 2520, 72.0),
        ],
    ),
    (
        "Mobile",
        &[
            ("iPhone 16", 1179, 2556, 72.0),
            ("iPhone 16 Pro Max", 1320, 2868, 72.0),
            ("iPad Pro 13\"", 2064, 2752, 72.0),
            ("Android 1080p", 1080, 1920, 72.0),
            ("Apple Watch 45mm", 396, 484, 72.0),
        ],
    ),
    (
        "Film & Video",
        &[
            ("HDTV 1080p", 1920, 1080, 72.0),
            ("HDTV 720p", 1280, 720, 72.0),
            ("UHD 4K", 3840, 2160, 72.0),
            ("DCI 4K", 4096, 2160, 72.0),
            ("UHD 8K", 7680, 4320, 72.0),
        ],
    ),
];

const DEPTH_OPTIONS: &[(u64, &str, &str)] = &[(8, "8 bit", "Integer"), (16, "16 bit", "Integer"), (32, "32 bit (float)", "Floating point")];

/// Width/Height units: (key, label, units per inch; 0 = pixels).
pub const UNITS: &[(&str, &str, f32)] =
    &[("px", "Pixels", 0.0), ("in", "Inches", 1.0), ("cm", "Centimeters", 2.54), ("mm", "Millimeters", 25.4), ("pt", "Points", 72.0), ("pica", "Picas", 6.0)];

/// Pixels to the display unit at `ppi`.
pub fn to_unit(px: f32, unit: &str, ppi: f32) -> f32 {
    match UNITS.iter().find(|u| u.0 == unit) {
        Some((_, _, per_in)) if *per_in > 0.0 => px / ppi.max(1.0) * per_in,
        _ => px,
    }
}

/// Display unit back to pixels at `ppi`.
pub fn from_unit(v: f32, unit: &str, ppi: f32) -> f32 {
    match UNITS.iter().find(|u| u.0 == unit) {
        Some((_, _, per_in)) if *per_in > 0.0 => (v / per_in * ppi.max(1.0)).round(),
        _ => v.round(),
    }
}

/// A typed size as the whole pixel count `file.new` takes (#254: a float like `512.0` isn't one, so
/// the command fell back to its 1920 x 1080 default). Clamped to the command's 1–300000 range.
pub fn px_value(px: f32) -> Value {
    json!(if px.is_finite() { px.round().clamp(1.0, 300_000.0) as u32 } else { 1 })
}

/// Name of the preset that takes the clipboard image's size.
pub const CLIPBOARD: &str = "Clipboard";

/// Offer the Clipboard preset (`w` × `h` px, at 72 ppi) first under the Recent presets, and
/// select it.
pub fn set_clipboard(f: &mut Map<String, Value>, w: u32, h: u32) {
    if w == 0 || h == 0 {
        return;
    }
    f.insert("__clipboard".into(), json!([w, h]));
    apply_preset(f, &(CLIPBOARD, w, h, 72.0), &UnitsAndRulers::default());
}

/// The clipboard image's size, when the dialog offers the Clipboard preset.
fn clipboard_preset(f: &Map<String, Value>) -> Option<Preset> {
    let size = f.get("__clipboard")?.as_array()?;
    let dim = |i: usize| size.get(i)?.as_u64().and_then(|v| u32::try_from(v).ok()).filter(|v| *v > 0);
    Some((CLIPBOARD, dim(0)?, dim(1)?, 72.0))
}

/// Set Width or Height (`key`) from a value typed in `unit`: whole pixels for `file.new`, plus the
/// typed value so the field keeps showing it (see [`shown_size`]).
pub fn set_size(f: &mut Map<String, Value>, key: &str, v: f32, unit: &str, ppi: f32) {
    f.insert(key.into(), px_value(from_unit(v, unit, ppi)));
    f.insert(typed_key(key), json!({"value": v, "unit": unit}));
    f.remove("__preset");
    f.remove("__savedPreset");
}

/// What Width or Height (`key`, `d` pixels when unset) shows in `unit`: the value the user typed
/// while it still gives the field's pixel count, else the pixels converted. Converting the rounded
/// pixels back on every keystroke turned a typed `5` mm into 14 px and the text into `4.9` (#1147).
/// A size over the 300000 px limit shows the limit, not the typed value the document won't have.
pub fn shown_size(f: &Map<String, Value>, key: &str, d: f32, unit: &str, ppi: f32) -> f32 {
    let px = get_f(f, key, d);
    let typed = f.get(&typed_key(key)).and_then(Value::as_object).filter(|t| unit != "px" && t.get("unit").and_then(Value::as_str) == Some(unit));
    match typed.and_then(|t| t.get("value")).and_then(Value::as_f64) {
        Some(v) if from_unit(v as f32, unit, ppi).max(1.0) == px => v as f32,
        _ => to_unit(px, unit, ppi),
    }
}

fn typed_key(key: &str) -> String {
    format!("__{key}Typed")
}

/// Swap Width and Height (the Orientation buttons), with the values typed for them, so a typed
/// 841 x 1189 mm turns into 1189 x 841 mm, not 1188.9 x 841.
pub fn swap_size(f: &mut Map<String, Value>) {
    f.remove("__savedPreset");
    let (w, h) = (get_f(f, "width", 1920.0), get_f(f, "height", 1080.0));
    f.insert("width".into(), px_value(h));
    f.insert("height".into(), px_value(w));
    let (typed_w, typed_h) = (f.remove(&typed_key("width")), f.remove(&typed_key("height")));
    if let Some(t) = typed_h {
        f.insert(typed_key("width"), t);
    }
    if let Some(t) = typed_w {
        f.insert(typed_key("height"), t);
    }
}

/// Print and photo presets (listed at 300 ppi) are specified in inches, screen presets in pixels.
fn is_print(p: &Preset) -> bool {
    p.3 >= 300.0
}

/// A built-in preset turned to the orientation picked with the Orientation buttons (#4668), so the
/// cards and what they apply follow it. Square presets and the Clipboard preset stay as they are.
fn oriented(p: &Preset, f: &Map<String, Value>) -> Preset {
    let portrait = match f.get("__orientation").and_then(Value::as_str) {
        Some("portrait") => true,
        Some("landscape") => false,
        _ => return *p,
    };
    if p.0 == CLIPBOARD || p.1 == p.2 || (p.2 > p.1) == portrait {
        return *p;
    }
    (p.0, p.2, p.1, p.3)
}

/// The unit a built-in preset shows and applies: a print preset in the physical unit picked for
/// Width/Height (#4668), else in inches; a screen preset in pixels.
fn preset_unit(p: &Preset, f: &Map<String, Value>) -> String {
    let unit = get_s(f, "__unit", "px");
    if !is_print(p) {
        "px".into()
    } else if unit != "px" && UNITS.iter().any(|u| u.0 == unit) {
        unit
    } else {
        "in".into()
    }
}

/// A built-in preset at Preferences › Units & Rulers › New Document Preset Resolutions, as in
/// Photoshop: a print preset keeps its size in inches at the Print Resolution (Letter at 200 ppi is
/// 1700 x 2200 px), a screen preset keeps its pixels at the Screen Resolution. The Clipboard preset
/// is the copied pixels and keeps its own resolution. A resolution that isn't a positive finite
/// number leaves the preset as listed.
pub fn at_pref_resolutions(p: &Preset, units: &UnitsAndRulers) -> Preset {
    let ppi = (if is_print(p) { units.print_resolution } else { units.screen_resolution }) as f32;
    if p.0 == CLIPBOARD || !(ppi.is_finite() && ppi > 0.0) {
        return *p;
    }
    if !is_print(p) {
        return (p.0, p.1, p.2, ppi);
    }
    let scale = ppi / p.3;
    let px = |v: u32| (v as f32 * scale).round().clamp(1.0, 300_000.0) as u32;
    (p.0, px(p.1), px(p.2), ppi)
}

/// Apply a built-in preset (at the preference resolutions, see [`at_pref_resolutions`]) to the
/// dialog fields.
pub fn apply_preset(f: &mut Map<String, Value>, p: &Preset, units: &UnitsAndRulers) {
    let (name, w, h, ppi) = at_pref_resolutions(p, units);
    let unit = preset_unit(p, f);
    f.remove("__savedPreset");
    f.insert("width".into(), json!(w));
    f.insert("height".into(), json!(h));
    f.insert("resolution".into(), json!(ppi));
    f.insert("__preset".into(), json!(name));
    f.insert("__unit".into(), json!(unit));
}

/// Fields `file.new` takes (drops the dialog's `__` UI keys).
pub fn command_params(f: &Map<String, Value>) -> Value {
    Value::Object(f.iter().filter(|(k, _)| !k.starts_with("__")).map(|(k, v)| (k.clone(), v.clone())).collect())
}

/// Set the resolution (pixels/inch) the way Photoshop's New Document does (#758): with Width/Height
/// in a physical unit the physical size is kept and the pixel count changes; in pixels the pixels
/// are kept.
pub fn set_resolution(f: &mut Map<String, Value>, new_ppi: f32) {
    let old_ppi = get_f(f, "resolution", 72.0);
    f.insert("resolution".into(), json!(new_ppi));
    if old_ppi != new_ppi {
        f.remove("__savedPreset");
    }
    if get_s(f, "__unit", "px") == "px" || !(old_ppi > 0.0 && new_ppi > 0.0) || old_ppi == new_ppi {
        return;
    }
    let scale = new_ppi / old_ppi;
    let (w, h) = (get_f(f, "width", 1920.0), get_f(f, "height", 1080.0));
    f.insert("width".into(), px_value(w * scale));
    f.insert("height".into(), px_value(h * scale));
    f.remove("__preset");
    f.remove("__savedPreset");
}

fn get_f(f: &Map<String, Value>, k: &str, d: f32) -> f32 {
    f.get(k).and_then(Value::as_f64).map_or(d, |v| v as f32)
}

fn get_s(f: &Map<String, Value>, k: &str, d: &str) -> String {
    f.get(k).and_then(Value::as_str).unwrap_or(d).to_string()
}

fn small_label(ui: &mut egui::Ui, s: &str) {
    let t = Tokens::get(ui.ctx());
    ui.label(RichText::new(s).size(11.5).color(t.text_dim));
}

/// Lay out a preset card's title centred in `width`: wrapped onto at most two lines, the rest
/// elided, so long translations stay inside the card.
fn card_title(painter: &egui::Painter, title: &str, width: f32, color: egui::Color32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple(title.to_owned(), egui::FontId::proportional(12.0), color, width);
    job.wrap.max_rows = 2;
    job.halign = egui::Align::Center;
    painter.layout_job(job)
}

/// The Background Contents menu's entry for a colour of the user's own ([`body`]).
const CUSTOM: &str = "custom";
/// Side of the Background Contents colour swatch.
const SWATCH: f32 = 22.0;

/// The colour the new document's background starts with: white, black, the captured or current
/// toolbox background colour, or a custom `"#rrggbb"`; `None` for Transparent.
fn background_rgb(app: &crate::PhotocraftApp, f: &Map<String, Value>) -> Option<[f32; 3]> {
    let bg = get_s(f, "background", "white");
    match bg.as_str() {
        "transparent" => None,
        "black" => Some([0.0; 3]),
        "backgroundColor" => Some(
            f.get("backgroundColor")
                .and_then(Value::as_array)
                .and_then(|a| Some([a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32]))
                .unwrap_or_else(|| {
                    let b = app.session.tools.background;
                    [b[0], b[1], b[2]]
                }),
        ),
        hex => Some(crate::color_picker_ui::parse_hex(hex).unwrap_or([1.0; 3])),
    }
}

/// Restore a snapshot without replacing the new document's name or its clipboard offer.
pub fn apply_saved_preset(f: &mut Map<String, Value>, preset: &photocraft_engine::document_preset_cmds::DocumentPreset) {
    apply_settings(f, &preset.settings);
    f.insert("__savedPreset".into(), json!(preset.name));
}

/// Restore every setting of a saved or recent document (all but its name).
pub fn apply_settings(f: &mut Map<String, Value>, settings: &DocumentSettings) {
    // Settings without Artboards or a profile turn them off (`command_params` omits defaults).
    for key in ["backgroundColor", "artboards", "profile"] {
        f.remove(key);
    }
    if let Some(params) = settings.command_params().as_object() {
        f.extend(params.clone());
    }
    f.insert("__unit".into(), json!(settings.unit));
    f.insert("__resUnit".into(), json!(settings.resolution_unit));
    f.remove("__savedPreset");
    f.remove("__preset");
    // Typed Width/Height belong to the previous values (`shown_size`).
    f.remove(&typed_key("width"));
    f.remove(&typed_key("height"));
}

/// The dialog's settings as `document.presets.save` / `document.recent.add` take them: the
/// `file.new` params plus the display units.
pub fn document_settings(f: &Map<String, Value>) -> Value {
    let mut settings = command_params(f);
    let unit = get_s(f, "__unit", "px");
    settings["unit"] = json!(if UNITS.iter().any(|u| u.0 == unit) { unit.as_str() } else { "px" });
    settings["resolutionUnit"] = json!(if get_s(f, "__resUnit", "in") == "cm" { "cm" } else { "in" });
    settings
}

/// Whether the dialog currently holds `settings` (so its Recent card shows selected).
fn holds(f: &Map<String, Value>, settings: &DocumentSettings) -> bool {
    // Unset fields hold the dialog's defaults, as the details pane shows them.
    let (w, h) = (get_f(f, "width", 1920.0), get_f(f, "height", 1080.0));
    (w, h, get_f(f, "resolution", 72.0)) == (settings.width as f32, settings.height as f32, settings.resolution)
        && f.get("depth").and_then(Value::as_u64).unwrap_or(8) == u64::from(settings.depth)
        && get_s(f, "mode", "rgb") == settings.mode
        && get_s(f, "background", "white") == settings.background
        && (f.get("artboards").and_then(Value::as_bool) == Some(true)) == settings.artboards
        && get_s(f, "profile", "working") == settings.profile
}

/// A card in the preset grid: a built-in preset or a recently created document.
enum Card {
    Builtin(Preset),
    Recent(DocumentSettings),
}

impl Card {
    /// Pixel size and resolution; a built-in preset at the Units & Rulers preset resolutions.
    fn size(&self, units: &UnitsAndRulers) -> (u32, u32, f32) {
        match self {
            Card::Builtin(listed) => {
                let p = at_pref_resolutions(listed, units);
                (p.1, p.2, p.3)
            }
            Card::Recent(s) => (s.width, s.height, s.resolution),
        }
    }

    /// The preset's name; a recent document takes the name of the built-in preset it matches.
    fn title(&self) -> &'static str {
        match self {
            Card::Builtin(p) => p.0,
            Card::Recent(s) => {
                CATEGORIES.iter().flat_map(|c| c.1.iter()).find(|p| (p.1, p.2, p.3) == (s.width, s.height, s.resolution)).map_or("Custom", |p| p.0)
            }
        }
    }

    /// The size line, in the unit the preset applies (a recent one: the unit it was made in).
    fn size_line(&self, units: &UnitsAndRulers, f: &Map<String, Value>) -> String {
        let (w, h, ppi) = self.size(units);
        let unit = match self {
            Card::Builtin(p) => preset_unit(p, f),
            Card::Recent(s) => s.unit.clone(),
        };
        if unit == "px" {
            format!("{w} x {h} px @ {} ppi", widgets::fmt_num(f64::from(ppi)))
        } else {
            let v = |n: u32| widgets::fmt_num(f64::from(to_unit(n as f32, &unit, ppi)));
            format!("{} x {} {unit} @ {} ppi", v(w), v(h), widgets::fmt_num(f64::from(ppi)))
        }
    }
}

/// The Recent category: the Clipboard preset, the documents recently created here (newest
/// first), then the built-in defaults they don't already cover.
fn recent_cards(f: &Map<String, Value>, recent: &[DocumentSettings], units: &UnitsAndRulers) -> Vec<Card> {
    let mut cards: Vec<Card> = clipboard_preset(f).map(Card::Builtin).into_iter().collect();
    cards.extend(recent.iter().cloned().map(Card::Recent));
    let defaults = CATEGORIES[0].1.iter().filter(|p| {
        let (_, w, h, ppi) = at_pref_resolutions(p, units);
        !recent.iter().any(|s| (s.width, s.height, s.resolution) == (w, h, ppi))
    });
    cards.extend(defaults.copied().map(Card::Builtin));
    cards
}

/// Width of the Preset Details column.
const DETAILS_W: f32 = 300.0;
/// A preset card, and the gap between cards (the dialog tests click at these offsets).
const CARD: egui::Vec2 = egui::Vec2::new(164.0, 112.0);
const CARD_GAP: f32 = 8.0;

/// The tabs, in order: the Recent and Saved lists, then the blank-document categories.
fn tab_names() -> impl Iterator<Item = &'static str> {
    std::iter::once(CATEGORIES[0].0).chain(["Saved"]).chain(CATEGORIES.iter().skip(1).map(|c| c.0))
}

/// The pictogram drawn inside a card's page outline: the category a preset belongs to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Glyph {
    Plain,
    Photo,
    Print,
    Art,
    Web,
    Mobile,
    Film,
    Clipboard,
    Saved,
}

impl Glyph {
    fn of_category(name: &str) -> Self {
        match name {
            "Photo" => Glyph::Photo,
            "Print" => Glyph::Print,
            "Art & Illustration" => Glyph::Art,
            "Web" => Glyph::Web,
            "Mobile" => Glyph::Mobile,
            "Film & Video" => Glyph::Film,
            "Saved" => Glyph::Saved,
            _ => Glyph::Plain,
        }
    }

    /// A built-in preset's glyph: the first category (after Recent) that lists it.
    fn of_preset(p: &Preset) -> Self {
        if p.0 == CLIPBOARD {
            return Glyph::Clipboard;
        }
        CATEGORIES.iter().skip(1).find(|c| c.1.iter().any(|q| q.0 == p.0)).map_or(Glyph::Plain, |c| Glyph::of_category(c.0))
    }
}

/// Paint a page outline with the preset's aspect ratio in `r` (a 40 pt box), and its category
/// pictogram inside it. Original drawings made from lines, circles and rectangles.
fn paint_glyph(p: &egui::Painter, r: Rect, w: u32, h: u32, glyph: Glyph, color: egui::Color32) {
    let s = 30.0 / (w.max(h).max(1) as f32);
    let size = vec2((w as f32 * s).max(8.0), (h as f32 * s).max(8.0));
    let page = Rect::from_center_size(r.center(), size);
    let stroke = Stroke::new(1.2, color);
    let thin = Stroke::new(1.0, color);
    let radius = if glyph == Glyph::Mobile { 3.0 } else { 1.0 };
    p.rect_stroke(page, radius, stroke, StrokeKind::Inside);
    let inner = page.shrink(3.0);
    let at = |fx: f32, fy: f32| pos2(inner.left() + inner.width() * fx, inner.top() + inner.height() * fy);
    match glyph {
        Glyph::Plain => {}
        Glyph::Photo => {
            p.line_segment([at(0.0, 0.95), at(0.35, 0.5)], thin);
            p.line_segment([at(0.35, 0.5), at(0.55, 0.75)], thin);
            p.line_segment([at(0.55, 0.75), at(0.75, 0.55)], thin);
            p.line_segment([at(0.75, 0.55), at(1.0, 0.95)], thin);
            p.circle_stroke(at(0.75, 0.22), (inner.width().min(inner.height()) * 0.12).max(1.0), thin);
        }
        Glyph::Print => {
            let fold = (page.width().min(page.height()) * 0.3).max(3.0);
            p.line_segment([page.right_top() + vec2(-fold, 0.0), page.right_top() + vec2(0.0, fold)], thin);
            for fy in [0.45, 0.6, 0.75] {
                p.line_segment([at(0.1, fy), at(0.8, fy)], thin);
            }
        }
        Glyph::Art => {
            p.line_segment([at(0.1, 0.9), at(0.85, 0.15)], Stroke::new(2.0, color));
            p.circle_filled(at(0.1, 0.9), 1.5, color);
        }
        Glyph::Web => {
            let bar = page.top() + (page.height() * 0.22).max(3.0);
            p.line_segment([pos2(page.left(), bar), pos2(page.right(), bar)], thin);
            for i in 0..3 {
                p.circle_filled(pos2(page.left() + 3.0 + i as f32 * 3.0, (page.top() + bar) / 2.0), 0.9, color);
            }
        }
        Glyph::Mobile => {
            let y = page.top() + 3.0;
            p.line_segment([pos2(page.center().x - 3.0, y), pos2(page.center().x + 3.0, y)], thin);
        }
        Glyph::Film => {
            let hole = 2.0;
            let n = ((page.width() / 5.0).floor() as usize).clamp(2, 8);
            for i in 0..n {
                let x = page.left() + (i as f32 + 0.5) * page.width() / n as f32;
                p.rect_filled(Rect::from_center_size(pos2(x, page.top() + 2.5), vec2(hole, hole)), 0.0, color);
                p.rect_filled(Rect::from_center_size(pos2(x, page.bottom() - 2.5), vec2(hole, hole)), 0.0, color);
            }
        }
        Glyph::Clipboard => {
            let clip = Rect::from_center_size(pos2(page.center().x, page.top()), vec2(page.width() * 0.4, 4.0));
            p.rect_filled(clip, 1.0, color);
        }
        Glyph::Saved => {
            let x = page.right() - 5.0;
            let (top, bottom) = (page.top(), page.top() + (page.height() * 0.45).max(6.0));
            p.line_segment([pos2(x - 2.0, top), pos2(x - 2.0, bottom)], thin);
            p.line_segment([pos2(x + 2.0, top), pos2(x + 2.0, bottom)], thin);
            p.line_segment([pos2(x - 2.0, bottom), pos2(x, bottom - 2.0)], thin);
            p.line_segment([pos2(x + 2.0, bottom), pos2(x, bottom - 2.0)], thin);
        }
    }
}

/// The Recent tab's welcome illustration: three overlapping pages with a plus badge.
fn paint_hero_art(p: &egui::Painter, r: Rect, pal: &crate::theme::NewDocPalette) {
    let page = vec2(r.height() * 0.62, r.height() * 0.8);
    for (i, alpha) in [(2.0, 0.35), (1.0, 0.6), (0.0, 1.0)] {
        let rect = Rect::from_min_size(r.left_top() + vec2(10.0 + i * 12.0, 4.0 + (2.0 - i) * 4.0), page);
        p.rect_filled(rect, 3.0, pal.card.lerp_to_gamma(pal.text, 0.08 * alpha));
        p.rect_stroke(rect, 3.0, Stroke::new(1.2, pal.section.gamma_multiply(alpha)), StrokeKind::Inside);
    }
    let front = Rect::from_min_size(r.left_top() + vec2(10.0, 12.0), page);
    let c = front.right_bottom() - vec2(2.0, 2.0);
    p.circle_filled(c, 9.0, pal.primary);
    p.line_segment([c - vec2(4.0, 0.0), c + vec2(4.0, 0.0)], Stroke::new(1.8, pal.primary_text));
    p.line_segment([c - vec2(0.0, 4.0), c + vec2(0.0, 4.0)], Stroke::new(1.8, pal.primary_text));
}

/// The save-preset pictogram: a page with an arrow down into a tray.
fn paint_save_icon(p: &egui::Painter, r: Rect, color: egui::Color32) {
    let s = Stroke::new(1.4, color);
    let c = r.center();
    p.line_segment([c + vec2(0.0, -6.0), c + vec2(0.0, 2.0)], s);
    p.line_segment([c + vec2(-3.5, -1.5), c + vec2(0.0, 2.0)], s);
    p.line_segment([c + vec2(3.5, -1.5), c + vec2(0.0, 2.0)], s);
    p.line_segment([c + vec2(-6.0, 1.0), c + vec2(-6.0, 6.0)], s);
    p.line_segment([c + vec2(-6.0, 6.0), c + vec2(6.0, 6.0)], s);
    p.line_segment([c + vec2(6.0, 6.0), c + vec2(6.0, 1.0)], s);
}

/// A bin pictogram for a saved preset's delete button.
fn paint_bin_icon(p: &egui::Painter, r: Rect, color: egui::Color32) {
    let s = Stroke::new(1.2, color);
    let c = r.center();
    p.line_segment([c + vec2(-5.0, -3.5), c + vec2(5.0, -3.5)], s);
    p.line_segment([c + vec2(-1.5, -5.0), c + vec2(1.5, -5.0)], s);
    let body = Rect::from_min_max(c + vec2(-3.5, -3.5), c + vec2(3.5, 5.0));
    p.rect_stroke(body, 1.0, s, StrokeKind::Middle);
    p.line_segment([c + vec2(0.0, -1.5), c + vec2(0.0, 3.0)], s);
}

/// A small square icon button painted by `paint`, named `label` for accessibility and tests.
fn painted_button(ui: &mut egui::Ui, label: &str, tooltip: &str, selected: bool, paint: impl FnOnce(&egui::Painter, Rect, egui::Color32)) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
    paint_small_button(ui, r, &resp, &t, selected, paint);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, label));
    resp.on_hover_text(tooltip).on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn paint_small_button(ui: &egui::Ui, r: Rect, resp: &egui::Response, t: &Tokens, selected: bool, paint: impl FnOnce(&egui::Painter, Rect, egui::Color32)) {
    if selected {
        ui.painter().rect_filled(r, t.radius_sm, t.accent_soft);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, t.radius_sm, t.hover);
    }
    paint(ui.painter(), r, if resp.hovered() || selected { t.text } else { t.icon });
}

/// What a card shows.
struct CardView<'a> {
    w: u32,
    h: u32,
    glyph: Glyph,
    title: &'a str,
    size: &'a str,
    on: bool,
}

/// Paint a preset card (fill, ring when selected, glyph, title, size line) and name it for
/// accessibility with its selected state. Returns the card's response and whether the title
/// was cut short.
fn card(ui: &mut egui::Ui, pal: &crate::theme::NewDocPalette, v: &CardView) -> (egui::Response, bool) {
    let t = Tokens::get(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(CARD, Sense::click());
    let painter = ui.painter();
    painter.rect_filled(r, t.radius_lg, if resp.hovered() && !v.on { pal.card_hover } else { pal.card });
    if v.on {
        painter.rect_stroke(r, t.radius_lg, Stroke::new(2.0, pal.selected_outline), StrokeKind::Inside);
    }
    paint_glyph(painter, Rect::from_center_size(pos2(r.center().x, r.top() + 34.0), vec2(40.0, 40.0)), v.w, v.h, v.glyph, pal.section);
    let title = card_title(painter, v.title, CARD.x - 12.0, pal.text);
    let elided = title.elided;
    painter.galley(pos2(r.center().x, r.top() + 70.0 - title.size().y / 2.0), title, pal.text);
    painter.text(pos2(r.center().x, r.top() + 97.0), Align2::CENTER_CENTER, v.size, egui::FontId::proportional(10.5), pal.section);
    let (on, title) = (v.on, v.title.to_owned());
    resp.widget_info(move || egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, &title));
    (resp.on_hover_cursor(egui::CursorIcon::PointingHand), elided)
}

/// How many cards fit across `width`.
fn columns(width: f32) -> usize {
    (((width + CARD_GAP) / (CARD.x + CARD_GAP)).floor() as usize).max(1)
}

/// The section heading above the cards: `text` in the palette's heading colour.
fn heading(ui: &mut egui::Ui, pal: &crate::theme::NewDocPalette, text: &str) {
    ui.label(RichText::new(text).size(11.0).color(pal.section));
    ui.add_space(6.0);
}

fn saved_presets(app: &mut crate::PhotocraftApp, ui: &mut egui::Ui, f: &mut Map<String, Value>, pal: &crate::theme::NewDocPalette) -> bool {
    let presets = app.session.presets.documents.clone();
    heading(ui, pal, &crate::i18n::fmt(tl!("YOUR SAVED PRESETS ({n})"), &[("n", &presets.len().to_string())]));
    if presets.is_empty() {
        ui.label(RichText::new(tl!("No saved presets yet.")).color(pal.section));
        return false;
    }
    let mut create = false;
    let cols = columns(ui.available_width());
    egui::ScrollArea::vertical().id_salt("saved-document-presets").max_height(3.0 * (CARD.y + CARD_GAP)).show(ui, |ui| {
        for row in presets.chunks(cols) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = CARD_GAP;
                for preset in row {
                    ui.push_id(&preset.name, |ui| {
                        let s = &preset.settings;
                        let selected = get_s(f, "__savedPreset", "") == preset.name;
                        let size = Card::Recent(s.clone()).size_line(&UnitsAndRulers::default(), f);
                        let (resp, _) =
                            card(ui, pal, &CardView { w: s.width, h: s.height, glyph: Glyph::Saved, title: &preset.name, size: &size, on: selected });
                        let rect = resp.rect;
                        let resp = resp.on_hover_text(format!(
                            "{} · {} × {} px @ {} ppi · {}/{}",
                            preset.name,
                            s.width,
                            s.height,
                            s.resolution,
                            s.mode.to_uppercase(),
                            s.depth
                        ));
                        // The delete button sits in the card's corner, above the card's own sense.
                        let bin = Rect::from_min_size(rect.right_top() + vec2(-26.0, 4.0), vec2(22.0, 22.0));
                        let del = ui.interact(bin, ui.id().with("delete"), Sense::click());
                        let t = Tokens::get(ui.ctx());
                        paint_small_button(ui, bin, &del, &t, false, paint_bin_icon);
                        del.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tl!("Delete")));
                        let del = del.on_hover_text(tl!("Delete Preset")).on_hover_cursor(egui::CursorIcon::PointingHand);
                        if del.clicked() {
                            match app.run("document.presets.delete", json!({"name":preset.name})) {
                                Ok(_) => {
                                    f.remove("__presetError");
                                    if selected {
                                        f.remove("__savedPreset");
                                    }
                                }
                                Err(e) => {
                                    f.insert("__presetError".into(), json!(e.to_string()));
                                }
                            }
                        } else if resp.clicked() {
                            apply_saved_preset(f, preset);
                            create |= resp.double_clicked() || resp.triple_clicked();
                        }
                    });
                }
            });
            ui.add_space(CARD_GAP);
        }
    });
    if let Some(error) = f.get("__presetError").and_then(Value::as_str) {
        ui.add(egui::Label::new(error).wrap());
    }
    create
}

/// The Save Preset form, shown below the name while the save button is on.
fn save_preset(app: &mut crate::PhotocraftApp, ui: &mut egui::Ui, f: &mut Map<String, Value>) {
    if f.get("__savingPreset").and_then(Value::as_bool) == Some(true) {
        ui.add_space(4.0);
        // Consume before TextEdit sees Enter, so naming can never confirm the outer dialog.
        let enter = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
        let label = ui.label(tl!("Preset Name"));
        let mut name = get_s(f, "__presetName", "");
        ui.add(egui::TextEdit::singleline(&mut name).id_salt("document-preset-name").desired_width(DETAILS_W - 28.0).char_limit(255)).labelled_by(label.id);
        f.insert("__presetName".into(), json!(name));
        ui.horizontal(|ui| {
            if widgets::secondary_button(ui, tl!("Save"), 110.0).clicked() || enter {
                match app.run("document.presets.save", json!({"name":name,"settings":document_settings(f)})) {
                    Ok(_) => {
                        if let Some(preset) = app.session.presets.documents.last() {
                            apply_saved_preset(f, preset);
                        }
                        f.insert("__category".into(), json!("Saved"));
                        f.remove("__savingPreset");
                        f.remove("__presetName");
                        f.remove("__presetError");
                        ui.ctx().request_repaint();
                    }
                    Err(e) => {
                        f.insert("__presetError".into(), json!(e.to_string()));
                    }
                }
            }
            if widgets::secondary_button(ui, tl!("Cancel"), 110.0).clicked() {
                f.remove("__savingPreset");
                f.remove("__presetError");
            }
        });
        if let Some(error) = f.get("__presetError").and_then(Value::as_str) {
            ui.add(egui::Label::new(error).wrap());
        }
    }
}

/// View › Pixel Aspect Ratio's presets: (id, menu label).
pub const PIXEL_ASPECTS: &[(&str, &str)] = &[
    ("square", "Square"),
    ("d1DvNtsc", "D1/DV NTSC (0.91)"),
    ("d1DvPal", "D1/DV PAL (1.09)"),
    ("d1DvNtscWidescreen", "D1/DV NTSC Widescreen (1.21)"),
    ("hdv1080", "HDV 1080/DVCPRO HD 720 (1.33)"),
    ("d1DvPalWidescreen", "D1/DV PAL Widescreen (1.46)"),
    ("dvcproHd1080", "DVCPRO HD 1080/HDV 1080 (1.5)"),
    ("anamorphic2To1", "Anamorphic 2:1 (2)"),
];

/// The colour mode a `mode` field names.
fn mode_of(mode: &str) -> photocraft_doc::ColorMode {
    match mode {
        "gray" | "grayscale" => photocraft_doc::ColorMode::Grayscale,
        "cmyk" => photocraft_doc::ColorMode::Cmyk,
        "lab" => photocraft_doc::ColorMode::Lab,
        _ => photocraft_doc::ColorMode::Rgb,
    }
}

/// Whether the `profile` field (a built-in id) can tag a `mode` document; "working" and "none"
/// always can.
fn profile_fits(profile: &str, mode: &str) -> bool {
    match photocraft_cms::Builtin::from_id(profile) {
        Some(b) => b.profile().color_space == photocraft_engine::color_cmds::mode_space(mode_of(mode)),
        None => true,
    }
}

/// Advanced Options: the colour profile the document is tagged with, and the view's pixel
/// aspect ratio (applied on Create).
fn advanced_options(app: &crate::PhotocraftApp, ui: &mut egui::Ui, f: &mut Map<String, Value>, width: f32) {
    let t = Tokens::get(ui.ctx());
    let open = f.get("__advanced").and_then(Value::as_bool) == Some(true);
    let label = tl!("Advanced Options");
    let (r, resp) = ui.allocate_exact_size(vec2(width, 22.0), Sense::click());
    let c = pos2(r.left() + 6.0, r.center().y);
    let chevron =
        if open { [c + vec2(-3.5, -1.5), c + vec2(0.0, 2.0), c + vec2(3.5, -1.5)] } else { [c + vec2(-1.5, -3.5), c + vec2(2.0, 0.0), c + vec2(-1.5, 3.5)] };
    let stroke = Stroke::new(1.4, if resp.hovered() { t.text } else { t.text_dim });
    ui.painter().line_segment([chevron[0], chevron[1]], stroke);
    ui.painter().line_segment([chevron[1], chevron[2]], stroke);
    ui.painter().text(
        pos2(r.left() + 16.0, r.center().y),
        Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(12.0),
        if resp.hovered() { t.text } else { t.text_dim },
    );
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::CollapsingHeader, true, open, label));
    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        f.insert("__advanced".into(), json!(!open));
    }
    if !open {
        return;
    }
    let mode = get_s(f, "mode", "rgb");
    small_label(ui, tl!("Color Profile"));
    let working = app.session.color.working(mode_of(&mode)).description.clone();
    let space = photocraft_engine::color_cmds::mode_space(mode_of(&mode));
    let mut opts: Vec<(String, String)> = vec![("working".into(), crate::i18n::fmt(tl!("Working space ({profile})"), &[("profile", &working)]))];
    opts.extend(photocraft_cms::Builtin::ALL.iter().filter(|b| b.profile().color_space == space).map(|b| (b.id().to_string(), b.description().to_string())));
    opts.push(("none".into(), tl!("No color profile (untagged)").to_string()));
    let shown: Vec<(String, &str)> = opts.iter().map(|(k, v)| (k.clone(), v.as_str())).collect();
    let mut profile = get_s(f, "profile", "working");
    if widgets::dropdown(ui, "nd-profile", &mut profile, &shown, width - 8.0) {
        if profile == "working" {
            f.remove("profile");
        } else {
            f.insert("profile".into(), json!(profile));
        }
        f.remove("__savedPreset");
    }
    ui.add_space(4.0);
    let tip = tl!("Pixel Aspect Ratio is a view setting: it applies to every open document.");
    small_label(ui, tl!("Pixel Aspect Ratio"));
    let current = if PIXEL_ASPECTS.iter().any(|p| p.0 == app.ui.view.pixel_aspect) { app.ui.view.pixel_aspect.as_str() } else { "square" };
    let mut aspect = get_s(f, "__pixelAspect", current);
    let lang = crate::i18n::current();
    let ids: Vec<String> = PIXEL_ASPECTS.iter().map(|p| format!("view.pixelAspectRatio.{}", p.0)).collect();
    let opts: Vec<(String, &str)> = PIXEL_ASPECTS.iter().zip(&ids).map(|(p, id)| (p.0.to_string(), crate::i18n::tr_id(lang, id, p.1))).collect();
    if widgets::dropdown(ui, "nd-pixel-aspect", &mut aspect, &opts, width - 8.0) {
        f.insert("__pixelAspect".into(), json!(aspect));
    }
    ui.label(RichText::new(tip).size(10.5).color(t.text_faint)).on_hover_text(tip);
}

/// The view command Create runs for the Pixel Aspect Ratio picked under Advanced Options, if it
/// changes the view's.
pub fn pixel_aspect_command(f: &Map<String, Value>, current: &str) -> Option<String> {
    let pick = f.get("__pixelAspect").and_then(Value::as_str)?;
    (pick != current && PIXEL_ASPECTS.iter().any(|p| p.0 == pick)).then(|| format!("view.pixelAspectRatio.{pick}"))
}

/// Returns whether a preset double-click requested creation, after applying its settings.
/// A recent category click can make egui count the preset double-click as a triple-click.
pub fn body(app: &mut crate::PhotocraftApp, ui: &mut egui::Ui, f: &mut Map<String, Value>) -> bool {
    let t = Tokens::get(ui.ctx());
    let pal = t.new_doc_palette();
    // Text fields and dropdowns in the dialog's own colours (scoped to this ui).
    let v = ui.visuals_mut();
    v.extreme_bg_color = pal.input;
    v.widgets.inactive.bg_fill = pal.dropdown;
    v.widgets.inactive.weak_bg_fill = pal.dropdown;
    let cat = get_s(f, "__category", "Recent");
    let mut create = false;
    // Category tabs.
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 22.0;
        for name in tab_names() {
            let on = cat == name;
            let label = if name == "Saved" { tl!("Saved") } else { tl!(name) };
            let r = ui
                .add(
                    egui::Label::new(RichText::new(label).size(13.0).color(if on { pal.text } else { pal.tab_inactive }))
                        .selectable(false)
                        .sense(Sense::click()),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, on, label));
            if on {
                ui.painter().line_segment([r.rect.left_bottom() + vec2(0.0, 4.0), r.rect.right_bottom() + vec2(0.0, 4.0)], Stroke::new(2.0, pal.text));
            }
            if r.clicked() {
                f.insert("__category".into(), json!(name));
            }
        }
    });
    ui.add_space(8.0);
    widgets::hairline(ui);
    ui.add_space(10.0);
    let units = app.session.prefs().units_and_rulers.clone();
    let presets: Vec<Card> = match CATEGORIES.iter().find(|c| c.0 == cat) {
        Some(c) if c.0 != CATEGORIES[0].0 => c.1.iter().copied().map(Card::Builtin).collect(),
        _ => recent_cards(f, &app.session.presets.recent_documents, &units),
    };
    let presets: Vec<Card> = presets
        .into_iter()
        .map(|c| match c {
            Card::Builtin(p) => Card::Builtin(oriented(&p, f)),
            recent => recent,
        })
        .collect();
    let chosen = get_s(f, "__preset", "");
    let gap = 12.0;
    let well_w = (ui.available_width() - DETAILS_W - gap).max(CARD.x + 24.0);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        // Left: the well with the cards.
        egui::Frame::new().fill(pal.well).corner_radius(t.radius_lg).inner_margin(12).show(ui, |ui| {
            ui.set_width(well_w - 24.0);
            ui.set_min_height(3.0 * (CARD.y + CARD_GAP) + 40.0);
            if cat == "Saved" {
                create |= saved_presets(app, ui, f, &pal);
                return;
            }
            if cat == CATEGORIES[0].0 {
                // Our own welcome card on the Recent tab.
                egui::Frame::new().fill(pal.hero).corner_radius(t.radius_lg).inner_margin(12).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(84.0, 56.0), Sense::hover());
                        paint_hero_art(ui.painter(), r, &pal);
                        ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(tl!("Start a new canvas")).font(crate::theme::semibold(17.0)).color(pal.text));
                            ui.add(
                                egui::Label::new(RichText::new(tl!("Choose a preset below, or set your own size in Preset Details.")).color(pal.section))
                                    .wrap(),
                            );
                        });
                    });
                });
                ui.add_space(12.0);
            }
            let title = if cat == CATEGORIES[0].0 { tl!("YOUR RECENT ITEMS ({n})") } else { tl!("BLANK DOCUMENT PRESETS ({n})") };
            heading(ui, &pal, &crate::i18n::fmt(title, &[("n", &presets.len().to_string())]));
            let cols = columns(ui.available_width());
            // Up to 20 recent documents: scroll past three rows rather than grow the dialog.
            egui::ScrollArea::vertical().id_salt("new-document-presets").max_height(3.0 * (CARD.y + CARD_GAP)).show(ui, |ui| {
                for row in presets.chunks(cols) {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = CARD_GAP;
                        for c in row {
                            let (w, h, _) = c.size(&units);
                            let on = match c {
                                Card::Builtin(p) => chosen == p.0,
                                Card::Recent(settings) => holds(f, settings),
                            };
                            let glyph = match c {
                                Card::Builtin(p) => Glyph::of_preset(p),
                                Card::Recent(s) => CATEGORIES
                                    .iter()
                                    .flat_map(|c| c.1.iter())
                                    .find(|p| (p.1, p.2, p.3) == (s.width, s.height, s.resolution))
                                    .map_or(Glyph::Plain, Glyph::of_preset),
                            };
                            let size = c.size_line(&units, f);
                            let (resp, elided) = card(ui, &pal, &CardView { w, h, glyph, title: tl!(c.title()), size: &size, on });
                            let resp = match c {
                                // A recent document's tooltip names everything it restores.
                                Card::Recent(s) => {
                                    let background = match s.background.as_str() {
                                        "white" => tl!("White"),
                                        "black" => tl!("Black"),
                                        "transparent" => tl!("Transparent"),
                                        "backgroundColor" => tl!("Background Color"),
                                        hex => hex,
                                    };
                                    resp.on_hover_text(format!("{} · {size} · {}/{} · {background}", tl!(c.title()), s.mode.to_uppercase(), s.depth))
                                }
                                Card::Builtin(_) if elided => resp.on_hover_text(tl!(c.title())),
                                Card::Builtin(_) => resp,
                            };
                            if resp.clicked() {
                                match c {
                                    Card::Builtin(p) => apply_preset(f, p, &units),
                                    Card::Recent(settings) => apply_settings(f, settings),
                                }
                                create |= resp.double_clicked() || resp.triple_clicked();
                            }
                        }
                    });
                    ui.add_space(CARD_GAP);
                }
            });
        });
        // Right: Preset Details.
        egui::Frame::new().fill(pal.details).corner_radius(t.radius_lg).inner_margin(14).show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let inner = DETAILS_W - 28.0;
            ui.set_width(inner);
            ui.vertical(|ui| {
                ui.label(RichText::new(tl!("PRESET DETAILS")).size(11.0).color(pal.section));
                ui.add_space(4.0);
                details(app, ui, f, inner);
            });
        });
    });
    create
}

/// The Preset Details column's fields.
fn details(app: &mut crate::PhotocraftApp, ui: &mut egui::Ui, f: &mut Map<String, Value>, inner: f32) {
    ui.horizontal(|ui| {
        // Keep the canonical English default in state; only the TextEdit shows a translation.
        let raw = get_s(f, "name", "Untitled-1");
        let mut name = if raw == "Untitled-1" { tl!("Untitled-1").to_string() } else { raw };
        let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(inner - 24.0 - 8.0).font(egui::FontId::proportional(15.0)));
        if r.changed() {
            f.insert("name".into(), json!(name));
        }
        crate::field_tab::register(ui.ctx(), r.id);
        // Photoshop opens the dialog with the name selected, so typing replaces it and Tab goes
        // straight on to Width. (`__focused` marks that this instance has had its turn.)
        if !f.contains_key("__focused") {
            f.insert("__focused".into(), json!(true));
            crate::field_tab::focus(ui.ctx(), r.id);
        }
        if r.gained_focus() {
            crate::field_tab::select_all(ui.ctx(), r.id, &name);
        }
        let saving = f.get("__savingPreset").and_then(Value::as_bool) == Some(true);
        if painted_button(ui, tl!("Save Preset…"), tl!("Save Preset…"), saving, paint_save_icon).clicked() {
            if saving {
                f.remove("__savingPreset");
            } else {
                f.insert("__savingPreset".into(), json!(true));
            }
            f.remove("__presetError");
        }
    });
    save_preset(app, ui, f);
    ui.add_space(8.0);
    let ppi = get_f(f, "resolution", 72.0);
    let mut unit = get_s(f, "__unit", "px");
    // A typed unit (`210mm` while in pixels) converts at the new document's resolution.
    let units = widgets::FieldUnits { unit: widgets::unit_of_key(&unit), dpi: f64::from(ppi), ppi: 72.0 };
    small_label(ui, tl!("Width"));
    ui.horizontal(|ui| {
        let mut w = shown_size(f, "width", 1920.0, &unit, ppi);
        if widgets::value_field_units(ui, &mut w, 0.01..=300_000.0, 110.0, widgets::unit_decimals(&unit), units).changed() {
            set_size(f, "width", w, &unit, ppi);
        }
        let opts: Vec<(String, &str)> = UNITS.iter().map(|u| (u.0.to_string(), u.1)).collect();
        if widgets::dropdown(ui, "nd-unit", &mut unit, &opts, 132.0) {
            f.insert("__unit".into(), json!(unit));
            f.remove("__savedPreset");
        }
    });
    small_label(ui, tl!("Height"));
    ui.horizontal(|ui| {
        let mut h = shown_size(f, "height", 1080.0, &unit, ppi);
        if widgets::value_field_units(ui, &mut h, 0.01..=300_000.0, 110.0, widgets::unit_decimals(&unit), units).changed() {
            set_size(f, "height", h, &unit, ppi);
        }
        ui.add_space(6.0);
        // Page orientation (≠ Type › Orientation → Hướng chữ).
        small_label(ui, crate::i18n::tr_ctx(crate::i18n::current(), "document", "Orientation"));
        let (w, h) = (get_f(f, "width", 1920.0), get_f(f, "height", 1080.0));
        for (icon, portrait) in [("rectangle-vertical", true), ("rectangle-horizontal", false)] {
            // Tooltip strings are looked up via `icons::button` → `tl!`; enum/state is aspect swap only.
            if icons::button(ui, icon, 24.0, (h > w) == portrait, if portrait { "Portrait" } else { "Landscape" }).clicked() {
                // The pick also turns the presets (`oriented`), even when the size already has it.
                f.insert("__orientation".into(), json!(if portrait { "portrait" } else { "landscape" }));
                if (h > w) != portrait {
                    swap_size(f);
                }
            }
        }
    });
    ui.add_space(2.0);
    let mut boards = f.get("artboards").and_then(Value::as_bool) == Some(true);
    if widgets::checkbox(ui, &mut boards, tl!("Artboards")).changed() {
        // Only sent when on, so `file.new` and saved settings stay as they were without it.
        if boards {
            f.insert("artboards".into(), json!(true));
        } else {
            f.remove("artboards");
        }
        f.remove("__savedPreset");
    }
    ui.add_space(4.0);
    small_label(ui, tl!("Resolution"));
    ui.horizontal(|ui| {
        let per_cm = get_s(f, "__resUnit", "in") == "cm";
        let mut r = if per_cm { ppi / 2.54 } else { ppi };
        if widgets::value_field(ui, &mut r, 1.0..=30_000.0, "", 110.0).changed() {
            set_resolution(f, if per_cm { r * 2.54 } else { r });
        }
        let mut ru = get_s(f, "__resUnit", "in");
        if widgets::dropdown(ui, "nd-resunit", &mut ru, &[("in".to_string(), tl!("Pixels/Inch")), ("cm".to_string(), tl!("Pixels/Centimeter"))], 132.0) {
            f.insert("__resUnit".into(), json!(ru));
            f.remove("__savedPreset");
        }
    });
    ui.add_space(4.0);
    small_label(ui, tl!("Color Mode"));
    ui.horizontal(|ui| {
        let mut mode = get_s(f, "mode", "rgb");
        if widgets::dropdown(
            ui,
            "nd-mode",
            &mut mode,
            &[
                ("gray".to_string(), tl!("Grayscale")),
                ("rgb".to_string(), tl!("RGB Color")),
                ("cmyk".to_string(), tl!("CMYK Color")),
                ("lab".to_string(), tl!("Lab Color")),
            ],
            110.0,
        ) {
            f.insert("mode".into(), json!(mode));
            // A profile for the old mode can't tag the new one.
            if !profile_fits(&get_s(f, "profile", "working"), &mode) {
                f.remove("profile");
            }
            f.remove("__savedPreset");
        }
        let mut depth = f.get("depth").and_then(Value::as_u64).unwrap_or(8);
        let depth_options: Vec<(u64, &str, &str)> = DEPTH_OPTIONS.iter().map(|(bits, label, tooltip)| (*bits, *label, *tooltip)).collect();
        if widgets::dropdown_with_tooltips(ui, "nd-depth", &mut depth, &depth_options, 132.0) {
            f.insert("depth".into(), json!(depth));
            f.remove("__savedPreset");
        }
    });
    ui.add_space(4.0);
    small_label(ui, tl!("Background Contents"));
    // A custom colour is kept as its "#rrggbb", which `file.new` takes as is (#4789).
    let current = get_s(f, "background", "white");
    let mut bg = if crate::color_picker_ui::parse_hex(&current).is_some() { CUSTOM.to_string() } else { current.clone() };
    let opts = [
        ("white".to_string(), tl!("White")),
        ("black".to_string(), tl!("Black")),
        ("backgroundColor".to_string(), tl!("Background Color")),
        ("transparent".to_string(), tl!("Transparent")),
        (CUSTOM.to_string(), tl!("Custom…")),
    ];
    let shown = background_rgb(app, f);
    let mut pick = false;
    ui.horizontal(|ui| {
        if widgets::dropdown(ui, "nd-bg", &mut bg, &opts, inner - SWATCH - 16.0) {
            if bg == CUSTOM {
                // Custom… opens the Color Picker on the colour shown so far.
                pick = true;
            } else {
                f.insert("background".into(), json!(bg));
            }
            f.remove("__savedPreset");
            // A fresh choice of Background Color uses today's toolbox colour. Selecting a
            // saved preset instead restores the colour captured when it was saved.
            f.remove("backgroundColor");
        }
        // Photoshop's swatch beside the menu: the colour the canvas starts with; a click
        // opens the Color Picker, as Custom… does.
        let (r, resp) = ui.allocate_exact_size(vec2(SWATCH, SWATCH), Sense::click());
        match shown {
            Some(c) => {
                ui.painter().rect_filled(
                    r,
                    2.0,
                    egui::Color32::from_rgb((c[0] * 255.0).round() as u8, (c[1] * 255.0).round() as u8, (c[2] * 255.0).round() as u8),
                );
            }
            None => widgets::checker(ui.painter(), r, 4.0),
        }
        ui.painter().rect_stroke(r, 2.0, Stroke::new(1.0, Tokens::get(ui.ctx()).field_border), StrokeKind::Inside);
        let resp = resp.on_hover_text(tl!("Custom background color")).on_hover_cursor(egui::CursorIcon::PointingHand);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tl!("Custom background color")));
        if resp.clicked() {
            pick = true;
            f.remove("__savedPreset");
        }
    });
    if pick {
        let start = shown.unwrap_or([1.0; 3]);
        f.insert("background".into(), json!(crate::color_picker_ui::hex(start)));
        f.remove("backgroundColor");
        crate::color_picker_ui::request_field(f, "background", &crate::color_picker_ui::title_for(tl!("Background Contents")));
    }
    ui.add_space(10.0);
    widgets::hairline(ui);
    ui.add_space(6.0);
    advanced_options(app, ui, f, inner);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_round_trip_through_pixels() {
        assert_eq!(to_unit(2100.0, "in", 300.0), 7.0);
        assert_eq!(from_unit(7.0, "in", 300.0), 2100.0);
        assert_eq!(from_unit(2.54, "cm", 300.0), 300.0);
        assert_eq!(to_unit(640.0, "px", 72.0), 640.0);
    }

    /// #1147: 5 mm at 72 ppi is 14 px, which is 4.94 mm. The field must keep showing the typed 5.
    #[test]
    fn typed_size_in_a_physical_unit_shows_as_typed() {
        let mut f = crate::state::UiState::new_document_fields();
        set_size(&mut f, "height", 5.0, "mm", 72.0);
        assert_eq!(command_params(&f)["height"], json!(14));
        assert_eq!(shown_size(&f, "height", 1080.0, "mm", 72.0), 5.0);
        // Another unit, or pixels changed elsewhere (a preset, the orientation swap): converted.
        assert_eq!(shown_size(&f, "height", 1080.0, "cm", 72.0), to_unit(14.0, "cm", 72.0));
        f.insert("height".into(), json!(1080));
        assert_eq!(shown_size(&f, "height", 1080.0, "mm", 72.0), to_unit(1080.0, "mm", 72.0));
        // Pixels show whole pixels.
        set_size(&mut f, "width", 512.4, "px", 72.0);
        assert_eq!(shown_size(&f, "width", 1920.0, "px", 72.0), 512.0);
        // Over the 300000 px limit the field shows the limit.
        set_size(&mut f, "width", 300_000.0, "mm", 72.0);
        assert_eq!(command_params(&f)["width"], json!(300_000));
        assert_eq!(shown_size(&f, "width", 1920.0, "mm", 72.0), to_unit(300_000.0, "mm", 72.0));
    }

    /// #1147: the Orientation buttons swap the typed values too: 841 x 1189 mm becomes 1189 x 841.
    #[test]
    fn orientation_swap_keeps_the_typed_values() {
        let mut f = crate::state::UiState::new_document_fields();
        set_size(&mut f, "width", 841.0, "mm", 72.0);
        set_size(&mut f, "height", 1189.0, "mm", 72.0);
        swap_size(&mut f);
        let p = command_params(&f);
        assert_eq!((p["width"].clone(), p["height"].clone()), (json!(3370), json!(2384)));
        assert_eq!((shown_size(&f, "width", 1920.0, "mm", 72.0), shown_size(&f, "height", 1080.0, "mm", 72.0)), (1189.0, 841.0));
    }

    #[test]
    fn resolution_keeps_physical_size_in_physical_units_and_pixels_in_px() {
        let a4 = CATEGORIES.iter().find(|c| c.0 == "Print").unwrap().1.iter().find(|p| p.0 == "A4").unwrap();
        let mut f = crate::state::UiState::new_document_fields();
        apply_preset(&mut f, a4, &Default::default());
        f.insert("__unit".into(), json!("in"));
        set_resolution(&mut f, 150.0);
        let p = command_params(&f);
        assert_eq!((p["width"].clone(), p["height"].clone(), p["resolution"].clone()), (json!(1240), json!(1754), json!(150.0)));
        assert!(!f.contains_key("__preset"));

        let mut f = crate::state::UiState::new_document_fields();
        apply_preset(&mut f, a4, &Default::default());
        f.insert("__unit".into(), json!("px"));
        set_resolution(&mut f, 150.0);
        let p = command_params(&f);
        assert_eq!((p["width"].clone(), p["height"].clone(), p["resolution"].clone()), (json!(2480), json!(3508), json!(150.0)));
    }

    /// Units & Rulers › New Document Preset Resolutions: print presets keep their inches at the
    /// Print Resolution, screen presets keep their pixels at the Screen Resolution.
    #[test]
    fn iso_b_paper_presets_use_correct_dimensions_and_selected_units() {
        let presets = CATEGORIES.iter().find(|c| c.0 == "Print").unwrap().1;
        let mut f = crate::state::UiState::new_document_fields();
        f.insert("__unit".into(), json!("mm"));
        for (name, w, h) in [("B3", 4169, 5906), ("B4", 2953, 4169)] {
            let preset = presets.iter().find(|p| p.0 == name).unwrap();
            assert_eq!((preset.1, preset.2, preset.3), (w, h, 300.0));
            assert_eq!(preset_unit(preset, &f), "mm");
            apply_preset(&mut f, preset, &Default::default());
            let params = command_params(&f);
            assert_eq!((params["width"].as_u64(), params["height"].as_u64()), (Some(u64::from(w)), Some(u64::from(h))));
            assert_eq!(params["resolution"], 300.0);
        }
    }

    #[test]
    fn presets_follow_the_preset_resolution_preferences() {
        let find = |cat: &str, name: &str| *CATEGORIES.iter().find(|c| c.0 == cat).unwrap().1.iter().find(|p| p.0 == name).unwrap();
        let (letter, web) = (find("Print", "Letter"), find("Web", "Web Minimum"));
        let stock = UnitsAndRulers::default();
        assert_eq!(at_pref_resolutions(&letter, &stock), letter, "the defaults are the listed sizes");
        assert_eq!(at_pref_resolutions(&web, &stock), web);

        let units = UnitsAndRulers { print_resolution: 200.0, screen_resolution: 96.0, ..stock.clone() };
        assert_eq!(at_pref_resolutions(&letter, &units), ("Letter", 1700, 2200, 200.0), "8.5 x 11 in at 200 ppi");
        assert_eq!(at_pref_resolutions(&find("Print", "A4"), &units), ("A4", 1653, 2339, 200.0));
        assert_eq!(at_pref_resolutions(&web, &units), ("Web Minimum", 1024, 768, 96.0), "pixels kept");
        let clipboard = (CLIPBOARD, 123, 45, 72.0);
        assert_eq!(at_pref_resolutions(&clipboard, &units), clipboard, "the copied pixels keep their resolution");

        let mut f = crate::state::UiState::new_document_fields();
        apply_preset(&mut f, &letter, &units);
        assert_eq!((f["width"].as_u64(), f["height"].as_u64(), f["resolution"].as_f64()), (Some(1700), Some(2200), Some(200.0)));
        assert_eq!((f["__preset"].as_str(), f["__unit"].as_str()), (Some("Letter"), Some("in")), "still a print preset below 300 ppi");
        apply_preset(&mut f, &web, &units);
        assert_eq!((f["width"].as_u64(), f["resolution"].as_f64(), f["__unit"].as_str()), (Some(1024), Some(96.0), Some("px")));

        // Out-of-range values (a hand-edited preferences file) never make an unusable size.
        for bad in [0.0, -5.0, f64::NAN, f64::INFINITY] {
            let units = UnitsAndRulers { print_resolution: bad, screen_resolution: bad, ..stock.clone() };
            assert_eq!(at_pref_resolutions(&letter, &units), letter, "{bad}");
            assert_eq!(at_pref_resolutions(&web, &units), web, "{bad}");
        }
        let huge = UnitsAndRulers { print_resolution: 1e9, ..stock };
        let (_, w, h, _) = at_pref_resolutions(&find("Art & Illustration", "Poster"), &huge);
        assert_eq!((w, h), (300_000, 300_000), "clamped to the file.new limit");
    }

    #[test]
    fn preset_card_titles_fit_the_card_in_every_language() {
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            for lang in crate::i18n::Lang::all() {
                for p in CATEGORIES.iter().flat_map(|c| c.1.iter()) {
                    let title = crate::i18n::tr(lang, p.0);
                    let g = card_title(ui.painter(), title, 152.0, egui::Color32::WHITE);
                    assert!(g.size().x <= 152.0 && g.rows.len() <= 2 && !g.elided, "{}: {title}", lang.code());
                }
            }
        });
        out.textures_delta.clear();
    }

    #[test]
    fn new_document_depth_labels_and_tooltips_are_translated() {
        // Bit-depth labels are unit/number strings that some languages legitimately write exactly
        // like the English source (Turkish and Italian "8 bit"); they only need a catalog entry there.
        const SAME_AS_SOURCE: &[(&str, &str)] = &[("tr", "8 bit"), ("tr", "16 bit"), ("it", "8 bit"), ("it", "16 bit")];
        for lang in crate::i18n::Lang::all().filter(|lang| lang.code() != "en") {
            for (_, label, tooltip) in DEPTH_OPTIONS {
                if SAME_AS_SOURCE.contains(&(lang.code(), *label)) {
                    assert!(crate::i18n::has(lang, label), "{}: {label}", lang.code());
                } else {
                    assert_ne!(crate::i18n::tr(lang, label), *label, "{}: {label}", lang.code());
                }
                assert_ne!(crate::i18n::tr(lang, tooltip), *tooltip, "{}: {tooltip}", lang.code());
            }
        }
    }

    #[test]
    fn recent_category_lists_clipboard_then_created_documents_then_uncovered_defaults() {
        let settings = |w, h, ppi| DocumentSettings {
            width: w,
            height: h,
            resolution: ppi,
            mode: "rgb".into(),
            depth: 8,
            background: "white".into(),
            background_color: None,
            unit: "px".into(),
            resolution_unit: "in".into(),
            artboards: false,
            profile: "working".into(),
        };
        let mut f = Map::new();
        set_clipboard(&mut f, 20, 30);
        let cards = recent_cards(&f, &[settings(800, 600, 72.0), settings(1920, 1080, 72.0)], &UnitsAndRulers::default());
        let titles: Vec<(&str, String)> = cards.iter().map(|c| (c.title(), c.size_line(&UnitsAndRulers::default(), &f))).collect();
        assert_eq!(
            titles,
            [
                (CLIPBOARD, "20 x 30 px @ 72 ppi".to_string()),
                ("Custom", "800 x 600 px @ 72 ppi".to_string()),
                ("HDTV 1080p", "1920 x 1080 px @ 72 ppi".to_string()),
                ("Default PhotoCraft Size", "7 x 5 in @ 300 ppi".to_string()),
            ]
        );
        assert!(holds(&f, &settings(20, 30, 72.0)) && !holds(&f, &settings(800, 600, 72.0)));
    }

    #[test]
    fn clipboard_preset_comes_first_and_is_selected() {
        // Nothing on the clipboard: the dialog opens as before.
        let mut app = crate::PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        let f = app.new_document_fields();
        assert_eq!(f, crate::state::UiState::new_document_fields());
        assert!(clipboard_preset(&f).is_none());
        // Pixels copied in the app: the Clipboard preset takes their size, at 72 ppi, selected.
        app.run("file.new", json!({"width": 200, "height": 100})).unwrap();
        app.run("select.rect", json!({"x": 10, "y": 20, "width": 123, "height": 45})).unwrap();
        app.run("edit.copy", json!({})).unwrap();
        let f = app.new_document_fields();
        assert_eq!(clipboard_preset(&f), Some((CLIPBOARD, 123, 45, 72.0)));
        assert_eq!((f["width"].as_u64(), f["height"].as_u64(), f["__preset"].as_str()), (Some(123), Some(45), Some(CLIPBOARD)));
        let p = command_params(&f);
        assert!(p.get("__clipboard").is_none(), "file.new never sees the dialog's keys");
        assert_eq!((p["width"].as_u64(), p["height"].as_u64(), p["resolution"].as_f64()), (Some(123), Some(45), Some(72.0)));
    }

    #[test]
    fn preset_sets_size_resolution_and_create_params_drop_ui_keys() {
        let mut f = crate::state::UiState::new_document_fields();
        let a4 = CATEGORIES.iter().find(|c| c.0 == "Print").unwrap().1.iter().find(|p| p.0 == "A4").unwrap();
        apply_preset(&mut f, a4, &Default::default());
        let p = command_params(&f);
        assert_eq!(p["width"], 2480);
        assert_eq!(p["height"], 3508);
        assert_eq!(p["resolution"], 300.0);
        assert!(p.get("__preset").is_none() && p.get("__unit").is_none());
        let mut s = photocraft_engine::Session::new();
        s.execute("file.new", p).unwrap();
        let d = &s.active().unwrap().doc;
        assert_eq!((d.size.width, d.size.height, d.resolution_dpi), (2480, 3508, 300.0));
    }

    #[test]
    fn saved_selection_restores_all_settings_but_keeps_the_document_name() {
        let mut session = photocraft_engine::Session::new();
        session
            .execute(
                "document.presets.save",
                json!({"name":"Print proof","settings":{
                    "width":1600,"height":800,"resolution":254.0,"mode":"cmyk","depth":16,
                    "background":"backgroundColor","backgroundColor":[0.2,0.3,0.4],"unit":"mm","resolutionUnit":"cm"
                }}),
            )
            .unwrap();
        let preset = session.presets.documents[0].clone();
        let mut f = crate::state::UiState::new_document_fields();
        set_clipboard(&mut f, 20, 30);
        f.insert("name".into(), json!("Catalog cover"));
        apply_saved_preset(&mut f, &preset);
        assert_eq!(f["name"], "Catalog cover");
        assert_eq!(f["__unit"], "mm");
        assert_eq!(f["__resUnit"], "cm");
        assert_eq!(f["__savedPreset"], "Print proof");
        assert!(f.get("__preset").is_none());
        assert_eq!(clipboard_preset(&f), Some((CLIPBOARD, 20, 30, 72.0)));
        let mut expected = preset.settings.command_params();
        expected["name"] = json!("Catalog cover");
        assert_eq!(command_params(&f), expected);
        f.insert("width".into(), json!(42));
        assert_eq!(session.presets.documents[0], preset, "editing the form does not edit the snapshot");
        apply_preset(&mut f, &(CLIPBOARD, 20, 30, 72.0), &Default::default());
        assert!(f.get("__savedPreset").is_none());
        assert_eq!((f["width"].as_u64(), f["height"].as_u64()), (Some(20), Some(30)));
    }
    /// The real dialog (#254): a typed size must reach `file.new`, however it is confirmed.
    mod dialog {
        use super::super::{CATEGORIES, apply_preset};
        use crate::PhotocraftApp;
        use crate::state::{DialogKind, UiState};
        use egui::accesskit::Role;
        use egui_kittest::{Harness, kittest::NodeT, kittest::Queryable};
        use std::sync::{Arc, Mutex};

        /// The real preference services, backed by an in-memory store shared across app restarts.
        fn harness_with_store(store: &Arc<Mutex<Option<String>>>) -> Harness<'static, PhotocraftApp> {
            let (read, write) = (store.clone(), store.clone());
            let services = crate::Services {
                load_prefs: Some(Box::new(move || read.lock().unwrap().clone())),
                save_prefs: Some(Box::new(move |text| {
                    *write.lock().unwrap() = Some(text.to_string());
                    Ok(())
                })),
                ..Default::default()
            };
            let app = PhotocraftApp::new(photocraft_engine::Session::new(), services);
            let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_ui_state(
                |ui, app| {
                    crate::prefs_ui::tick(app, ui.ctx());
                    crate::dialogs::show(app, ui.ctx());
                },
                app,
            );
            PhotocraftApp::setup_context(&h.ctx, crate::theme::ThemeKind::ALL[0]);
            h.state_mut().ui.open_dialog(DialogKind::NewDocument, UiState::new_document_fields());
            h.run_steps(3);
            h
        }

        /// #4789: the swatch beside Background Contents (and Custom…) opens the Color Picker; its
        /// colour becomes the new document's background (kept as its "#rrggbb").
        #[test]
        fn a_custom_background_color_is_picked_and_used() {
            let mut h = harness_with_store(&Arc::new(Mutex::new(None)));
            let nd = h.state().ui.dialogs[0].id;
            assert!(h.query_by_label("Custom background color").is_some());
            h.get_by_label("Custom background color").click();
            h.run_steps(3);
            let picker = h
                .state()
                .ui
                .dialogs
                .iter()
                .find(|d| d.fields.get("__dialog").and_then(serde_json::Value::as_u64) == Some(nd))
                .map(|d| d.id)
                .expect("the Color Picker opens over the dialog");
            h.state_mut().ui.dialog_mut(picker).unwrap().fields.insert("color".into(), serde_json::json!("#336699"));
            crate::dialogs::confirm(h.state_mut(), picker).unwrap();
            h.run_steps(3);
            let bg = h.state().ui.dialogs.iter().find(|d| d.id == nd).unwrap().fields["background"].clone();
            assert_eq!(bg, "#336699");
            crate::dialogs::confirm(h.state_mut(), nd).unwrap();
            let doc = &h.state().session.active().unwrap().doc;
            let px = photocraft_compose::flatten(doc).px[0];
            assert!((px[0] * 255.0 - 51.0).abs() < 1.0 && (px[1] * 255.0 - 102.0).abs() < 1.0 && (px[2] * 255.0 - 153.0).abs() < 1.0, "{px:?}");
        }

        /// #4789: the swatch shows what each Background Contents choice paints, Background Color included.
        #[test]
        fn the_swatch_shows_each_background_choice() {
            let mut h = harness_with_store(&Arc::new(Mutex::new(None)));
            h.state_mut().run("tools.setColors", serde_json::json!({"foreground": "#000000", "background": "#336699"})).unwrap();
            let rgb = |bg: &str| super::super::background_rgb(h.state(), serde_json::json!({ "background": bg }).as_object().unwrap());
            assert_eq!(rgb("white"), Some([1.0; 3]));
            assert_eq!(rgb("black"), Some([0.0; 3]));
            assert_eq!(rgb("transparent"), None);
            assert_eq!(rgb("#ff0000"), Some([1.0, 0.0, 0.0]));
            let c = rgb("backgroundColor").expect("Background Color shows the toolbox colour");
            assert!((c[0] * 255.0 - 51.0).abs() < 1.0 && (c[1] * 255.0 - 102.0).abs() < 1.0 && (c[2] * 255.0 - 153.0).abs() < 1.0, "{c:?}");
        }

        fn name_preset(h: &mut Harness<'static, PhotocraftApp>, name: &str) {
            h.get_by_label("Save Preset…").click();
            h.run_steps(2);
            h.get_by_role_and_label(Role::TextInput, "Preset Name").click();
            h.run_steps(1);
            h.event(egui::Event::Text(name.into()));
            h.run_steps(1);
        }

        #[test]
        fn save_restart_select_create_and_delete_through_the_dialog() {
            let store = Arc::new(Mutex::new(None));
            let mut h = harness_with_store(&store);
            type_into(&mut h, 0, "1600");
            type_into(&mut h, 1, "1600");
            let mut f = fields(&h);
            f.insert("background".into(), serde_json::json!("transparent"));
            set_fields(&mut h, f);
            name_preset(&mut h, " Product square ");
            h.get_by_label("Save").click();
            h.run_steps(3);
            assert!(h.state().session.documents().is_empty(), "saving does not create a document");
            assert_eq!(h.state().session.presets.documents[0].name, "Product square");
            type_into(&mut h, 0, "32");
            assert_eq!(h.state().session.presets.documents[0].settings.width, 1600);
            drop(h);

            let mut h = harness_with_store(&store);
            h.get_by_label("Saved").click();
            h.run_steps(2);
            h.get_by_label("Product square").click();
            h.run_steps(2);
            assert!(h.state().session.documents().is_empty(), "selection only populates the form");
            assert_eq!(fields(&h)["name"], "Untitled-1");
            assert_eq!(fields(&h)["background"], "transparent");
            h.get_by_label("Create").click();
            h.run_steps(3);
            assert_eq!(created(&h), (1600, 1600, 72.0));
            let pixel = h.state_mut().session.execute("document.pixel", serde_json::json!({"x":0,"y":0})).unwrap();
            assert_eq!(pixel[3], 0.0);

            h.state_mut().ui.open_dialog(DialogKind::NewDocument, UiState::new_document_fields());
            h.run_steps(2);
            h.get_by_label("Saved").click();
            h.run_steps(2);
            h.get_by_label("Delete").click();
            h.run_steps(3);
            drop(h);
            let mut h = harness_with_store(&store);
            h.get_by_label("Saved").click();
            h.run_steps(2);
            assert!(h.state().session.presets.documents.is_empty());
            assert!(h.query_by_label("No saved presets yet.").is_some());
        }

        #[test]
        fn enter_saves_the_preset_and_bad_names_leave_the_form_open() {
            let mut h = harness();
            name_preset(&mut h, "Enter test");
            enter(&mut h);
            assert_eq!(h.state().session.presets.documents.len(), 1);
            assert!(h.state().session.documents().is_empty(), "Enter must not also press Create");
            assert_eq!(h.state().ui.dialogs.len(), 1);
            name_preset(&mut h, " ENTER TEST ");
            h.get_by_label("Save").click();
            h.run_steps(2);
            assert!(fields(&h).get("__presetError").is_some());
            assert_eq!(h.state().session.presets.documents.len(), 1);
            assert!(h.state().session.documents().is_empty());
            h.get_by_label("Cancel").click();
            h.run_steps(2);
            name_preset(&mut h, "");
            // Cancel keeps the entered name; clear it as an automation edit, then use the button.
            h.state_mut().ui.dialogs[0].fields.insert("__presetName".into(), serde_json::json!("   "));
            h.run_steps(2);
            h.get_by_label("Save").click();
            h.run_steps(2);
            assert!(fields(&h).get("__presetError").is_some());
            assert_eq!(h.state().session.presets.documents.len(), 1);
        }

        fn harness() -> Harness<'static, PhotocraftApp> {
            let app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
            let mut h = Harness::builder()
                .with_size(egui::vec2(1400.0, 900.0))
                .with_step_dt(1.0 / 60.0)
                .build_ui_state(|ui, app| crate::dialogs::show(app, ui.ctx()), app);
            PhotocraftApp::setup_context(&h.ctx, crate::theme::ThemeKind::ALL[0]);
            h.state_mut().ui.open_dialog(DialogKind::NewDocument, UiState::new_document_fields());
            h.run_steps(3);
            h
        }

        fn click_at(h: &mut Harness<'static, PhotocraftApp>, at: egui::Pos2) {
            h.hover_at(at);
            h.run_steps(1);
            h.drag_at(at);
            h.run_steps(1);
            h.drop_at(at);
            h.run_steps(2);
        }

        /// The Width (0), Height (1) and Resolution (2) fields.
        fn field(h: &Harness<'static, PhotocraftApp>, i: usize) -> egui::Rect {
            h.query_all_by_role(Role::SpinButton).nth(i).map(|n| n.rect()).expect("a size field")
        }

        /// Click into field `i` (which selects its text) and type `text`, as a user does.
        fn type_into(h: &mut Harness<'static, PhotocraftApp>, i: usize, text: &str) {
            let r = field(h, i);
            click_at(h, r.center());
            for c in text.chars() {
                h.event(egui::Event::Text(c.to_string()));
                h.run_steps(1);
            }
        }

        fn fields(h: &Harness<'static, PhotocraftApp>) -> serde_json::Map<String, serde_json::Value> {
            h.state().ui.dialogs.first().map(|d| d.fields.clone()).expect("the dialog is open")
        }

        fn set_fields(h: &mut Harness<'static, PhotocraftApp>, f: serde_json::Map<String, serde_json::Value>) {
            h.state_mut().ui.dialogs[0].fields = f;
            h.run_steps(2);
        }

        fn enter(h: &mut Harness<'static, PhotocraftApp>) {
            h.key_press(egui::Key::Enter);
            h.run_steps(3);
        }

        fn created(h: &Harness<'static, PhotocraftApp>) -> (u32, u32, f32) {
            assert!(h.state().ui.dialogs.is_empty(), "the dialog closed");
            let d = &h.state().session.active().expect("a new document").doc;
            (d.size.width, d.size.height, d.resolution_dpi)
        }

        /// Preset cards paint their titles directly, without an accessibility label.
        fn preset_title_at(h: &Harness<'static, PhotocraftApp>, title: &str) -> egui::Pos2 {
            fn find(shape: &egui::Shape, title: &str) -> Option<egui::Pos2> {
                match shape {
                    egui::Shape::Text(text) if text.galley.job.text == title => Some(text.visual_bounding_rect().center()),
                    egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find(shape, title)),
                    _ => None,
                }
            }
            h.output().shapes.iter().find_map(|shape| find(&shape.shape, title)).expect("visible preset title")
        }

        fn second_click_creates_once(h: &mut Harness<'static, PhotocraftApp>, at: egui::Pos2) {
            assert!(h.state().session.documents().is_empty(), "the first click only selects the preset");
            assert_eq!(h.state().ui.dialogs.len(), 1);
            // Two real pointer press/release pairs within egui's double-click interval.
            click_at(h, at);
            h.run_steps(3);
            assert_eq!(h.state().session.documents().len(), 1, "the second click creates exactly one document");
            assert!(h.state().ui.dialogs.is_empty(), "double-click closes the New Document dialog");
            assert!(h.state().ui.views[0].fit_pending, "the normal Create path fits the new document");
        }

        fn assert_pixel(h: &mut Harness<'static, PhotocraftApp>, expected: [f64; 4]) {
            let pixel = h.state_mut().session.execute("document.pixel", serde_json::json!({"x":0,"y":0})).unwrap();
            for (actual, expected) in pixel.as_array().unwrap().iter().zip(expected) {
                assert!((actual.as_f64().unwrap() - expected).abs() < 0.001, "background pixel: {pixel}");
            }
        }

        #[test]
        fn double_click_builtin_preset_creates_its_size_and_current_details() {
            let mut h = harness();
            let mut f = fields(&h);
            f.extend(serde_json::json!({"name":"Portrait proof","mode":"cmyk","depth":16,"background":"transparent"}).as_object().unwrap().clone());
            set_fields(&mut h, f);
            h.get_by_label("Photo").click();
            h.run_steps(2);
            let at = preset_title_at(&h, "Portrait, 4 x 6");
            click_at(&mut h, at);
            assert_eq!(fields(&h)["__preset"], "Portrait, 4 x 6");
            assert_eq!(
                super::super::command_params(&fields(&h)),
                serde_json::json!({"name":"Portrait proof","width":1200,"height":1800,"resolution":300.0,
                    "mode":"cmyk","depth":16,"background":"transparent"})
            );
            second_click_creates_once(&mut h, at);
            assert_eq!(created(&h), (1200, 1800, 300.0));
            let doc = &h.state().session.active().unwrap().doc;
            assert_eq!(doc.name, "Portrait proof");
            assert_eq!((doc.mode, doc.depth), (photocraft_doc::ColorMode::Cmyk, photocraft_doc::SampleType::U16));
            assert_pixel(&mut h, [0.0; 4]);
        }

        #[test]
        fn double_click_clipboard_preset_creates_its_size_and_current_details() {
            let mut h = harness();
            let mut f = fields(&h);
            super::super::set_clipboard(&mut f, 37, 23);
            apply_preset(&mut f, &("Other size", 100, 200, 300.0), &Default::default());
            f.extend(serde_json::json!({"mode":"gray","depth":8,"background":"white"}).as_object().unwrap().clone());
            set_fields(&mut h, f);
            let at = preset_title_at(&h, super::super::CLIPBOARD);
            click_at(&mut h, at);
            assert_eq!(fields(&h)["__preset"], super::super::CLIPBOARD);
            assert_eq!(
                super::super::command_params(&fields(&h)),
                serde_json::json!({"name":"Untitled-1","width":37,"height":23,"resolution":72.0,
                    "mode":"gray","depth":8,"background":"white"})
            );
            second_click_creates_once(&mut h, at);
            assert_eq!(created(&h), (37, 23, 72.0));
            let doc = &h.state().session.active().unwrap().doc;
            assert_eq!((doc.mode, doc.depth), (photocraft_doc::ColorMode::Grayscale, photocraft_doc::SampleType::U8));
            assert_pixel(&mut h, [1.0; 4]);
        }

        #[test]
        fn double_click_saved_preset_restores_all_settings_and_captured_background() {
            let mut h = harness();
            h.state_mut()
                .run(
                    "document.presets.save",
                    serde_json::json!({"name":"Saved proof","settings":{
                        "width":48,"height":32,"resolution":254.0,"mode":"rgb","depth":32,
                        "background":"backgroundColor","backgroundColor":[0.125,0.375,0.625],"unit":"mm","resolutionUnit":"cm"
                    }}),
                )
                .unwrap();
            h.state_mut().session.tools.background = [1.0, 0.0, 1.0, 1.0];
            let mut f = fields(&h);
            f.insert("name".into(), serde_json::json!("Current document name"));
            set_fields(&mut h, f);
            h.get_by_label("Saved").click();
            h.run_steps(2);
            let at = h.get_by_label("Saved proof").rect().center();
            click_at(&mut h, at);
            assert_eq!(fields(&h)["__savedPreset"], "Saved proof");
            assert_eq!(fields(&h)["__unit"], "mm");
            assert_eq!(fields(&h)["__resUnit"], "cm");
            second_click_creates_once(&mut h, at);
            assert_eq!(created(&h), (48, 32, 254.0));
            let doc = &h.state().session.active().unwrap().doc;
            assert_eq!(doc.name, "Current document name");
            assert_eq!((doc.mode, doc.depth), (photocraft_doc::ColorMode::Rgb, photocraft_doc::SampleType::F32));
            assert_pixel(&mut h, [0.125, 0.375, 0.625, 1.0]);
        }

        #[test]
        fn preset_cards_use_the_preset_resolution_preferences() {
            let mut h = harness();
            h.state_mut().run("prefs.set", serde_json::json!({"path": "unitsAndRulers.printResolution", "value": 150.0})).unwrap();
            h.state_mut().run("prefs.set", serde_json::json!({"path": "unitsAndRulers.screenResolution", "value": 96.0})).unwrap();
            h.get_by_label("Photo").click();
            h.run_steps(2);
            let at = preset_title_at(&h, "Landscape, 6 x 4");
            click_at(&mut h, at);
            assert_eq!(fields(&h)["__unit"], "in");
            second_click_creates_once(&mut h, at);
            assert_eq!(created(&h), (900, 600, 150.0), "6 x 4 in at 150 ppi");

            h.state_mut().ui.open_dialog(DialogKind::NewDocument, UiState::new_document_fields());
            h.run_steps(2);
            h.get_by_label("Web").click();
            h.run_steps(2);
            let at = preset_title_at(&h, "Web Minimum");
            click_at(&mut h, at);
            enter(&mut h);
            assert_eq!(created(&h), (1024, 768, 96.0));
        }

        #[test]
        fn arithmetic_dimensions_create_the_evaluated_size() {
            let mut h = harness();
            type_into(&mut h, 0, "1920/2");
            type_into(&mut h, 1, "(100+50)*2");
            enter(&mut h);
            assert_eq!(created(&h), (960, 300, 72.0));
        }

        #[test]
        fn typed_size_then_enter_creates_that_size() {
            let mut h = harness();
            type_into(&mut h, 0, "512");
            type_into(&mut h, 1, "512");
            let f = fields(&h);
            assert_eq!((f["width"].as_u64(), f["height"].as_u64()), (Some(512), Some(512)), "whole pixels: {f:?}");
            enter(&mut h);
            assert_eq!(created(&h), (512, 512, 72.0));
        }

        #[test]
        fn typed_size_then_create_without_leaving_the_field_creates_that_size() {
            let mut h = harness();
            type_into(&mut h, 0, "512");
            type_into(&mut h, 1, "300");
            // Still editing Height: click Create straight away.
            let create = h.get_by_label("Create").rect();
            click_at(&mut h, create.center());
            assert_eq!(created(&h), (512, 300, 72.0));
        }

        #[test]
        fn typing_over_a_preset_wins() {
            let mut h = harness();
            let mut f = fields(&h);
            let web = CATEGORIES.iter().find(|c| c.0 == "Web").unwrap().1.iter().find(|p| p.0 == "Web Minimum").unwrap();
            apply_preset(&mut f, web, &Default::default());
            set_fields(&mut h, f);
            type_into(&mut h, 0, "512");
            type_into(&mut h, 1, "512");
            assert!(fields(&h).get("__preset").is_none(), "typing deselects the preset");
            enter(&mut h);
            assert_eq!(created(&h), (512, 512, 72.0));
        }

        #[test]
        fn the_clipboard_card_is_first_and_creates_the_clipboard_size() {
            let mut h = harness();
            let mut f = fields(&h);
            super::super::set_clipboard(&mut f, 640, 360);
            set_fields(&mut h, f);
            // Recent lists the Clipboard card first: three presets instead of two.
            assert!(h.query_by_label_contains("YOUR RECENT ITEMS (3)").is_some());
            let heading = h.get_by_label_contains("YOUR RECENT ITEMS").rect();
            // Pick the second card, then the first (Clipboard) again.
            click_at(&mut h, heading.left_bottom() + egui::vec2(80.0 + 172.0, 60.0));
            assert_eq!(fields(&h).get("__preset").and_then(|v| v.as_str()), Some("Default PhotoCraft Size"));
            click_at(&mut h, heading.left_bottom() + egui::vec2(80.0, 60.0));
            assert_eq!(fields(&h).get("__preset").and_then(|v| v.as_str()), Some(super::super::CLIPBOARD));
            enter(&mut h);
            assert_eq!(created(&h), (640, 360, 72.0));
        }

        #[test]
        fn clicking_a_preset_card_after_typing_sets_its_size() {
            let mut h = harness();
            type_into(&mut h, 0, "512");
            h.get_by_label("Photo").click();
            h.run_steps(2);
            // The first card ("Landscape, 6 x 4") sits under the presets heading.
            let heading = h.get_by_label_contains("BLANK DOCUMENT PRESETS").rect();
            click_at(&mut h, heading.left_bottom() + egui::vec2(80.0, 60.0));
            assert_eq!(fields(&h).get("__preset").and_then(|v| v.as_str()), Some("Landscape, 6 x 4"));
            enter(&mut h);
            assert_eq!(created(&h), (1800, 1200, 300.0));
        }

        #[test]
        fn typed_size_in_inches_converts_at_the_resolution() {
            let mut h = harness();
            type_into(&mut h, 2, "300");
            let mut f = fields(&h);
            f.insert("__unit".into(), serde_json::json!("in"));
            set_fields(&mut h, f);
            type_into(&mut h, 0, "2");
            type_into(&mut h, 1, "1.5");
            enter(&mut h);
            assert_eq!(created(&h), (600, 450, 300.0));
        }

        /// #1147: typed digit by digit, millimetres used to be rewritten after each keystroke
        /// (`8` became `8.1`), so 841 never arrived.
        #[test]
        fn typed_size_in_millimetres_keeps_every_digit() {
            let mut h = harness();
            let mut f = fields(&h);
            f.insert("__unit".into(), serde_json::json!("mm"));
            set_fields(&mut h, f);
            type_into(&mut h, 0, "841");
            type_into(&mut h, 1, "1189");
            enter(&mut h);
            assert_eq!(created(&h), (2384, 3370, 72.0));
        }

        #[test]
        fn changing_units_keeps_the_typed_pixel_size() {
            let mut h = harness();
            type_into(&mut h, 0, "512");
            type_into(&mut h, 1, "512");
            for unit in ["in", "cm", "mm", "pt", "pica", "px"] {
                let mut f = fields(&h);
                f.insert("__unit".into(), serde_json::json!(unit));
                set_fields(&mut h, f);
            }
            enter(&mut h);
            assert_eq!(created(&h), (512, 512, 72.0));
        }

        #[test]
        fn orientation_swap_keeps_whole_pixels() {
            let mut h = harness();
            type_into(&mut h, 0, "512");
            type_into(&mut h, 1, "256");
            // The Portrait icon button follows the "Orientation" label (icons have tooltips only).
            let label = h.get_by_label("Orientation").rect();
            let gap = h.ctx.global_style().spacing.item_spacing.x;
            click_at(&mut h, egui::pos2(label.right() + gap + 12.0, label.center().y));
            enter(&mut h);
            assert_eq!(created(&h), (256, 512, 72.0));
        }

        /// #4668: once an orientation is picked, the presets turn to it, on the cards and when applied.
        #[test]
        fn picked_orientation_turns_the_presets() {
            let mut h = harness();
            h.get_by_label("Print").click();
            h.run_steps(2);
            // The default 1920 x 1080 is already landscape: Landscape swaps nothing, but is picked.
            let label = h.get_by_label("Orientation").rect();
            let gap = h.ctx.global_style().spacing.item_spacing.x;
            click_at(&mut h, egui::pos2(label.right() + gap + 24.0 + gap + 12.0, label.center().y));
            assert_eq!((fields(&h)["width"].as_u64(), fields(&h)["height"].as_u64()), (Some(1920), Some(1080)));
            let _ = preset_title_at(&h, "11.7 x 8.3 in @ 300 ppi");
            let at = preset_title_at(&h, "A4");
            click_at(&mut h, at);
            enter(&mut h);
            assert_eq!(created(&h), (3508, 2480, 300.0));
        }

        /// #4668: a physical unit picked for Width/Height is the one print presets show and keep.
        #[test]
        fn picked_physical_unit_is_the_print_presets_unit() {
            let mut h = harness();
            let mut f = fields(&h);
            f.insert("__unit".into(), serde_json::json!("mm"));
            set_fields(&mut h, f);
            h.get_by_label("Print").click();
            h.run_steps(2);
            let _ = preset_title_at(&h, "210 x 297 mm @ 300 ppi");
            let at = preset_title_at(&h, "A4");
            click_at(&mut h, at);
            assert_eq!(fields(&h)["__unit"], "mm");
            // Screen presets stay in pixels.
            h.get_by_label("Web").click();
            h.run_steps(2);
            let _ = preset_title_at(&h, "1024 x 768 px @ 72 ppi");
            enter(&mut h);
            assert_eq!(created(&h), (2480, 3508, 300.0));
        }

        /// The tab row reads Recent, Saved, then the categories, left to right, and says which
        /// tab is current; the selected card says so too.
        #[test]
        fn saved_is_the_second_tab_and_tabs_and_cards_expose_selection() {
            let mut h = harness();
            let tabs: Vec<(String, f32)> = ["Recent", "Saved", "Photo", "Print", "Art & Illustration", "Web", "Mobile", "Film & Video"]
                .iter()
                .map(|t| (t.to_string(), h.get_by_label(t).rect().left()))
                .collect();
            assert!(tabs.windows(2).all(|w| w[0].1 < w[1].1), "{tabs:?}");
            assert_eq!(h.get_by_label("Recent").accesskit_node().toggled(), Some(egui::accesskit::Toggled::True));
            assert_eq!(h.get_by_label("Saved").accesskit_node().toggled(), Some(egui::accesskit::Toggled::False));
            h.get_by_label("Photo").click();
            h.run_steps(2);
            assert_eq!(h.get_by_label("Photo").accesskit_node().toggled(), Some(egui::accesskit::Toggled::True));
            assert_eq!(h.get_by_label("Recent").accesskit_node().toggled(), Some(egui::accesskit::Toggled::False));
            assert_eq!(h.get_by_label("Landscape, 6 x 4").accesskit_node().toggled(), Some(egui::accesskit::Toggled::False));
            h.get_by_label("Landscape, 6 x 4").click();
            h.run_steps(2);
            assert_eq!(fields(&h)["__preset"], "Landscape, 6 x 4");
            assert_eq!(h.get_by_label("Landscape, 6 x 4").accesskit_node().toggled(), Some(egui::accesskit::Toggled::True));
        }

        /// The welcome card is on Recent only, and each tab has its own heading.
        #[test]
        fn the_hero_is_on_recent_only_with_a_heading_per_tab() {
            let mut h = harness();
            assert!(h.query_by_label("Start a new canvas").is_some());
            assert!(h.query_by_label_contains("YOUR RECENT ITEMS (").is_some());
            h.get_by_label("Saved").click();
            h.run_steps(2);
            assert!(h.query_by_label("Start a new canvas").is_none());
            assert!(h.query_by_label("YOUR SAVED PRESETS (0)").is_some());
            assert!(h.query_by_label("No saved presets yet.").is_some());
            h.get_by_label("Web").click();
            h.run_steps(2);
            assert!(h.query_by_label("Start a new canvas").is_none());
            assert!(h.query_by_label("BLANK DOCUMENT PRESETS (5)").is_some());
        }

        /// The selected card is ringed in the palette's outline colour, 2 pt wide.
        #[test]
        fn the_selected_card_has_the_outline() {
            let mut h = harness();
            h.get_by_label("Web").click();
            h.run_steps(2);
            h.get_by_label("Web Minimum").click();
            h.run_steps(2);
            let pal = crate::theme::Tokens::for_kind(crate::theme::ThemeKind::ALL[0]).new_doc_palette();
            let card = h.get_by_label("Web Minimum").rect();
            fn ring(shape: &egui::Shape, card: egui::Rect, color: egui::Color32) -> bool {
                match shape {
                    egui::Shape::Rect(r) => r.rect == card && r.stroke.width == 2.0 && r.stroke.color == color,
                    egui::Shape::Vec(shapes) => shapes.iter().any(|s| ring(s, card, color)),
                    _ => false,
                }
            }
            assert!(h.output().shapes.iter().any(|s| ring(&s.shape, card, pal.selected_outline)), "a 2 pt ring around the selected card");
            // The theme's own tokens are back after the dialog.
            let after = crate::theme::Tokens::get(&h.ctx);
            assert_ne!(after, after.for_new_doc());
        }

        /// Advanced Options starts closed; `__advanced` (or a click) opens it, and its colour profile
        /// and pixel aspect ratio reach `file.new` and the View menu's command on Create.
        #[test]
        fn advanced_options_toggle_and_reach_create() {
            let mut h = harness();
            assert!(h.query_by_label("Color Profile").is_none(), "closed by default");
            let mut f = fields(&h);
            f.insert("__advanced".into(), serde_json::json!(true));
            set_fields(&mut h, f);
            assert!(h.query_by_label("Color Profile").is_some());
            assert!(h.query_by_label("Pixel Aspect Ratio").is_some());
            h.get_by_label("Advanced Options").click();
            h.run_steps(2);
            assert_eq!(fields(&h)["__advanced"], false);
            assert!(h.query_by_label("Color Profile").is_none());
            let mut f = fields(&h);
            f.extend(serde_json::json!({"profile":"display-p3","__pixelAspect":"hdv1080"}).as_object().unwrap().clone());
            set_fields(&mut h, f);
            // A mode the profile can't tag drops it (the Color Mode menu checks this).
            assert!(super::super::profile_fits("display-p3", "rgb") && !super::super::profile_fits("display-p3", "cmyk"));
            h.get_by_label("Create").click();
            h.run_steps(3);
            let doc = &h.state().session.active().expect("a new document").doc;
            assert!(doc.icc_profile.is_some(), "tagged with Display P3");
            assert_eq!(h.state().ui.view.pixel_aspect, "hdv1080");
        }

        /// The Artboards box sends `"artboards": true` (and nothing when off).
        #[test]
        fn the_artboards_checkbox_reaches_file_new() {
            let mut h = harness();
            assert!(super::super::command_params(&fields(&h)).get("artboards").is_none());
            h.get_by_label("Artboards").click();
            h.run_steps(2);
            assert_eq!(super::super::command_params(&fields(&h))["artboards"], true);
            h.get_by_label("Artboards").click();
            h.run_steps(2);
            assert!(super::super::command_params(&fields(&h)).get("artboards").is_none());
            h.get_by_label("Artboards").click();
            h.run_steps(2);
            h.get_by_label("Create").click();
            h.run_steps(3);
            let doc = &h.state().session.active().expect("a new document").doc;
            assert_eq!(doc.artboards().len(), 1);
        }

        /// Close then Create from the left, Create the primary pill in the corner.
        #[test]
        fn close_sits_left_of_create() {
            let h = harness();
            assert!(h.get_by_label("Close").rect().right() < h.get_by_label("Create").rect().left());
        }

        #[test]
        fn hovering_preset_tabs_shows_pointing_hand_not_text_cursor() {
            let mut h = harness();
            for tab in ["Recent", "Saved", "Photo", "Print", "Art & Illustration", "Web", "Mobile", "Film & Video"] {
                let rect = h.get_by_label(tab).rect();
                h.hover_at(rect.center());
                h.run_steps(2);
                assert_eq!(
                    h.output().platform_output.cursor_icon,
                    egui::CursorIcon::PointingHand,
                    "hovering preset tab {tab:?} must show PointingHand instead of text caret"
                );
            }
        }
    }
}
