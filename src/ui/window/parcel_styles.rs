//! Parcel Thematic Zoning & Styles Manager for OpenCADStudio.
//!
//! Provides a floating dialog to configure parcel styles, colors, hatch patterns,
//! transparency, and scale for different zoning classifications, dynamically
//! update the drawing, generate a cadastral legend table, and automatically
//! apply styling to new or individually edited parcels.

use crate::app::commands::civil_parcel::ParcelData;
use crate::app::Message;
use crate::i18n::lang_str;
use crate::ui::style::form::dialog_button;
use codec::entities::hatch::{
    BoundaryEdge, BoundaryPath, Hatch, HatchPattern, HatchPatternLine, HatchPatternType,
    HatchStyleType, PolylineEdge,
};
use codec::tables::layer::Layer;
use codec::types::{Color, Transparency, Vector2, Vector3};
use codec::xdata::{ExtendedDataRecord, XDataValue};
use codec::{CadDocument, EntityType, Handle};
use iced::widget::{
    button, checkbox, column, container, pick_list, row, scrollable, slider, text, text_input,
    Space,
};
use iced::{Background, Border, Color as IcedColor, Element, Fill, Length, Theme};
use std::borrow::Cow;

pub const XDATA_PARCEL_HATCH: &str = "OPENCAD_PARCEL_HATCH";

/// Color palette chips for quick color assignment.
pub const COLOR_PALETTE: &[(&str, [u8; 3])] = &[
    ("Gold", [235, 185, 50]),
    ("Coral", [230, 80, 60]),
    ("Emerald", [50, 180, 80]),
    ("Civic Blue", [50, 140, 230]),
    ("Slate Gray", [140, 150, 160]),
    ("Purple", [160, 80, 220]),
    ("Orange", [245, 130, 32]),
    ("Teal", [20, 184, 166]),
    ("Brown", [160, 100, 50]),
];

/// Supported hatch patterns for parcel thematic displays.
pub const HATCH_PATTERNS: &[&str] = &[
    "SOLID",
    "ANSI31",
    "NET",
    "CROSS",
    "DOTS",
    "AR-CONC",
    "NONE",
];

/// A single zoning style definition.
#[derive(Clone, Debug, PartialEq)]
pub struct ParcelZoningStyle {
    pub key: String,
    pub name: String,
    pub layer: String,
    pub color_rgb: [u8; 3],
    pub enable_hatch: bool,
    pub hatch_pattern: String,
    pub hatch_scale: f64,
    pub hatch_angle_deg: f64,
    pub transparency_pct: u8,
}

impl ParcelZoningStyle {
    pub fn new(
        key: &str,
        name: &str,
        layer: &str,
        color_rgb: [u8; 3],
        enable_hatch: bool,
        hatch_pattern: &str,
        hatch_scale: f64,
        hatch_angle_deg: f64,
        transparency_pct: u8,
    ) -> Self {
        Self {
            key: key.to_string(),
            name: name.to_string(),
            layer: layer.to_string(),
            color_rgb,
            enable_hatch,
            hatch_pattern: hatch_pattern.to_string(),
            hatch_scale,
            hatch_angle_deg,
            transparency_pct,
        }
    }

    pub fn iced_color(&self) -> IcedColor {
        IcedColor::from_rgb(
            self.color_rgb[0] as f32 / 255.0,
            self.color_rgb[1] as f32 / 255.0,
            self.color_rgb[2] as f32 / 255.0,
        )
    }
}

/// State of the Parcel Thematic Styles dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct ParcelStylesDialogState {
    pub selected_index: usize,
    pub styles: Vec<ParcelZoningStyle>,
    pub auto_apply_to_new: bool,
    pub status_message: String,
}

impl Default for ParcelStylesDialogState {
    fn default() -> Self {
        Self {
            selected_index: 0,
            auto_apply_to_new: true,
            status_message: String::new(),
            styles: vec![
                ParcelZoningStyle::new(
                    "Residential",
                    "Single-Family (Residential)",
                    "PC-RESIDENTIAL",
                    [235, 185, 50],
                    true,
                    "SOLID",
                    1.0,
                    0.0,
                    80,
                ),
                ParcelZoningStyle::new(
                    "Commercial",
                    "Commercial",
                    "PC-COMMERCIAL",
                    [230, 80, 60],
                    true,
                    "ANSI31",
                    2.0,
                    45.0,
                    60,
                ),
                ParcelZoningStyle::new(
                    "Park",
                    "Park / Open Space",
                    "PC-PARK",
                    [50, 180, 80],
                    true,
                    "NET",
                    2.5,
                    0.0,
                    70,
                ),
                ParcelZoningStyle::new(
                    "Civic",
                    "Civic / Institutional",
                    "PC-CIVIC",
                    [50, 140, 230],
                    true,
                    "CROSS",
                    2.0,
                    0.0,
                    70,
                ),
                ParcelZoningStyle::new(
                    "ROW",
                    "Right-of-Way (Roads)",
                    "PC-ROW",
                    [140, 150, 160],
                    true,
                    "SOLID",
                    1.0,
                    0.0,
                    85,
                ),
                ParcelZoningStyle::new(
                    "Industrial",
                    "Industrial",
                    "PC-INDUSTRIAL",
                    [160, 80, 220],
                    true,
                    "DOTS",
                    2.0,
                    0.0,
                    65,
                ),
            ],
        }
    }
}

