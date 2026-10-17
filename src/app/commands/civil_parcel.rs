use codec::xdata::{ExtendedDataRecord, XDataValue};
use codec::types::Vector3;
use codec::{CadDocument, EntityType, Handle};

pub const XDATA_PARCEL: &str = "OPENCAD_PARCEL";
#[allow(dead_code)]
pub const LAYER_PARCEL: &str = "PC-PARCEL";
#[allow(dead_code)]
pub const LAYER_RESIDENTIAL: &str = "PC-RESIDENTIAL";
#[allow(dead_code)]
pub const LAYER_COMMERCIAL: &str = "PC-COMMERCIAL";
#[allow(dead_code)]
pub const LAYER_CIVIC: &str = "PC-CIVIC";
#[allow(dead_code)]
pub const LAYER_PARK: &str = "PC-PARK";

/// Geometric and topological data extracted from a Parcel entity in the CAD drawing.
#[derive(Debug, Clone, PartialEq)]
pub struct ParcelData {
    pub handle: Handle,
    pub name: String,
    pub number: u32,
    pub site_name: String,
    pub zoning: String,
    pub owner: String,
    pub deed_no: String,
    pub tax_id: String,
    pub layer: String,

    // Geometric properties
    pub area_2d: f64,
    pub perimeter: f64,
    pub centroid: [f64; 2],
    pub bbox_min: [f64; 2],
    pub bbox_max: [f64; 2],
    pub vertices: Vec<[f64; 2]>,
    pub segment_count: usize,
    pub is_closed: bool,

    // Mapcheck verification
    pub mapcheck_closure_error: f64,
    pub mapcheck_bearing_deg: f64,
    pub mapcheck_precision: f64,
}

/// Helper to identify valid parcel polygon layers (excluding setbacks, labels, tables, etc.)
pub fn is_parcel_layer(layer: &str) -> bool {
    let l = layer.to_ascii_uppercase();
    if l.contains("LABEL")
        || l.contains("TABLE")
        || l.contains("SETBACK")
        || l.contains("SEGMENT")
        || l.contains("ROAD")
        || l.contains("EASEMENT")
        || l.contains("ALIGNMENT")
    {
        return false;
    }
    l == "PC-PARCEL"
        || l.starts_with("PC-PARCEL")
        || l.starts_with("PC-RESIDENTIAL")
        || l.starts_with("PC-COMMERCIAL")
        || l.starts_with("PC-CIVIC")
        || l.starts_with("PC-PARK")
        || l.contains("PARCEL")
        || l.contains("LOT")
}

/// Helper to verify if an entity is a valid parcel polygon
pub fn is_parcel_entity(ent: &EntityType) -> bool {
    if let EntityType::LwPolyline(lw) = ent {
        if !lw.is_closed && lw.vertices.len() < 3 {
            return false;
        }
        let layer = &ent.common().layer;
        is_parcel_layer(layer)
    } else {
        false
    }
}