/// Finds the closest matching style rule for a given zoning or style string.
pub fn find_rule_for_zoning<'a>(
    zoning: &str,
    styles: &'a [ParcelZoningStyle],
) -> &'a ParcelZoningStyle {
    let z = zoning.to_ascii_lowercase();
    for rule in styles {
        let k = rule.key.to_ascii_lowercase();
        if z.contains(&k) {
            return rule;
        }
    }
    if z.contains("comm") || z.contains("ret") {
        if let Some(r) = styles.iter().find(|s| s.key.eq_ignore_ascii_case("Commercial")) {
            return r;
        }
    }
    if z.contains("park") || z.contains("green") || z.contains("open") {
        if let Some(r) = styles.iter().find(|s| s.key.eq_ignore_ascii_case("Park")) {
            return r;
        }
    }
    if z.contains("civ") || z.contains("pub") || z.contains("inst") {
        if let Some(r) = styles.iter().find(|s| s.key.eq_ignore_ascii_case("Civic")) {
            return r;
        }
    }
    if z.contains("row") || z.contains("street") || z.contains("road") {
        if let Some(r) = styles.iter().find(|s| s.key.eq_ignore_ascii_case("ROW")) {
            return r;
        }
    }
    if z.contains("ind") {
        if let Some(r) = styles.iter().find(|s| s.key.eq_ignore_ascii_case("Industrial")) {
            return r;
        }
    }
    // Default to first style (usually Residential)
    styles.first().expect("at least one default style exists")
}

/// Resolves the Model Space block record handle from a document.
pub fn get_model_space_handle(doc: &CadDocument) -> Handle {
    if doc.header.model_space_block_handle.is_valid() {
        return doc.header.model_space_block_handle;
    }
    if let Some(h) = doc.objects.values().find_map(|obj| {
        if let codec::objects::ObjectType::Layout(l) = obj {
            if l.name == "Model" && !l.block_record.is_null() {
                Some(l.block_record)
            } else {
                None
            }
        } else {
            None
        }
    }) {
        return h;
    }
    doc.block_records
        .get("*Model_Space")
        .map(|br| br.handle)
        .unwrap_or(Handle::NULL)
}

/// Constructs a valid DXF Hatch EntityType from parcel vertices and style parameters.
pub fn create_parcel_hatch_entity(
    vertices: &[[f64; 2]],
    layer: &str,
    color_rgb: [u8; 3],
    pattern_name: &str,
    scale: f64,
    angle_deg: f64,
    transparency_pct: u8,
    parcel_handle: Handle,
    owner_handle: Handle,
) -> EntityType {
    let mut hatch = Hatch::new();
    hatch.normal = Vector3::new(0.0, 0.0, 1.0);
    hatch.elevation = 0.0;
    let is_solid = pattern_name.eq_ignore_ascii_case("SOLID");
    hatch.is_solid = is_solid;
    hatch.style = HatchStyleType::Normal;
    hatch.pattern_angle = angle_deg.to_radians();
    let pattern_scale = if scale > 0.001 { scale } else { 1.0 };
    hatch.pattern_scale = pattern_scale;

    // Build closed boundary polyline path
    let ring: Vec<Vector2> = vertices.iter().map(|&[x, y]| Vector2::new(x, y)).collect();
    let mut path = BoundaryPath::external();
    path.flags.set_polyline(true);
    path.add_edge(BoundaryEdge::Polyline(PolylineEdge::new(ring, true)));
    hatch.paths.push(path);

    if is_solid {
        hatch.pattern = HatchPattern::solid();
        hatch.pattern_type = HatchPatternType::Predefined;
    } else if !pattern_name.eq_ignore_ascii_case("NONE") {
        let pat_clean = pattern_name.trim().to_ascii_uppercase();
        if let Some(entry) = crate::scene::model::hatch_patterns::find(&pat_clean) {
            if let crate::scene::model::hatch_model::HatchPattern::Pattern(families) = &entry.gpu {
                let mut pat = HatchPattern::new(&pat_clean);
                let rotation = angle_deg.to_radians();
                let (rotation_sin, rotation_cos) = rotation.sin_cos();
                let pattern_origin = vertices.first().copied().unwrap_or([0.0, 0.0]);
                for family in families {
                    let family_angle = (family.angle_deg as f64).to_radians();
                    let angle = family_angle + rotation;
                    let (family_sin, family_cos) = family_angle.sin_cos();
                    let local_offset_x = family.dx as f64 * family_cos - family.dy as f64 * family_sin;
                    let local_offset_y = family.dx as f64 * family_sin + family.dy as f64 * family_cos;
                    let base_x = family.x0 as f64 * pattern_scale;
                    let base_y = family.y0 as f64 * pattern_scale;
                    pat.lines.push(HatchPatternLine {
                        angle,
                        base_point: Vector2::new(
                            pattern_origin[0] + base_x * rotation_cos - base_y * rotation_sin,
                            pattern_origin[1] + base_x * rotation_sin + base_y * rotation_cos,
                        ),
                        offset: Vector2::new(
                            (local_offset_x * rotation_cos - local_offset_y * rotation_sin) * pattern_scale,
                            (local_offset_x * rotation_sin + local_offset_y * rotation_cos) * pattern_scale,
                        ),
                        dash_lengths: family
                            .dashes
                            .iter()
                            .map(|d| *d as f64 * pattern_scale)
                            .collect(),
                    });
                }
                hatch.pattern = pat;
                hatch.pattern_type = HatchPatternType::Predefined;
            }
        }
    }

    let mut ent = EntityType::Hatch(Box::new(hatch));
    let common = ent.common_mut();
    common.layer = layer.to_string();
    common.owner_handle = owner_handle;
    common.color = Color::Rgb {
        r: color_rgb[0],
        g: color_rgb[1],
        b: color_rgb[2],
    };

    let clamped_transparency = (transparency_pct.min(95) as f64) / 100.0;
    common.transparency = Transparency::from_percent(clamped_transparency);

    // Tag hatch with parent parcel handle via XDATA
    let mut xrec = ExtendedDataRecord::new(XDATA_PARCEL_HATCH);
    xrec.add_value(XDataValue::String("PARCEL_HATCH".to_string()));
    xrec.add_value(XDataValue::Handle(parcel_handle));
    let xd = &mut common.extended_data;
    let kept: Vec<_> = xd
        .records()
        .iter()
        .filter(|r| r.application_name != XDATA_PARCEL_HATCH)
        .cloned()
        .collect();
    xd.clear();
    for r in kept {
        xd.add_record(r);
    }
    xd.add_record(xrec);

    ent
}

/// Applies thematic styles and creates/updates hatches for ALL parcels in the document.
pub fn apply_thematic_styles_to_document(
    doc: &mut CadDocument,
    styles: &[ParcelZoningStyle],
) -> (usize, usize) {
    let parcels = ParcelData::all_parcels(doc);
    if parcels.is_empty() {
        return (0, 0);
    }

    let ms_handle = get_model_space_handle(doc);

    // 1. Remove existing parcel hatches
    let to_remove: Vec<Handle> = doc
        .entities()
        .filter_map(|e| {
            if e.common().extended_data.get_record(XDATA_PARCEL_HATCH).is_some() {
                Some(e.common().handle)
            } else {
                None
            }
        })
        .collect();
    for h in to_remove {
        let _ = doc.remove_entity(h);
    }

    let mut parcels_updated = 0;
    let mut hatches_created = 0;

    for p in &parcels {
        let rule = find_rule_for_zoning(&p.zoning, styles);
        let zoning_color = Color::Rgb {
            r: rule.color_rgb[0],
            g: rule.color_rgb[1],
            b: rule.color_rgb[2],
        };

        // Ensure boundary layer exists and has the zoning color
        if let Some(l) = doc.layers.get_mut(&rule.layer) {
            l.color = zoning_color.clone();
        } else {
            let mut l = Layer::new(&rule.layer);
            l.color = zoning_color.clone();
            doc.layers.add_or_replace(l);
        }

        // Update boundary entity layer and color so parcel boundaries take the thematic color immediately
        if let Some(ent) = doc.get_entity_mut(p.handle) {
            ent.common_mut().layer = rule.layer.clone();
            ent.common_mut().color = zoning_color.clone();
            parcels_updated += 1;
        }

        // Create hatch if enabled
        if rule.enable_hatch && !rule.hatch_pattern.eq_ignore_ascii_case("NONE") && p.vertices.len() >= 3 {
            let hatch_layer = format!("{}-HATCH", rule.layer);
            if let Some(hl) = doc.layers.get_mut(&hatch_layer) {
                hl.color = zoning_color.clone();
            } else {
                let mut hl = Layer::new(&hatch_layer);
                hl.color = zoning_color.clone();
                doc.layers.add_or_replace(hl);
            }

            let hatch_ent = create_parcel_hatch_entity(
                &p.vertices,
                &hatch_layer,
                rule.color_rgb,
                &rule.hatch_pattern,
                rule.hatch_scale,
                rule.hatch_angle_deg,
                rule.transparency_pct,
                p.handle,
                ms_handle,
            );
            if let Ok(h_hatch) = doc.add_entity(hatch_ent) {
                hatches_created += 1;
                if ms_handle.is_valid() {
                    if let Some(br) = doc.block_records.iter_mut().find(|b| b.handle == ms_handle) {
                        if !br.entity_handles.is_empty() {
                            br.entity_handles.push(h_hatch);
                        }
                    }
                }
            }
        }
    }

    (parcels_updated, hatches_created)
}

/// Automatically synchronizes hatches for all parcels (used by the Dynamic Reactor).
pub fn sync_parcel_hatches(doc: &mut CadDocument, styles: &[ParcelZoningStyle]) -> usize {
    let parcels = ParcelData::all_parcels(doc);
    if parcels.is_empty() {
        return 0;
    }

    let ms_handle = get_model_space_handle(doc);

    // Collect existing hatch handles and their associated parcel handles
    let mut existing_parcel_hatches: std::collections::HashMap<Handle, Vec<Handle>> =
        std::collections::HashMap::new();

    for ent in doc.entities() {
        if let Some(rec) = ent.common().extended_data.get_record(XDATA_PARCEL_HATCH) {
            if let Some(XDataValue::Handle(p_handle)) = rec.values.get(1) {
                existing_parcel_hatches
                    .entry(*p_handle)
                    .or_default()
                    .push(ent.common().handle);
            }
        }
    }

    let mut created_count = 0;

    for p in &parcels {
        let rule = find_rule_for_zoning(&p.zoning, styles);
        let has_hatch = existing_parcel_hatches.contains_key(&p.handle);

        if !has_hatch && rule.enable_hatch && !rule.hatch_pattern.eq_ignore_ascii_case("NONE") && p.vertices.len() >= 3 {
            let hatch_layer = format!("{}-HATCH", rule.layer);
            let zoning_color = Color::Rgb {
                r: rule.color_rgb[0],
                g: rule.color_rgb[1],
                b: rule.color_rgb[2],
            };
            if let Some(hl) = doc.layers.get_mut(&hatch_layer) {
                hl.color = zoning_color.clone();
            } else {
                let mut hl = Layer::new(&hatch_layer);
                hl.color = zoning_color.clone();
                doc.layers.add_or_replace(hl);
            }

            let hatch_ent = create_parcel_hatch_entity(
                &p.vertices,
                &hatch_layer,
                rule.color_rgb,
                &rule.hatch_pattern,
                rule.hatch_scale,
                rule.hatch_angle_deg,
                rule.transparency_pct,
                p.handle,
                ms_handle,
            );
            if let Ok(h_hatch) = doc.add_entity(hatch_ent) {
                created_count += 1;
                if ms_handle.is_valid() {
                    if let Some(br) = doc.block_records.iter_mut().find(|b| b.handle == ms_handle) {
                        if !br.entity_handles.is_empty() {
                            br.entity_handles.push(h_hatch);
                        }
                    }
                }
            }
        }
    }

    created_count
}