impl ParcelData {
    /// Attempt to construct a `ParcelData` from an arbitrary entity in the CAD document.
    pub fn from_entity(entity: &EntityType) -> Option<Self> {
        let EntityType::LwPolyline(lw) = entity else {
            return None;
        };

        let handle = entity.common().handle;
        let layer = entity.common().layer.clone();

        // Must have at least 3 vertices to form a polygon parcel
        if lw.vertices.len() < 3 {
            return None;
        }

        let vertices: Vec<[f64; 2]> = lw
            .vertices
            .iter()
            .map(|v| [v.location.x, v.location.y])
            .collect();

        let n = vertices.len();
        let is_closed = lw.is_closed;

        // Compute 2D Area (Shoelace formula)
        let mut area_sum = 0.0;
        let mut cx_sum = 0.0;
        let mut cy_sum = 0.0;
        let mut perimeter = 0.0;
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        let mut delta_x_sum = 0.0;
        let mut delta_y_sum = 0.0;

        for i in 0..n {
            let j = (i + 1) % n;
            let (x0, y0) = (vertices[i][0], vertices[i][1]);
            let (x1, y1) = (vertices[j][0], vertices[j][1]);

            min_x = min_x.min(x0);
            min_y = min_y.min(y0);
            max_x = max_x.max(x0);
            max_y = max_y.max(y0);

            let cross = x0 * y1 - x1 * y0;
            area_sum += cross;
            cx_sum += (x0 + x1) * cross;
            cy_sum += (y0 + y1) * cross;

            let dx = x1 - x0;
            let dy = y1 - y0;
            let seg_len = (dx * dx + dy * dy).sqrt();
            perimeter += seg_len;

            delta_x_sum += dx;
            delta_y_sum += dy;
        }

        let area_2d = (area_sum.abs() * 0.5).max(0.0);
        let centroid = if area_sum.abs() > 1e-9 {
            let factor = 1.0 / (3.0 * area_sum);
            [cx_sum * factor, cy_sum * factor]
        } else {
            [(min_x + max_x) * 0.5, (min_y + max_y) * 0.5]
        };

        // Mapcheck closure analysis
        let closure_error = (delta_x_sum * delta_x_sum + delta_y_sum * delta_y_sum).sqrt();
        let mapcheck_bearing_deg = delta_y_sum.atan2(delta_x_sum).to_degrees();
        let mapcheck_precision = if closure_error > 1e-7 {
            perimeter / closure_error
        } else {
            500_000.0 // Near perfect closure
        };

        // Extract XDATA attributes if attached
        let (name, number, site_name, zoning, owner, deed_no, tax_id) =
            if let Some(xrec) = entity.common().extended_data.get_record(XDATA_PARCEL) {
                let s = |idx: usize, default: &str| -> String {
                    match xrec.values.get(idx) {
                        Some(XDataValue::String(val)) => val.clone(),
                        _ => default.to_string(),
                    }
                };
                let n_str = s(2, "1");
                let parsed_num = n_str.parse::<u32>().unwrap_or(1);
                (
                    s(0, &format!("Lot {:X}", handle.value())),
                    parsed_num,
                    s(1, "Site 1"),
                    s(3, "Single-Family Residential (R-1)"),
                    s(4, "Standard Property Owner"),
                    s(5, "D-2026-001"),
                    s(6, "042-881-01"),
                )
            } else {
                // Infer from layer or defaults
                let default_zoning = if layer.contains("RESIDENTIAL") {
                    "Single-Family Residential (R-1)"
                } else if layer.contains("COMMERCIAL") {
                    "Commercial / Retail (C-1)"
                } else if layer.contains("CIVIC") {
                    "Civic / Institutional (PUB)"
                } else if layer.contains("PARK") {
                    "Parks & Open Space (REC)"
                } else {
                    "Single-Family Residential (R-1)"
                };

                (
                    format!("Lot {:X}", handle.value()),
                    1,
                    "Site 1".to_string(),
                    default_zoning.to_string(),
                    "Standard Property Owner".to_string(),
                    "D-2026-001".to_string(),
                    "042-881-01".to_string(),
                )
            };

        Some(Self {
            handle,
            name,
            number,
            site_name,
            zoning,
            owner,
            deed_no,
            tax_id,
            layer,
            area_2d,
            perimeter,
            centroid,
            bbox_min: [min_x, min_y],
            bbox_max: [max_x, max_y],
            vertices,
            segment_count: n,
            is_closed,
            mapcheck_closure_error: closure_error,
            mapcheck_bearing_deg,
            mapcheck_precision,
        })
    }

    /// Retrieve a parcel from a document by its handle.
    pub fn from_document_entity(doc: &CadDocument, handle: Handle) -> Option<Self> {
        let ent = doc.entities().find(|e| e.common().handle == handle)?;
        Self::from_entity(ent)
    }