/// Updates the hatch specifically for a single parcel (e.g. from Parcel Properties).
pub fn update_single_parcel_hatch(
    doc: &mut CadDocument,
    parcel_handle: Handle,
    zoning_or_style: &str,
    custom_pattern: &str,
    styles_state: Option<&ParcelStylesDialogState>,
) {
    let ms_handle = get_model_space_handle(doc);

    // 1. Remove old hatch for this parcel
    let to_remove: Vec<Handle> = doc
        .entities()
        .filter_map(|e| {
            if let Some(rec) = e.common().extended_data.get_record(XDATA_PARCEL_HATCH) {
                if let Some(XDataValue::Handle(h)) = rec.values.get(1) {
                    if h == &parcel_handle {
                        return Some(e.common().handle);
                    }
                }
            }
            None
        })
        .collect();
    for h in to_remove {
        let _ = doc.remove_entity(h);
    }

    let Some(parcel) = ParcelData::from_document_entity(doc, parcel_handle) else {
        return;
    };

    if parcel.vertices.len() < 3 {
        return;
    }

    let default_styles = ParcelStylesDialogState::default();
    let styles = styles_state.map(|s| &s.styles).unwrap_or(&default_styles.styles);
    let rule = find_rule_for_zoning(zoning_or_style, styles);

    let pattern = if !custom_pattern.is_empty() && custom_pattern != "DEFAULT" {
        custom_pattern
    } else {
        &rule.hatch_pattern
    };

    if rule.enable_hatch && !pattern.eq_ignore_ascii_case("NONE") {
        let hatch_layer = format!("{}-HATCH", rule.layer);
        let zoning_color = Color::Rgb {
            r: rule.color_rgb[0],
            g: rule.color_rgb[1],
            b: rule.color_rgb[2],
        };
        if let Some(hl) = doc.layers.get_mut(&hatch_layer) {
            hl.color = zoning_color.clone();
        } else {
            let mut hl = Layer::new(&hatch_layer);
            hl.color = zoning_color.clone();
            doc.layers.add_or_replace(hl);
        }

        let hatch_ent = create_parcel_hatch_entity(
            &parcel.vertices,
            &hatch_layer,
            rule.color_rgb,
            pattern,
            rule.hatch_scale,
            rule.hatch_angle_deg,
            rule.transparency_pct,
            parcel.handle,
            ms_handle,
        );
        if let Ok(h_hatch) = doc.add_entity(hatch_ent) {
            if ms_handle.is_valid() {
                if let Some(br) = doc.block_records.iter_mut().find(|b| b.handle == ms_handle) {
                    if !br.entity_handles.is_empty() {
                        br.entity_handles.push(h_hatch);
                    }
                }
            }
        }
    }
}

/// Draws a full Civil 3D formatted Parcel Thematic Legend table on the canvas.
pub fn draw_parcel_zoning_legend(
    doc: &mut CadDocument,
    styles: &[ParcelZoningStyle],
    origin: [f64; 2],
    text_h: f64,
) -> usize {
    let parcels = ParcelData::all_parcels(doc);
    let total_site_area: f64 = parcels.iter().map(|p| p.area_2d).sum();
    let total_lots_count = parcels.len();

    let row_h = text_h * 2.5;
    let col_widths = [
        text_h * 5.0,  // Swatch
        text_h * 16.0, // Zoning Category
        text_h * 10.0, // Hatch Pattern
        text_h * 5.5,  // Lots
        text_h * 11.5, // Total Area (m²)
        text_h * 6.0,  // % of Site
    ];
    let total_w: f64 = col_widths.iter().sum();
    let layer = "PC-LEGEND";
    let ms_handle = get_model_space_handle(doc);

    if !doc.layers.contains(layer) {
        let mut ll = Layer::new(layer);
        ll.color = Color::Index(7);
        doc.layers.add_or_replace(ll);
    }

    let add_line = |doc: &mut CadDocument, p1: [f64; 2], p2: [f64; 2]| {
        let mut l = codec::Line::new();
        l.start = Vector3::new(p1[0], p1[1], 0.0);
        l.end = Vector3::new(p2[0], p2[1], 0.0);
        let mut ent = EntityType::Line(l);
        ent.common_mut().layer = layer.to_string();
        ent.common_mut().owner_handle = ms_handle;
        if let Ok(h) = doc.add_entity(ent) {
            if ms_handle.is_valid() {
                if let Some(br) = doc.block_records.iter_mut().find(|b| b.handle == ms_handle) {
                    if !br.entity_handles.is_empty() {
                        br.entity_handles.push(h);
                    }
                }
            }
        }
    };

    let add_text = |doc: &mut CadDocument, txt: &str, pos: [f64; 2], h: f64, bold: bool| {
        let t = codec::Text::with_value(txt.to_string(), Vector3::new(pos[0], pos[1], 0.0))
            .with_height(h);
        let mut ent = EntityType::Text(t);
        ent.common_mut().layer = layer.to_string();
        ent.common_mut().owner_handle = ms_handle;
        if bold {
            ent.common_mut().color = Color::Index(7); // White / Distinct
        }
        if let Ok(h) = doc.add_entity(ent) {
            if ms_handle.is_valid() {
                if let Some(br) = doc.block_records.iter_mut().find(|b| b.handle == ms_handle) {
                    if !br.entity_handles.is_empty() {
                        br.entity_handles.push(h);
                    }
                }
            }
        }
    };

    let mut cur_y = origin[1];

    // 1. Title Row
    add_line(doc, [origin[0], cur_y], [origin[0] + total_w, cur_y]);
    add_text(
        doc,
        "PARCEL ZONING & LAND USE LEGEND",
        [origin[0] + text_h * 0.8, cur_y - row_h * 0.7],
        text_h * 1.15,
        true,
    );
    cur_y -= row_h;

    // 2. Header Row
    add_line(doc, [origin[0], cur_y], [origin[0] + total_w, cur_y]);
    let headers = ["Swatch", "Zoning Category", "Hatch Fill", "Lots", "Area (m²)", "% Area"];
    let mut cur_x = origin[0];
    for (i, h_txt) in headers.iter().enumerate() {
        add_text(
            doc,
            h_txt,
            [cur_x + text_h * 0.4, cur_y - row_h * 0.7],
            text_h * 0.85,
            true,
        );
        cur_x += col_widths[i];
    }
    cur_y -= row_h;

    let table_top_y = origin[1] - row_h; // where headers start

    // 3. Category Data Rows
    let mut row_count = 0;
    for style in styles {
        // Compute count and area for this category
        let cat_parcels: Vec<&ParcelData> = parcels
            .iter()
            .filter(|p| {
                let z = p.zoning.to_ascii_lowercase();
                let k = style.key.to_ascii_lowercase();
                z.contains(&k)
                    || (k == "residential" && (z.contains("resid") || z.contains("single")))
                    || (k == "commercial" && (z.contains("comm") || z.contains("ret")))
                    || (k == "park" && (z.contains("park") || z.contains("green") || z.contains("open")))
                    || (k == "civic" && (z.contains("civ") || z.contains("pub") || z.contains("inst")))
                    || (k == "row" && (z.contains("row") || z.contains("street") || z.contains("road")))
            })
            .collect();

        let count = cat_parcels.len();
        let cat_area: f64 = cat_parcels.iter().map(|p| p.area_2d).sum();
        let pct = if total_site_area > 1e-4 {
            (cat_area / total_site_area) * 100.0
        } else {
            0.0
        };

        add_line(doc, [origin[0], cur_y], [origin[0] + total_w, cur_y]);

        // Draw Swatch in col 0
        let s_x1 = origin[0] + text_h * 0.4;
        let s_x2 = origin[0] + col_widths[0] - text_h * 0.4;
        let s_y1 = cur_y - text_h * 0.35;
        let s_y2 = cur_y - row_h + text_h * 0.35;

        // Swatch outline
        add_line(doc, [s_x1, s_y1], [s_x2, s_y1]);
        add_line(doc, [s_x2, s_y1], [s_x2, s_y2]);
        add_line(doc, [s_x2, s_y2], [s_x1, s_y2]);
        add_line(doc, [s_x1, s_y2], [s_x1, s_y1]);

        // Swatch hatch fill
        if style.enable_hatch && !style.hatch_pattern.eq_ignore_ascii_case("NONE") {
            let swatch_verts = vec![[s_x1, s_y1], [s_x2, s_y1], [s_x2, s_y2], [s_x1, s_y2]];
            let hatch_ent = create_parcel_hatch_entity(
                &swatch_verts,
                layer,
                style.color_rgb,
                &style.hatch_pattern,
                0.6,
                style.hatch_angle_deg,
                style.transparency_pct,
                Handle::NULL,
                ms_handle,
            );
            if let Ok(h_hatch) = doc.add_entity(hatch_ent) {
                if ms_handle.is_valid() {
                    if let Some(br) = doc.block_records.iter_mut().find(|b| b.handle == ms_handle) {
                        if !br.entity_handles.is_empty() {
                            br.entity_handles.push(h_hatch);
                        }
                    }
                }
            }
        }

        // Column 1: Category Name
        let mut x_pos = origin[0] + col_widths[0];
        add_text(
            doc,
            &style.name,
            [x_pos + text_h * 0.4, cur_y - row_h * 0.7],
            text_h * 0.85,
            false,
        );

        // Column 2: Hatch Pattern
        x_pos += col_widths[1];
        let pat_str = if !style.enable_hatch || style.hatch_pattern.eq_ignore_ascii_case("NONE") {
            "Empty".to_string()
        } else {
            format!("{} ({}%)", style.hatch_pattern, style.transparency_pct)
        };
        add_text(
            doc,
            &pat_str,
            [x_pos + text_h * 0.4, cur_y - row_h * 0.7],
            text_h * 0.80,
            false,
        );

        // Column 3: Count
        x_pos += col_widths[2];
        let cnt_str = if count > 0 { format!("{}", count) } else { "-".to_string() };
        add_text(
            doc,
            &cnt_str,
            [x_pos + text_h * 0.4, cur_y - row_h * 0.7],
            text_h * 0.85,
            false,
        );

        // Column 4: Area
        x_pos += col_widths[3];
        let area_str = if cat_area > 0.0 {
            format!("{:.2}", cat_area)
        } else {
            "-".to_string()
        };
        add_text(
            doc,
            &area_str,
            [x_pos + text_h * 0.4, cur_y - row_h * 0.7],
            text_h * 0.85,
            false,
        );

        // Column 5: %
        x_pos += col_widths[4];
        let pct_str = if count > 0 { format!("{:.1}%", pct) } else { "-".to_string() };
        add_text(
            doc,
            &pct_str,
            [x_pos + text_h * 0.4, cur_y - row_h * 0.7],
            text_h * 0.85,
            false,
        );

        cur_y -= row_h;
        row_count += 1;
    }

    // 4. Summary Row (Totals)
    add_line(doc, [origin[0], cur_y], [origin[0] + total_w, cur_y]);
    add_text(
        doc,
        "TOTAL SITE SUMMARY",
        [origin[0] + col_widths[0] + text_h * 0.4, cur_y - row_h * 0.7],
        text_h * 0.9,
        true,
    );

    let mut sum_x = origin[0] + col_widths[0] + col_widths[1] + col_widths[2];
    add_text(
        doc,
        &format!("{}", total_lots_count),
        [sum_x + text_h * 0.4, cur_y - row_h * 0.7],
        text_h * 0.9,
        true,
    );

    sum_x += col_widths[3];
    add_text(
        doc,
        &format!("{:.2}", total_site_area),
        [sum_x + text_h * 0.4, cur_y - row_h * 0.7],
        text_h * 0.9,
        true,
    );

    sum_x += col_widths[4];
    add_text(
        doc,
        "100.0%",
        [sum_x + text_h * 0.4, cur_y - row_h * 0.7],
        text_h * 0.9,
        true,
    );

    cur_y -= row_h;
    add_line(doc, [origin[0], cur_y], [origin[0] + total_w, cur_y]);

    // 5. Vertical Lines
    let table_bottom_y = cur_y;
    // Outer left and right
    add_line(doc, [origin[0], origin[1]], [origin[0], table_bottom_y]);
    add_line(doc, [origin[0] + total_w, origin[1]], [origin[0] + total_w, table_bottom_y]);

    // Internal column dividers (from header down to bottom)
    let mut v_x = origin[0];
    for w in &col_widths[..col_widths.len() - 1] {
        v_x += w;
        add_line(doc, [v_x, table_top_y], [v_x, table_bottom_y]);
    }

    row_count
}