    /// Discover all parcels present in the drawing.
    pub fn all_parcels(doc: &CadDocument) -> Vec<Self> {
        let mut parcels: Vec<Self> = doc.entities()
            .filter_map(|ent| {
                if let EntityType::LwPolyline(lw) = ent {
                    if !lw.is_closed && lw.vertices.len() < 3 {
                        return None;
                    }
                    let layer = &ent.common().layer;
                    if is_parcel_layer(layer) {
                        Self::from_entity(ent)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        // Infer parcel names from existing PC-PARCEL-LABEL text entities if XDATA is absent!
        let label_texts: Vec<(String, [f64; 2])> = doc.entities()
            .filter_map(|ent| {
                let layer = &ent.common().layer;
                if layer.eq_ignore_ascii_case("PC-PARCEL-LABEL") || layer.starts_with("PC-PARCEL-LABEL") {
                    let (val, pt) = match ent {
                        EntityType::Text(t) => (t.value.trim().to_string(), [t.insertion_point.x, t.insertion_point.y]),
                        EntityType::MText(m) => (m.value.trim().to_string(), [m.insertion_point.x, m.insertion_point.y]),
                        _ => return None,
                    };
                    let is_name_candidate = !val.contains("m²")
                        && !val.contains("m\u{b2}")
                        && !val.contains("m\u{fffd}")
                        && !val.contains("m2")
                        && !val.contains("ac")
                        && !val.starts_with("P:")
                        && !val.starts_with("P :")
                        && !val.to_ascii_lowercase().starts_with("perim");
                    if is_name_candidate {
                        return Some((val, pt));
                    }
                }
                None
            })
            .collect();

        for p in &mut parcels {
            // If the name is default "Lot {:X}", try to find existing label inside parcel bbox
            if p.name.starts_with("Lot ") {
                let mut best_label = None;
                let mut min_d = f64::INFINITY;
                for (val, pt) in &label_texts {
                    if pt[0] >= p.bbox_min[0] - 5.0 && pt[0] <= p.bbox_max[0] + 5.0
                        && pt[1] >= p.bbox_min[1] - 5.0 && pt[1] <= p.bbox_max[1] + 5.0
                    {
                        let dx = pt[0] - p.centroid[0];
                        let dy = pt[1] - p.centroid[1];
                        let d = dx * dx + dy * dy;
                        if d < min_d {
                            min_d = d;
                            best_label = Some(val.clone());
                        }
                    }
                }
                if let Some(lbl) = best_label {
                    p.name = lbl;
                }
            }
        }

        parcels
    }

    /// Update parcel attributes and write XDATA back to the document entity.
    pub fn update_entity_xdata(
        doc: &mut CadDocument,
        handle: Handle,
        name: &str,
        number: u32,
        site: &str,
        zoning: &str,
        owner: &str,
        deed: &str,
        tax_id: &str,
        layer: &str,
    ) -> bool {
        let Some(ent) = doc.get_entity_mut(handle) else {
            return false;
        };

        if !layer.is_empty() {
            ent.common_mut().layer = layer.to_string();
        }

        let mut rec = ExtendedDataRecord::new(XDATA_PARCEL);
        rec.add_value(XDataValue::String(name.to_string()));
        rec.add_value(XDataValue::String(site.to_string()));
        rec.add_value(XDataValue::String(number.to_string()));
        rec.add_value(XDataValue::String(zoning.to_string()));
        rec.add_value(XDataValue::String(owner.to_string()));
        rec.add_value(XDataValue::String(deed.to_string()));
        rec.add_value(XDataValue::String(tax_id.to_string()));

        let xd = &mut ent.common_mut().extended_data;
        let kept: Vec<_> = xd
            .records()
            .iter()
            .filter(|r| r.application_name != XDATA_PARCEL)
            .cloned()
            .collect();
        xd.clear();
        for r in kept {
            xd.add_record(r);
        }
        xd.add_record(rec);
        true
    }

    /// Dynamically synchronizes parcel centroid labels (`PC-PARCEL-LABEL`) and table rows (`PC-TABLE`)
    /// across the CAD document when parcel properties or geometry are modified.
    pub fn sync_dynamic_parcels(
        doc: &mut CadDocument,
        target_handle: Option<Handle>,
        old_name: Option<&str>,
    ) -> usize {
        // 0. Clean up accidental XDATA on non-parcel entities (such as PC-SETBACK)
        for ent in doc.entities_mut() {
            let layer = &ent.common().layer;
            if !is_parcel_layer(layer) {
                let has_xdata = ent.common().extended_data.get_record(XDATA_PARCEL).is_some();
                if has_xdata {
                    let kept: Vec<_> = ent.common().extended_data.records().iter()
                        .filter(|r| r.application_name != XDATA_PARCEL)
                        .cloned()
                        .collect();
                    ent.common_mut().extended_data.clear();
                    for r in kept {
                        ent.common_mut().extended_data.add_record(r);
                    }
                }
            }
        }

        let all = Self::all_parcels(doc);
        let parcels_to_sync: Vec<Self> = if let Some(th) = target_handle {
            all.into_iter().filter(|p| p.handle == th).collect()
        } else {
            all
        };

        if parcels_to_sync.is_empty() {
            return 0;
        }

        let mut updated_total = 0;

        for parcel in &parcels_to_sync {
            let p_old_name = old_name.unwrap_or(&parcel.name);
            let mut labels_updated = 0;
            let mut table_updated = false;

            // Ensure polyline entity has XDATA_PARCEL with parcel.name
            let has_xdata = doc.get_entity(parcel.handle)
                .map(|e| e.common().extended_data.get_record(XDATA_PARCEL).is_some())
                .unwrap_or(false);
            if !has_xdata {
                Self::update_entity_xdata(
                    doc,
                    parcel.handle,
                    &parcel.name,
                    parcel.number,
                    &parcel.site_name,
                    &parcel.zoning,
                    &parcel.owner,
                    &parcel.deed_no,
                    &parcel.tax_id,
                    &parcel.layer,
                );
            }

            // ── 1. Update Labels on PC-PARCEL-LABEL ───────────────────
            let label_candidates: Vec<(Handle, String, [f64; 2])> = doc
                .entities()
                .filter_map(|ent| {
                    let layer = &ent.common().layer;
                    if layer.eq_ignore_ascii_case("PC-PARCEL-LABEL")
                        || layer.starts_with("PC-PARCEL-LABEL")
                    {
                        let (text_val, pt) = match ent {
                            EntityType::Text(t) => (t.value.trim().to_string(), [t.insertion_point.x, t.insertion_point.y]),
                            EntityType::MText(m) => (m.value.trim().to_string(), [m.insertion_point.x, m.insertion_point.y]),
                            _ => return None,
                        };

                        let label_parcel_tag = ent
                            .common()
                            .extended_data
                            .get_record("OPENCAD_PARCEL_LABEL")
                            .and_then(|r| r.values.first())
                            .and_then(|v| match v {
                                codec::xdata::XDataValue::String(s) => Some(s.as_str()),
                                _ => None,
                            });

                        if let Some(tag) = label_parcel_tag {
                            if tag != p_old_name && tag != parcel.name.as_str() {
                                return None; // Belongs to a different parcel!
                            }
                        }

                        if (text_val.starts_with("LOT-") || text_val.starts_with("Lot "))
                            && text_val != p_old_name
                            && text_val != parcel.name.as_str()
                        {
                            return None; // Name of another lot!
                        }

                        let xdata_match = label_parcel_tag.is_some_and(|s| s == p_old_name || s == parcel.name.as_str());
                        let text_match = text_val == p_old_name || text_val == parcel.name.as_str();

                        let dx = pt[0] - parcel.centroid[0];
                        let dy = pt[1] - parcel.centroid[1];
                        let dist = (dx * dx + dy * dy).sqrt();

                        let max_dim = (parcel.bbox_max[0] - parcel.bbox_min[0])
                            .max(parcel.bbox_max[1] - parcel.bbox_min[1])
                            .max(30.0);

                        let in_bbox = pt[0] >= parcel.bbox_min[0] - 6.0
                            && pt[0] <= parcel.bbox_max[0] + 6.0
                            && pt[1] >= parcel.bbox_min[1] - 6.0
                            && pt[1] <= parcel.bbox_max[1] + 6.0;

                        if xdata_match || text_match || (in_bbox && dist < max_dim) {
                            return Some((
                                ent.common().handle,
                                text_val,
                                pt,
                            ));
                        }
                    }
                    None
                })
                .collect();

            let text_h = 2.2;
            let line_spacing = text_h * 1.35;
            let cx = parcel.centroid[0];
            let cy = parcel.centroid[1];

            let area_str = format!(
                "{:.1} m² ({:.3} ac)",
                parcel.area_2d,
                parcel.area_2d * 0.000247105
            );
            let perim_str = format!("P: {:.2} m", parcel.perimeter);

            if label_candidates.is_empty() {
                // If this parcel doesn't have centroid labels yet, create all three dynamically!
                let mut t1 = codec::entities::Text::with_value(
                    parcel.name.clone(),
                    Vector3::new(cx, cy + line_spacing, 0.0),
                );
                t1.height = text_h;
                let mut ent1 = EntityType::Text(t1);
                ent1.common_mut().layer = "PC-PARCEL-LABEL".to_string();
                let mut rec1 = ExtendedDataRecord::new("OPENCAD_PARCEL_LABEL");
                rec1.add_value(XDataValue::String(parcel.name.clone()));
                rec1.add_value(XDataValue::String(parcel.name.clone()));
                ent1.common_mut().extended_data.add_record(rec1);
                let _ = doc.add_entity(ent1);

                let mut t2 = codec::entities::Text::with_value(
                    area_str.clone(),
                    Vector3::new(cx, cy, 0.0),
                );
                t2.height = text_h * 0.9;
                let mut ent2 = EntityType::Text(t2);
                ent2.common_mut().layer = "PC-PARCEL-LABEL".to_string();
                let mut rec2 = ExtendedDataRecord::new("OPENCAD_PARCEL_LABEL");
                rec2.add_value(XDataValue::String(parcel.name.clone()));
                rec2.add_value(XDataValue::String(area_str.clone()));
                ent2.common_mut().extended_data.add_record(rec2);
                let _ = doc.add_entity(ent2);

                let mut t3 = codec::entities::Text::with_value(
                    perim_str.clone(),
                    Vector3::new(cx, cy - line_spacing, 0.0),
                );
                t3.height = text_h * 0.85;
                let mut ent3 = EntityType::Text(t3);
                ent3.common_mut().layer = "PC-PARCEL-LABEL".to_string();
                let mut rec3 = ExtendedDataRecord::new("OPENCAD_PARCEL_LABEL");
                rec3.add_value(XDataValue::String(parcel.name.clone()));
                rec3.add_value(XDataValue::String(perim_str.clone()));
                ent3.common_mut().extended_data.add_record(rec3);
                let _ = doc.add_entity(ent3);

                labels_updated += 3;
            } else {
                let mut has_perim = false;
                let mut has_area = false;
                let mut has_name = false;

                for (lh, val, _) in &label_candidates {
                    let Some(ent) = doc.get_entity_mut(*lh) else {
                        continue;
                    };

                    let val_trim = val.trim();
                    let is_explicit_name = val_trim == p_old_name
                        || val_trim == &parcel.name
                        || (val_trim.to_ascii_uppercase().starts_with("LOT") && !val_trim.contains("m"));

                    let is_explicit_perim = val_trim.starts_with("P:")
                        || val_trim.starts_with("P :")
                        || val_trim.to_ascii_lowercase().starts_with("perim");

                    let is_explicit_area = !is_explicit_name && !is_explicit_perim && (
                        val_trim.contains("m²")
                            || val_trim.contains("m\u{b2}")
                            || val_trim.contains("m\u{fffd}")
                            || val_trim.contains("m2")
                            || val_trim.contains("ac")
                            || val_trim.contains("Acres")
                            || val_trim.contains("sq")
                            || val_trim.starts_with("A:")
                            || val_trim.starts_with("A :")
                            || val_trim.ends_with('m')
                            || (val_trim.chars().any(|c| c.is_ascii_digit()) && !val_trim.starts_with("LOT"))
                    );

                    let new_val;
                    let new_pos;
                    if is_explicit_name || (!has_name && !is_explicit_area && !is_explicit_perim) {
                        new_val = parcel.name.clone();
                        new_pos = Vector3::new(cx, cy + line_spacing, 0.0);
                        has_name = true;
                    } else if is_explicit_area || (!has_area && !is_explicit_perim) {
                        new_val = area_str.clone();
                        new_pos = Vector3::new(cx, cy, 0.0);
                        has_area = true;
                    } else if is_explicit_perim || !has_perim {
                        new_val = perim_str.clone();
                        new_pos = Vector3::new(cx, cy - line_spacing, 0.0);
                        has_perim = true;
                    } else {
                        continue;
                    }

                    match ent {
                        EntityType::Text(t) => {
                            t.value = new_val.clone();
                            t.insertion_point = new_pos;
                        }
                        EntityType::MText(m) => {
                            m.value = new_val.clone();
                            m.insertion_point = new_pos;
                        }
                        _ => {}
                    }

                    let mut rec = ExtendedDataRecord::new("OPENCAD_PARCEL_LABEL");
                    rec.add_value(XDataValue::String(parcel.name.clone()));
                    rec.add_value(XDataValue::String(new_val));
                    let xd = &mut ent.common_mut().extended_data;
                    let kept: Vec<_> = xd
                        .records()
                        .iter()
                        .filter(|r| r.application_name != "OPENCAD_PARCEL_LABEL")
                        .cloned()
                        .collect();
                    xd.clear();
                    for r in kept {
                        xd.add_record(r);
                    }
                    xd.add_record(rec);

                    labels_updated += 1;
                }

                // If existing labels did NOT include a perimeter label, synthesize it dynamically!
                if !has_perim {
                    let mut t3 = codec::entities::Text::with_value(
                        perim_str.clone(),
                        Vector3::new(cx, cy - line_spacing, 0.0),
                    );
                    t3.height = text_h * 0.85;
                    let mut ent3 = EntityType::Text(t3);
                    ent3.common_mut().layer = "PC-PARCEL-LABEL".to_string();
                    let mut rec3 = ExtendedDataRecord::new("OPENCAD_PARCEL_LABEL");
                    rec3.add_value(XDataValue::String(parcel.name.clone()));
                    rec3.add_value(XDataValue::String(perim_str.clone()));
                    ent3.common_mut().extended_data.add_record(rec3);
                    let _ = doc.add_entity(ent3);
                    labels_updated += 1;
                }
            }

            // ── 2. Update Table Rows on PC-TABLE ──────────────────────
            let table_texts: Vec<(Handle, String, f64, f64)> = doc
                .entities()
                .filter_map(|ent| {
                    let layer = &ent.common().layer;
                    if layer.eq_ignore_ascii_case("PC-TABLE")
                        || layer.starts_with("PC-TABLE")
                    {
                        match ent {
                            EntityType::Text(t) => Some((
                                ent.common().handle,
                                t.value.clone(),
                                t.insertion_point.x,
                                t.insertion_point.y,
                            )),
                            EntityType::MText(m) => Some((
                                ent.common().handle,
                                m.value.clone(),
                                m.insertion_point.x,
                                m.insertion_point.y,
                            )),
                            _ => None,
                        }
                    } else {
                        None
                    }
                })
                .collect();

            // Cluster table text rows by Y coordinate (within 1.0 unit tolerance)
            let mut clustered_rows: Vec<Vec<(Handle, String, f64, f64)>> = Vec::new();
            for item in table_texts {
                let ty = item.3;
                if let Some(existing_row) = clustered_rows
                    .iter_mut()
                    .find(|row| (row[0].3 - ty).abs() <= 1.0)
                {
                    existing_row.push(item);
                } else {
                    clustered_rows.push(vec![item]);
                }
            }

            let update_text_cell = |doc: &mut CadDocument, h: Handle, val: String| {
                if let Some(ent) = doc.get_entity_mut(h) {
                    match ent {
                        EntityType::Text(t) => t.value = val,
                        EntityType::MText(m) => m.value = val,
                        _ => {}
                    }
                }
            };

            for mut cells in clustered_rows {
                cells.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
                if cells.len() >= 4 {
                    let col0_val = cells[0].1.trim();
                    if col0_val == p_old_name || col0_val == &parcel.name {
                        // Col 0: Lot Name
                        update_text_cell(doc, cells[0].0, parcel.name.clone());
                        // Col 1: Area (m²)
                        if let Some(c1) = cells.get(1) {
                            update_text_cell(doc, c1.0, format!("{:.2}", parcel.area_2d));
                        }
                        // Col 2: Area (Acres)
                        if let Some(c2) = cells.get(2) {
                            update_text_cell(doc, c2.0, format!("{:.3}", parcel.area_2d * 0.000247105));
                        }
                        // Col 3: Perimeter (m)
                        if let Some(c3) = cells.get(3) {
                            update_text_cell(doc, c3.0, format!("{:.2}", parcel.perimeter));
                        }
                        // Col 4: Zoning
                        if let Some(c4) = cells.get(4) {
                            if !parcel.zoning.is_empty() {
                                update_text_cell(doc, c4.0, parcel.zoning.clone());
                            }
                        }
                        // Col 5: Owner
                        if let Some(c5) = cells.get(5) {
                            if !parcel.owner.is_empty() {
                                update_text_cell(doc, c5.0, parcel.owner.clone());
                            }
                        }
                        table_updated = true;
                    }
                }
            }

            // Also check for Line Table titles: "LINE TABLE - {p_old_name}"
            for ent in doc.entities_mut() {
                if ent.common().layer.eq_ignore_ascii_case("PC-TABLE") {
                    let text_val = match ent {
                        EntityType::Text(t) => Some(&mut t.value),
                        EntityType::MText(m) => Some(&mut m.value),
                        _ => None,
                    };
                    if let Some(val) = text_val {
                        if val.starts_with("LINE TABLE - ")
                            && (val.ends_with(p_old_name) || val.ends_with(&parcel.name))
                        {
                            *val = format!("LINE TABLE - {}", parcel.name);
                            table_updated = true;
                        }
                    }
                }
            }

            if labels_updated > 0 || table_updated {
                updated_total += 1;
            }
        }

        updated_total
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codec::entities::{LwPolyline, Text};
    use codec::types::{Vector2, Vector3};

    #[test]
    fn test_dynamic_parcel_reactor_vertex_modification() {
        let mut doc = CadDocument::new();

        // 1. Create a 20x50m rectangular parcel (Area = 1000 m², Perimeter = 140 m)
        let mut poly = LwPolyline::from_points(vec![
            Vector2::new(0.0, 0.0),
            Vector2::new(20.0, 0.0),
            Vector2::new(20.0, 50.0),
            Vector2::new(0.0, 50.0),
        ]);
        poly.is_closed = true;
        let mut ent_poly = EntityType::LwPolyline(poly);
        ent_poly.common_mut().layer = "PC-PARCEL".to_string();
        let poly_handle = doc.add_entity(ent_poly).unwrap();

        // 2. Add centroid labels (Name, Area)
        let t_name = Text::with_value("LOT-TEST-1", Vector3::new(10.0, 28.0, 0.0));
        let mut ent_name = EntityType::Text(t_name);
        ent_name.common_mut().layer = "PC-PARCEL-LABEL".to_string();
        doc.add_entity(ent_name).unwrap();

        let t_area = Text::with_value("1000.0 m² (0.247 ac)", Vector3::new(10.0, 25.0, 0.0));
        let mut ent_area = EntityType::Text(t_area);
        ent_area.common_mut().layer = "PC-PARCEL-LABEL".to_string();
        doc.add_entity(ent_area).unwrap();

        // 3. Add table row
        let t_col0 = Text::with_value("LOT-TEST-1", Vector3::new(100.0, 50.0, 0.0));
        let mut ent_col0 = EntityType::Text(t_col0);
        ent_col0.common_mut().layer = "PC-TABLE".to_string();
        doc.add_entity(ent_col0).unwrap();

        let t_col1 = Text::with_value("1000.00", Vector3::new(115.0, 50.0, 0.0));
        let mut ent_col1 = EntityType::Text(t_col1);
        ent_col1.common_mut().layer = "PC-TABLE".to_string();
        let col1_h = doc.add_entity(ent_col1).unwrap();

        let t_col2 = Text::with_value("0.247", Vector3::new(130.0, 50.0, 0.0));
        let mut ent_col2 = EntityType::Text(t_col2);
        ent_col2.common_mut().layer = "PC-TABLE".to_string();
        doc.add_entity(ent_col2).unwrap();

        let t_col3 = Text::with_value("140.00", Vector3::new(145.0, 50.0, 0.0));
        let mut ent_col3 = EntityType::Text(t_col3);
        ent_col3.common_mut().layer = "PC-TABLE".to_string();
        let col3_h = doc.add_entity(ent_col3).unwrap();

        // 4. Modify a vertex: stretch width from 20m to 30m!
        // New area = 30 * 50 = 1500 m², New perimeter = 2 * (30 + 50) = 160 m
        if let Some(EntityType::LwPolyline(lw)) = doc.get_entity_mut(poly_handle) {
            lw.vertices[1].location.x = 30.0;
            lw.vertices[2].location.x = 30.0;
        }

        // 5. Trigger dynamic synchronization
        let updated = ParcelData::sync_dynamic_parcels(&mut doc, None, None);
        assert!(updated > 0, "Expected parcels to be updated");

        // 6. Verify Table Row was updated
        if let Some(EntityType::Text(t)) = doc.get_entity(col1_h) {
            assert_eq!(t.value, "1500.00", "Table area should update dynamically to 1500.00");
        } else {
            panic!("Col 1 entity not found");
        }

        if let Some(EntityType::Text(t)) = doc.get_entity(col3_h) {
            assert_eq!(t.value, "160.00", "Table perimeter should update dynamically to 160.00");
        } else {
            panic!("Col 3 entity not found");
        }

        // 7. Verify Labels were updated dynamically & perimeter was synthesized!
        let mut found_area = false;
        let mut found_perim = false;
        for ent in doc.entities() {
            if ent.common().layer == "PC-PARCEL-LABEL" {
                if let EntityType::Text(t) = ent {
                    if t.value.contains("1500.0 m²") {
                        found_area = true;
                    }
                    if t.value == "P: 160.00 m" {
                        found_perim = true;
                    }
                }
            }
        }
        assert!(found_area, "Centroid area label must update dynamically to 1500.0 m²");
        assert!(found_perim, "Centroid perimeter label must update dynamically to P: 160.00 m");
    }
}