/// Outlined group box matching Civil 3D aesthetics.
fn group_box<'a>(
    title: impl Into<Cow<'a, str>>,
    body: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(
        column![
            text(title.into())
                .size(11)
                .style(|theme: &Theme| text::Style {
                    color: Some(theme.palette().background.strongest.text.scale_alpha(0.85)),
                }),
            body.into(),
        ]
        .spacing(8),
    )
    .padding([8, 12])
    .width(Fill)
    .style(|theme: &Theme| container::Style {
        border: Border {
            width: 1.0,
            radius: 4.0.into(),
            color: theme.palette().background.strong.color,
        },
        ..Default::default()
    })
    .into()
}

/// Renders the Civil 3D Parcel Thematic Zoning & Styles dialog.
pub fn view_window<'a>(
    state: &'a ParcelStylesDialogState,
    _sizing: crate::ui::modal::ModalSizing,
) -> Element<'a, Message> {
    let sel_idx = state.selected_index.min(state.styles.len().saturating_sub(1));
    let cur_style = &state.styles[sel_idx];

    // 1. Header Information & Description
    let header_text = column![
        text(lang_str(
            "Parcel Zoning, Colors & Thematic Hatching Manager",
            "إدارة أنماط وتخصيص استخدامات الأراضي والتهشيرات",
            "Gestionnaire de styles et hachures de zonage"
        ))
        .size(14)
        .style(|theme: &Theme| text::Style {
            color: Some(theme.palette().primary.strong.color),
        }),
        text(lang_str(
            "Customize colors, transparent fills, and hatch patterns for each cadastral land-use category. Changes update dynamically across the drawing.",
            "تخصيص الألوان والتهشيرات ونسب الشفافية لكل فئة عمرانية، مع تحديث المخطط ديناميكياً وإخراج مفتاح الخريطة (Legend).",
            "Personnalisez les couleurs, le remplissage transparent et les hachures pour chaque catégorie de zonage."
        ))
        .size(11)
        .style(crate::ui::style::common::muted_style),
    ]
    .spacing(4);

    // 2. Left Styles List (Categories)
    let mut style_buttons = column![].spacing(6);
    for (i, st) in state.styles.iter().enumerate() {
        let is_selected = i == sel_idx;
        let swatch_box = container(Space::new().width(18).height(18))
            .style({
                let col = st.iced_color();
                move |_theme: &Theme| container::Style {
                    background: Some(Background::Color(col)),
                    border: Border {
                        width: 1.0,
                        radius: 3.0.into(),
                        color: IcedColor::WHITE,
                    },
                    ..Default::default()
                }
            });

        let cat_label = column![
            text(st.name.clone()).size(11.5),
            text(format!("{} • {}", st.layer, if st.enable_hatch { &st.hatch_pattern } else { "No Hatch" }))
                .size(9.5)
                .style(crate::ui::style::common::muted_style),
        ]
        .spacing(1);

        let row_content = row![swatch_box, cat_label]
            .spacing(8)
            .align_y(iced::Alignment::Center);

        let btn = button(row_content)
            .width(Fill)
            .padding([5, 8])
            .style(if is_selected { button::primary } else { button::secondary })
            .on_press(Message::ParcelStylesSelectStyle(i));

        style_buttons = style_buttons.push(btn);
    }

    let styles_list_col = column![
        text(lang_str("Zoning Categories:", "فئات التخصيص والاستخدام:", "Catégories de zonage:")).size(11),
        scrollable(style_buttons).height(Length::Fixed(240.0)),
    ]
    .spacing(6)
    .width(Length::Fixed(240.0));

    // 3. Right Details Editor Panel for `cur_style`
    let hatch_check = row![
        checkbox(cur_style.enable_hatch)
            .on_toggle(move |v| Message::ParcelStylesToggleHatch(sel_idx, v))
            .size(14),
        text(lang_str("Enable Hatch Fill / Interior Shading", "تفعيل التهشير والتظليل الداخلي", "Activer la hachure"))
            .size(11.5),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    // Pattern picker
    let pat_pick = pick_list(
        Some(cur_style.hatch_pattern.clone()),
        HATCH_PATTERNS.iter().map(|&s| s.to_string()).collect::<Vec<_>>(),
        |s| s.to_string(),
    )
    .on_select(move |p| Message::ParcelStylesPatternChanged(sel_idx, p))
    .text_size(11)
    .padding([3, 6])
    .width(160);

    let pat_row = row![
        text(lang_str("Hatch Pattern:", "نمط التهشير:", "Motif:")).size(11).width(120),
        pat_pick,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    // Quick Color Chips
    let mut color_chips = row![].spacing(5).align_y(iced::Alignment::Center);
    for &(_name, rgb) in COLOR_PALETTE {
        let is_cur = cur_style.color_rgb == rgb;
        let chip = button(Space::new().width(18).height(18))
            .padding(0)
            .style(move |_theme: &Theme, _status| button::Style {
                background: Some(Background::Color(IcedColor::from_rgb(
                    rgb[0] as f32 / 255.0,
                    rgb[1] as f32 / 255.0,
                    rgb[2] as f32 / 255.0,
                ))),
                border: Border {
                    width: if is_cur { 2.5 } else { 1.0 },
                    radius: 3.0.into(),
                    color: if is_cur { IcedColor::WHITE } else { IcedColor::from_rgb(0.3, 0.3, 0.3) },
                },
                ..Default::default()
            })
            .on_press(Message::ParcelStylesColorPreset(sel_idx, rgb));
        color_chips = color_chips.push(chip);
    }

    let color_row = row![
        text(lang_str("Category Color:", "لون الفئة والطبقة:", "Couleur:")).size(11).width(120),
        color_chips,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    // Transparency Slider
    let transp_slider = slider(0..=95, cur_style.transparency_pct, move |val| {
        Message::ParcelStylesTransparencyChanged(sel_idx, val)
    })
    .width(180);

    let transp_row = row![
        text(lang_str("Transparency:", "الشفافية:", "Transparence:")).size(11).width(120),
        transp_slider,
        text(format!("{}%", cur_style.transparency_pct)).size(11),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    // Scale Input
    let scale_inp = text_input("Scale", &format!("{:.1}", cur_style.hatch_scale))
        .on_input(move |s| Message::ParcelStylesScaleChanged(sel_idx, s))
        .size(11)
        .padding([3, 6])
        .width(80);

    let scale_row = row![
        text(lang_str("Pattern Scale:", "مقياس التهشير:", "Échelle du motif:")).size(11).width(120),
        scale_inp,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let editor_col = column![
        text(format!("⚙ {}", cur_style.name)).size(12).style(|theme: &Theme| text::Style {
            color: Some(theme.palette().primary.strong.color),
        }),
        hatch_check,
        pat_row,
        color_row,
        transp_row,
        scale_row,
    ]
    .spacing(10)
    .width(Fill);

    let settings_box = group_box(
        lang_str("Category Display Configuration", "إعدادات العرض والتهشير للفئة المحددة", "Configuration d'affichage"),
        editor_col,
    );

    let main_body = row![styles_list_col, settings_box]
        .spacing(14)
        .width(Fill);

    // 4. Status and Actions Toolbar
    let legend_btn = dialog_button(
        lang_str("▦ Insert Legend Table", "▦ إخراج مفتاح الخريطة (Legend)", "▦ Insérer la légende"),
        Message::ParcelStylesInsertLegend,
        false,
    );

    let pick_lot_btn = dialog_button(
        lang_str("🎯 Pick Lot to Apply", "🎯 تطبيق بالتقاط قطعة", "🎯 Appliquer au lot"),
        Message::ParcelStylesPickLot,
        false,
    );

    let apply_all_btn = dialog_button(
        lang_str("✔ Update Drawing Dynamically", "✔ تحديث الرسم ديناميكياً", "✔ Mettre à jour dynamiquement"),
        Message::ParcelStylesApplyAll,
        true,
    );

    let close_btn = dialog_button(
        lang_str("Close", "إغلاق", "Fermer"),
        Message::ParcelStylesClose,
        false,
    );

    let buttons_row = row![
        legend_btn,
        pick_lot_btn,
        Space::new().width(Fill),
        apply_all_btn,
        close_btn,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    column![
        header_text,
        Space::new().height(4),
        main_body,
        Space::new().height(6),
        buttons_row,
    ]
    .spacing(8)
    .padding(12)
    .width(Fill)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use codec::CadDocument;

    #[test]
    fn test_hatch_model_conversion() {
        let verts = vec![
            [0.0, 0.0],
            [10.0, 0.0],
            [10.0, 10.0],
            [0.0, 10.0],
        ];
        let ent = create_parcel_hatch_entity(
            &verts,
            "PC-RESIDENTIAL-HATCH",
            [235, 185, 50],
            "SOLID",
            1.0,
            0.0,
            80,
            Handle::NULL,
            Handle::NULL,
        );
        let EntityType::Hatch(ht) = ent else { panic!("not a hatch") };
        let model = crate::scene::Scene::hatch_model_from_dxf(&ht, [1.0, 0.5, 0.0, 1.0]);
        assert!(model.is_some(), "hatch_model_from_dxf must succeed for created hatch!");
    }

    #[test]
    fn test_parcel_styles_default_state() {
        let state = ParcelStylesDialogState::default();
        assert_eq!(state.styles.len(), 6);
        assert_eq!(state.styles[0].key, "Residential");
        assert_eq!(state.styles[1].key, "Commercial");
        assert_eq!(state.styles[2].key, "Park");
        assert_eq!(state.styles[3].key, "Civic");
        assert_eq!(state.styles[4].key, "ROW");
        assert_eq!(state.styles[5].key, "Industrial");
        assert!(state.auto_apply_to_new);
    }

    #[test]
    fn test_find_rule_for_zoning() {
        let state = ParcelStylesDialogState::default();
        assert_eq!(find_rule_for_zoning("Single-Family", &state.styles).key, "Residential");
        assert_eq!(find_rule_for_zoning("Commercial Retail", &state.styles).key, "Commercial");
        assert_eq!(find_rule_for_zoning("Public Green Park", &state.styles).key, "Park");
        assert_eq!(find_rule_for_zoning("Civic Center", &state.styles).key, "Civic");
        assert_eq!(find_rule_for_zoning("Main Street ROW", &state.styles).key, "ROW");
    }

    #[test]
    fn test_create_parcel_hatch_and_apply_to_document() {
        let mut doc = CadDocument::new();
        let poly1 = codec::LwPolyline {
            vertices: vec![
                codec::entities::LwVertex::from_coords(0.0, 0.0),
                codec::entities::LwVertex::from_coords(20.0, 0.0),
                codec::entities::LwVertex::from_coords(20.0, 20.0),
                codec::entities::LwVertex::from_coords(0.0, 20.0),
            ],
            is_closed: true,
            ..Default::default()
        };
        let mut ent1 = EntityType::LwPolyline(poly1);
        ent1.common_mut().layer = "PC-PARCEL".to_string();
        let mut rec1 = ExtendedDataRecord::new(crate::app::commands::civil_parcel::XDATA_PARCEL);
        rec1.add_value(XDataValue::String("Lot 101".to_string()));
        rec1.add_value(XDataValue::String("Site 1".to_string()));
        rec1.add_value(XDataValue::String("101".to_string()));
        rec1.add_value(XDataValue::String("Commercial".to_string()));
        ent1.common_mut().extended_data.add_record(rec1);
        let h1 = doc.add_entity(ent1).unwrap();

        let poly2 = codec::LwPolyline {
            vertices: vec![
                codec::entities::LwVertex::from_coords(30.0, 0.0),
                codec::entities::LwVertex::from_coords(50.0, 0.0),
                codec::entities::LwVertex::from_coords(50.0, 20.0),
                codec::entities::LwVertex::from_coords(30.0, 20.0),
            ],
            is_closed: true,
            ..Default::default()
        };
        let mut ent2 = EntityType::LwPolyline(poly2);
        ent2.common_mut().layer = "PC-PARCEL".to_string();
        let mut rec2 = ExtendedDataRecord::new(crate::app::commands::civil_parcel::XDATA_PARCEL);
        rec2.add_value(XDataValue::String("Lot 102".to_string()));
        rec2.add_value(XDataValue::String("Site 1".to_string()));
        rec2.add_value(XDataValue::String("102".to_string()));
        rec2.add_value(XDataValue::String("Residential".to_string()));
        ent2.common_mut().extended_data.add_record(rec2);
        let h2 = doc.add_entity(ent2).unwrap();

        let state = ParcelStylesDialogState::default();
        let (updated, hatches) = apply_thematic_styles_to_document(&mut doc, &state.styles);
        assert_eq!(updated, 2);
        assert_eq!(hatches, 2);

        // Verify layer assignment
        assert_eq!(doc.get_entity(h1).unwrap().common().layer, "PC-COMMERCIAL");
        assert_eq!(doc.get_entity(h2).unwrap().common().layer, "PC-RESIDENTIAL");

        // Verify hatches created with XDATA
        let hatch_count = doc.entities().filter(|e| e.common().extended_data.get_record(XDATA_PARCEL_HATCH).is_some()).count();
        assert_eq!(hatch_count, 2);

        // Test legend generation
        let rows = draw_parcel_zoning_legend(&mut doc, &state.styles, [100.0, 100.0], 2.5);
        assert_eq!(rows, 6);

        // Verify legend entities created on PC-LEGEND
        let legend_entities = doc.entities().filter(|e| e.common().layer == "PC-LEGEND").count();
        assert!(legend_entities > 10, "Legend table should contain borders, text and swatches");
    }

    #[test]
    fn test_update_single_parcel_hatch() {
        let mut doc = CadDocument::new();
        let poly = codec::LwPolyline {
            vertices: vec![
                codec::entities::LwVertex::from_coords(0.0, 0.0),
                codec::entities::LwVertex::from_coords(10.0, 0.0),
                codec::entities::LwVertex::from_coords(10.0, 10.0),
                codec::entities::LwVertex::from_coords(0.0, 10.0),
            ],
            is_closed: true,
            ..Default::default()
        };
        let mut ent = EntityType::LwPolyline(poly);
        let mut rec = ExtendedDataRecord::new(crate::app::commands::civil_parcel::XDATA_PARCEL);
        rec.add_value(XDataValue::String("Lot 201".to_string()));
        rec.add_value(XDataValue::String("Site 1".to_string()));
        rec.add_value(XDataValue::String("201".to_string()));
        rec.add_value(XDataValue::String("Single-Family".to_string()));
        ent.common_mut().extended_data.add_record(rec);
        let h = doc.add_entity(ent).unwrap();

        // Update single hatch to ANSI31
        update_single_parcel_hatch(&mut doc, h, "Commercial", "ANSI31", None);
        let hatches: Vec<_> = doc.entities().filter(|e| e.common().extended_data.get_record(XDATA_PARCEL_HATCH).is_some()).collect();
        assert_eq!(hatches.len(), 1);
        if let EntityType::Hatch(ht) = &hatches[0] {
            assert_eq!(ht.pattern.name, "ANSI31");
            let model = crate::scene::Scene::hatch_model_from_dxf(ht, [1.0; 4]);
            assert!(model.is_some(), "Hatch model must convert successfully from DXF");
        } else {
            panic!("Expected Hatch entity");
        }
    }
}
