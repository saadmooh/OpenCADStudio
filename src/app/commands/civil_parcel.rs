use codec::xdata::{ExtendedDataRecord, XDataValue};
use codec::types::Vector3;
use codec::entities::mtext::AttachmentPoint;
use codec::entities::{MText, Text, TextHorizontalAlignment, TextVerticalAlignment};
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

    // Label styling & synchronization
    pub area_label_style: String,
    pub line_label_style: String,
    pub curve_label_style: String,
    pub auto_add_segment_labels: bool,
    pub segment_layer: String,

    // Geometric properties
    pub area_2d: f64,
    pub perimeter: f64,
    pub centroid: [f64; 2],
    pub bbox_min: [f64; 2],
    pub bbox_max: [f64; 2],
    pub vertices: Vec<[f64; 2]>,
    pub bulges: Vec<f64>,
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
        || l.contains("PROP-LINE")
        || l.contains("PROP_LINE")
    {
        return false;
    }
    l == "PC-PARCEL"
        || l.starts_with("PC-PARCEL")
        || l.starts_with("PC-RESIDENTIAL")
        || l.starts_with("PC-COMMERCIAL")
        || l.starts_with("PC-CIVIC")
        || l.starts_with("PC-PARK")
        || l == "C-PROP"
        || l.starts_with("C-PROP")
        || l.starts_with("PROP")
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
        let l = layer.to_ascii_uppercase();
        if l.contains("SETBACK")
            || l.contains("LABEL")
            || l.contains("TABLE")
            || l.contains("SEGMENT")
            || l.contains("ROAD")
            || l.contains("PROP-LINE")
            || l.contains("PROP_LINE")
        {
            return false;
        }
        ent.common().extended_data.get_record(XDATA_PARCEL).is_some() || is_parcel_layer(layer)
    } else {
        false
    }
}


/// Computes 2D signed area of a polygon loop using the Shoelace formula.
pub fn calc_polygon_area(pts: &[[f64; 2]]) -> f64 {
    let n = pts.len();
    if n < 3 {
        return 0.0;
    }
    let mut area = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        area += pts[i][0] * pts[j][1] - pts[j][0] * pts[i][1];
    }
    0.5 * area
}

/// Point-in-polygon ray casting test (strictly interior or on boundary).
pub fn is_point_inside_polygon(pt: [f64; 2], poly: &[[f64; 2]]) -> bool {
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    for i in 0..n {
        let j = (i + 1) % n;
        let xi = poly[i][0];
        let yi = poly[i][1];
        let xj = poly[j][0];
        let yj = poly[j][1];

        let denom = yj - yi;
        let intersect = ((yi > pt[1]) != (yj > pt[1]))
            && (pt[0] < (xj - xi) * (pt[1] - yi) / (if denom.abs() < 1e-12 { 1e-12 * denom.signum() } else { denom }) + xi);
        if intersect {
            inside = !inside;
        }
    }
    inside
}

/// 2D Segment-segment intersection test returning (t, u, [x, y]).
pub fn segment_intersection(
    p1: [f64; 2],
    p2: [f64; 2],
    q1: [f64; 2],
    q2: [f64; 2],
) -> Option<(f64, f64, [f64; 2])> {
    let d = (p2[0] - p1[0]) * (q2[1] - q1[1]) - (p2[1] - p1[1]) * (q2[0] - q1[0]);
    if d.abs() < 1e-11 {
        return None;
    }
    let t = ((q1[0] - p1[0]) * (q2[1] - q1[1]) - (q1[1] - p1[1]) * (q2[0] - q1[0])) / d;
    let u = ((q1[0] - p1[0]) * (p2[1] - p1[1]) - (q1[1] - p1[1]) * (p2[0] - p1[0])) / d;
    if t > 1e-6 && t < (1.0 - 1e-6) && u > 1e-6 && u < (1.0 - 1e-6) {
        let x = p1[0] + t * (p2[0] - p1[0]);
        let y = p1[1] + t * (p2[1] - p1[1]);
        Some((t, u, [x, y]))
    } else {
        None
    }
}

/// Projects point `p` onto line segment `a`->`b`.
pub fn project_point_to_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let ab2 = ab[0] * ab[0] + ab[1] * ab[1];
    if ab2 < 1e-12 {
        return a;
    }
    let ap = [p[0] - a[0], p[1] - a[1]];
    let t = ((ap[0] * ab[0] + ap[1] * ab[1]) / ab2).clamp(0.0, 1.0);
    [a[0] + t * ab[0], a[1] + t * ab[1]]
}

/// Outward normal offset of polygon loop to ensure clean transverse intersections.
pub fn inflate_polygon(poly: &[[f64; 2]], delta: f64) -> Vec<[f64; 2]> {
    let n = poly.len();
    if n < 3 {
        return poly.to_vec();
    }
    let mut normals = Vec::with_capacity(n);
    for i in 0..n {
        let j = (i + 1) % n;
        let dx = poly[j][0] - poly[i][0];
        let dy = poly[j][1] - poly[i][1];
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-9 {
            normals.push([0.0, 0.0]);
        } else {
            normals.push([dy / len, -dx / len]);
        }
    }

    let mut inflated = Vec::with_capacity(n);
    for i in 0..n {
        let prev = if i == 0 { n - 1 } else { i - 1 };
        let n1 = normals[prev];
        let n2 = normals[i];
        let mut dir = [n1[0] + n2[0], n1[1] + n2[1]];
        let len = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
        if len < 1e-5 {
            dir = n2;
        } else {
            dir[0] /= len;
            dir[1] /= len;
        }
        let dot = (dir[0] * n2[0] + dir[1] * n2[1]).max(0.1);
        let miter = (delta / dot).min(delta * 4.0);
        inflated.push([poly[i][0] + dir[0] * miter, poly[i][1] + dir[1] * miter]);
    }
    inflated
}

/// Computes the 2D polygon difference `subject \ cutter`.
/// Returns the list of remaining closed polygon loops (each CCW oriented).
pub fn polygon_difference(
    subject_raw: &[[f64; 2]],
    cutter_raw: &[[f64; 2]],
) -> Vec<Vec<[f64; 2]>> {
    if subject_raw.len() < 3 || cutter_raw.len() < 3 {
        return vec![subject_raw.to_vec()];
    }

    let mut subject = subject_raw.to_vec();
    if calc_polygon_area(&subject) < 0.0 {
        subject.reverse();
    }
    let mut cutter = cutter_raw.to_vec();
    if calc_polygon_area(&cutter) < 0.0 {
        cutter.reverse();
    }

    let delta = 0.001; // 1 mm outward perturbation to resolve collinear edges and vertex-on-edge contacts
    let cutter_inflated = inflate_polygon(&cutter, delta);

    let all_inside = subject.iter().all(|&p| is_point_inside_polygon(p, &cutter_inflated));
    if all_inside {
        return Vec::new();
    }

    let n_s = subject.len();
    let n_c = cutter_inflated.len();

    let mut intersections = Vec::new();
    for i in 0..n_s {
        let p1 = subject[i];
        let p2 = subject[(i + 1) % n_s];
        for j in 0..n_c {
            let q1 = cutter_inflated[j];
            let q2 = cutter_inflated[(j + 1) % n_c];
            if let Some((t, u, pt)) = segment_intersection(p1, p2, q1, q2) {
                intersections.push((i, t, j, u, pt));
            }
        }
    }

    if intersections.is_empty() {
        return vec![subject];
    }

    use std::collections::BTreeMap;
    let mut s_by_edge: BTreeMap<usize, Vec<(f64, usize, [f64; 2])>> = BTreeMap::new();
    let mut c_by_edge: BTreeMap<usize, Vec<(f64, usize, [f64; 2])>> = BTreeMap::new();

    for (int_id, &(i, t, j, u, pt)) in intersections.iter().enumerate() {
        s_by_edge.entry(i).or_default().push((t, int_id, pt));
        c_by_edge.entry(j).or_default().push((u, int_id, pt));
    }

    for list in s_by_edge.values_mut() {
        list.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    }
    for list in c_by_edge.values_mut() {
        list.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    }

    #[derive(Clone, Debug)]
    struct Node {
        pt: [f64; 2],
        is_int: bool,
        int_id: usize,
        is_entry: bool,
    }

    let mut new_s: Vec<Node> = Vec::new();
    for i in 0..n_s {
        new_s.push(Node {
            pt: subject[i],
            is_int: false,
            int_id: 0,
            is_entry: false,
        });
        if let Some(list) = s_by_edge.get(&i) {
            for &(_, int_id, pt) in list {
                new_s.push(Node {
                    pt,
                    is_int: true,
                    int_id,
                    is_entry: false,
                });
            }
        }
    }

    let mut new_c: Vec<Node> = Vec::new();
    for j in 0..n_c {
        new_c.push(Node {
            pt: cutter_inflated[j],
            is_int: false,
            int_id: 0,
            is_entry: false,
        });
        if let Some(list) = c_by_edge.get(&j) {
            for &(_, int_id, pt) in list {
                new_c.push(Node {
                    pt,
                    is_int: true,
                    int_id,
                    is_entry: false,
                });
            }
        }
    }

    let len_s = new_s.len();
    for i in 0..len_s {
        if new_s[i].is_int {
            let next_idx = (i + 1) % len_s;
            let mid = [
                (new_s[i].pt[0] + new_s[next_idx].pt[0]) * 0.5,
                (new_s[i].pt[1] + new_s[next_idx].pt[1]) * 0.5,
            ];
            new_s[i].is_entry = is_point_inside_polygon(mid, &cutter_inflated);
        }
    }

    let mut c_lookup = rustc_hash::FxHashMap::default();
    for (j, node) in new_c.iter().enumerate() {
        if node.is_int {
            c_lookup.insert(node.int_id, j);
        }
    }

    let mut s_lookup = rustc_hash::FxHashMap::default();
    for (i, node) in new_s.iter().enumerate() {
        if node.is_int {
            s_lookup.insert(node.int_id, i);
        }
    }

    let len_c = new_c.len();
    let mut visited_s = vec![false; len_s];
    let mut visited_c = vec![false; len_c];
    let mut result_polygons = Vec::new();

    let mut start_candidates: Vec<usize> = new_s
        .iter()
        .enumerate()
        .filter(|(_, node)| !node.is_int && !is_point_inside_polygon(node.pt, &cutter_inflated))
        .map(|(i, _)| i)
        .collect();

    if start_candidates.is_empty() {
        start_candidates = new_s
            .iter()
            .enumerate()
            .filter(|(_, node)| node.is_int && !node.is_entry)
            .map(|(i, _)| i)
            .collect();
    }

    for start_i in start_candidates {
        if visited_s[start_i] {
            continue;
        }

        let mut poly: Vec<[f64; 2]> = Vec::new();
        let mut curr_i = start_i;
        let mut curr_on_s = true;
        let mut step = 0;

        while step < 2000 {
            step += 1;
            if curr_on_s {
                visited_s[curr_i] = true;
                poly.push(new_s[curr_i].pt);
                if new_s[curr_i].is_int && new_s[curr_i].is_entry {
                    if let Some(&c_idx) = c_lookup.get(&new_s[curr_i].int_id) {
                        curr_on_s = false;
                        curr_i = if c_idx == 0 { len_c - 1 } else { c_idx - 1 };
                    } else {
                        curr_i = (curr_i + 1) % len_s;
                    }
                } else {
                    curr_i = (curr_i + 1) % len_s;
                }
            } else {
                visited_c[curr_i] = true;
                poly.push(new_c[curr_i].pt);
                if new_c[curr_i].is_int {
                    if let Some(&s_idx) = s_lookup.get(&new_c[curr_i].int_id) {
                        curr_on_s = true;
                        curr_i = (s_idx + 1) % len_s;
                    } else {
                        curr_i = if curr_i == 0 { len_c - 1 } else { curr_i - 1 };
                    }
                } else {
                    curr_i = if curr_i == 0 { len_c - 1 } else { curr_i - 1 };
                }
            }

            if curr_on_s && curr_i == start_i {
                break;
            }
        }

        // Snap vertices that lie on/near the cutter back to the exact original cutter boundary
        let mut snapped: Vec<[f64; 2]> = Vec::with_capacity(poly.len());
        for p in poly {
            let mut best_pt = p;
            let mut min_d = f64::INFINITY;
            for j in 0..cutter.len() {
                let q1 = cutter[j];
                let q2 = cutter[(j + 1) % cutter.len()];
                let proj = project_point_to_segment(p, q1, q2);
                let d = ((p[0] - proj[0]).powi(2) + (p[1] - proj[1]).powi(2)).sqrt();
                if d < min_d {
                    min_d = d;
                    best_pt = proj;
                }
            }
            if min_d <= 0.005 {
                snapped.push(best_pt);
            } else {
                snapped.push(p);
            }
        }

        let mut clean: Vec<[f64; 2]> = Vec::new();
        for p in snapped {
            if clean.is_empty() {
                clean.push(p);
            } else {
                let last = clean.last().unwrap();
                let dx = p[0] - last[0];
                let dy = p[1] - last[1];
                if (dx * dx + dy * dy).sqrt() > 1e-4 {
                    clean.push(p);
                }
            }
        }

        if clean.len() >= 4 {
            let first = clean.first().unwrap();
            let last = clean.last().unwrap();
            let dx = first[0] - last[0];
            let dy = first[1] - last[1];
            if (dx * dx + dy * dy).sqrt() < 1e-4 {
                clean.pop();
            }
        }

        if clean.len() >= 3 && calc_polygon_area(&clean).abs() > 0.01 {
            result_polygons.push(clean);
        }
    }

    if result_polygons.is_empty() && !all_inside {
        vec![subject]
    } else {
        result_polygons
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

        let bulges: Vec<f64> = lw
            .vertices
            .iter()
            .map(|v| v.bulge)
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
        let (name, number, site_name, zoning, owner, deed_no, tax_id, area_label_style, line_label_style, curve_label_style, auto_add_segment_labels, segment_layer) =
            if let Some(xrec) = entity.common().extended_data.get_record(XDATA_PARCEL) {
                let s = |idx: usize, default: &str| -> String {
                    match xrec.values.get(idx) {
                        Some(XDataValue::String(val)) => val.clone(),
                        _ => default.to_string(),
                    }
                };
                let n_str = s(2, "1");
                let parsed_num = n_str.parse::<u32>().unwrap_or(1);
                let auto_labels = s(10, "true") != "false";
                (
                    s(0, &format!("Lot {:X}", handle.value())),
                    parsed_num,
                    s(1, "Site 1"),
                    s(3, "Single-Family Residential (R-1)"),
                    s(4, "Standard Property Owner"),
                    s(5, "D-2026-001"),
                    s(6, "042-881-01"),
                    s(7, "Parcel Name, Area & Perimeter"),
                    s(8, "Bearing over Distance"),
                    s(9, "Distance over Radius"),
                    auto_labels,
                    s(11, "C-PROP-LINE"),
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
                    "Parcel Name, Area & Perimeter".to_string(),
                    "Bearing over Distance".to_string(),
                    "Distance over Radius".to_string(),
                    true,
                    "C-PROP-LINE".to_string(),
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
            area_label_style,
            line_label_style,
            curve_label_style,
            auto_add_segment_labels,
            segment_layer,
            area_2d,
            perimeter,
            centroid,
            bbox_min: [min_x, min_y],
            bbox_max: [max_x, max_y],
            vertices,
            bulges,
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
                    if is_parcel_entity(ent) {
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
            // If the name is default "Lot {:X}", try to find existing label inside parcel polygon
            let default_hex_name = format!("Lot {:X}", p.handle.value());
            if p.name == default_hex_name {
                let mut best_label = None;
                let mut min_d = f64::INFINITY;
                for (val, pt) in &label_texts {
                    if p.contains_point(*pt) {
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

    /// Calculate the next available lot number (101, 102, 103...) by scanning
    /// all existing parcels and parcel labels in the document.
    pub fn next_available_lot_number(doc: &CadDocument) -> u32 {
        let mut found_any = false;
        let mut max_num = 0u32;
        let parcels = Self::all_parcels(doc);
        for p in &parcels {
            found_any = true;
            if p.number > max_num {
                max_num = p.number;
            }
            for part in p.name.split(|c: char| !c.is_ascii_digit()) {
                if let Ok(num) = part.parse::<u32>() {
                    if num > max_num {
                        max_num = num;
                    }
                }
            }
        }
        for ent in doc.entities() {
            if let Some(rec) = ent.common().extended_data.get_record(XDATA_PARCEL) {
                found_any = true;
                if let Some(codec::xdata::XDataValue::String(s)) = rec.values.get(2) {
                    if let Ok(num) = s.parse::<u32>() {
                        if num > max_num {
                            max_num = num;
                        }
                    }
                }
                if let Some(codec::xdata::XDataValue::String(s)) = rec.values.first() {
                    for part in s.split(|c: char| !c.is_ascii_digit()) {
                        if let Ok(num) = part.parse::<u32>() {
                            if num > max_num {
                                max_num = num;
                            }
                        }
                    }
                }
            }
        }
        if !found_any || max_num < 100 {
            if found_any && max_num > 0 {
                max_num + 1
            } else {
                101
            }
        } else {
            max_num + 1
        }
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
        let (area_style, line_style, curve_style, auto_labels, seg_layer) = {
            if let Some(ent) = doc.get_entity(handle) {
                if let Some(r) = ent.common().extended_data.get_record(XDATA_PARCEL) {
                    let s = |idx: usize, def: &str| -> String {
                        r.values.get(idx).and_then(|v| match v {
                            XDataValue::String(val) => Some(val.clone()),
                            _ => None,
                        }).unwrap_or_else(|| def.to_string())
                    };
                    (
                        s(7, "Parcel Name, Area & Perimeter"),
                        s(8, "Bearing over Distance"),
                        s(9, "Distance over Radius"),
                        s(10, "true") == "true",
                        s(11, "C-PROP-LINE"),
                    )
                } else {
                    ("Parcel Name, Area & Perimeter".into(), "Bearing over Distance".into(), "Distance over Radius".into(), true, "C-PROP-LINE".into())
                }
            } else {
                return false;
            }
        };

        Self::update_entity_xdata_full(
            doc,
            handle,
            name,
            number,
            site,
            zoning,
            owner,
            deed,
            tax_id,
            layer,
            &area_style,
            &line_style,
            &curve_style,
            auto_labels,
            &seg_layer,
        )
    }

    /// Update all 12 parcel attributes (including Civil 3D styles) and write XDATA back to the document entity.
    pub fn update_entity_xdata_full(
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
        area_label_style: &str,
        line_label_style: &str,
        curve_label_style: &str,
        auto_add_segment_labels: bool,
        segment_layer: &str,
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
        rec.add_value(XDataValue::String(area_label_style.to_string()));
        rec.add_value(XDataValue::String(line_label_style.to_string()));
        rec.add_value(XDataValue::String(curve_label_style.to_string()));
        rec.add_value(XDataValue::String(if auto_add_segment_labels { "true".to_string() } else { "false".to_string() }));
        rec.add_value(XDataValue::String(segment_layer.to_string()));

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

    /// If the given handle corresponds to a parcel label (centroid label on PC-PARCEL-LABEL
    /// or segment label on C-PROP-LINE / PC-SEGMENT-LABEL), resolves and returns the handle
    /// of the underlying Parcel boundary polyline.
    pub fn resolve_parcel_from_label(doc: &CadDocument, label_handle: Handle) -> Option<Handle> {
        let ent = doc.get_entity(label_handle)?;
        if !matches!(ent, EntityType::Text(_) | EntityType::MText(_)) {
            return None;
        }

        let layer = &ent.common().layer;
        let l = layer.to_ascii_uppercase();
        let is_label_layer = l.contains("LABEL") || l.contains("PROP-LINE") || l.contains("PROP_LINE");

        // 1. Check OPENCAD_PARCEL_LABEL
        let name_opt = ent.common().extended_data.get_record("OPENCAD_PARCEL_LABEL")
            .and_then(|r| r.values.first())
            .and_then(|v| match v {
                codec::xdata::XDataValue::String(s) => Some(s.clone()),
                _ => None,
            });

        // 2. Check OPENCAD_PARCEL_SEGMENT_LABEL
        let seg_name_opt = ent.common().extended_data.get_record("OPENCAD_PARCEL_SEGMENT_LABEL")
            .and_then(|r| r.values.first())
            .and_then(|v| match v {
                codec::xdata::XDataValue::String(s) => Some(s.clone()),
                _ => None,
            });

        let target_name = name_opt.or(seg_name_opt);
        let all = Self::all_parcels(doc);

        if let Some(target) = target_name {
            if let Some(p) = all.iter().find(|p| p.name == target) {
                return Some(p.handle);
            }
        }

        // 3. Fallback for untagged text on parcel label layers: locate by point containment
        if is_label_layer {
            let pt = match ent {
                EntityType::Text(t) => [t.insertion_point.x, t.insertion_point.y],
                EntityType::MText(m) => [m.insertion_point.x, m.insertion_point.y],
                _ => return None,
            };
            for p in &all {
                if p.contains_point(pt) {
                    return Some(p.handle);
                }
            }
        }

        None
    }

    /// Computes the safe interior width and height for text placement
    /// inside the parcel polygon around its centroid.
    pub fn interior_clearance(&self) -> (f64, f64) {
        let cx = self.centroid[0];
        let cy = self.centroid[1];
        let n = self.vertices.len();
        let bbox_w = (self.bbox_max[0] - self.bbox_min[0]).max(2.0);
        let bbox_h = (self.bbox_max[1] - self.bbox_min[1]).max(2.0);

        if n < 3 {
            return ((bbox_w * 0.78).max(2.0), (bbox_h * 0.75).max(2.0));
        }

        // Horizontal ray intersection with polygon edges at y = cy
        let mut x_left = f64::NEG_INFINITY;
        let mut x_right = f64::INFINITY;

        // Vertical ray intersection with polygon edges at x = cx
        let mut y_bottom = f64::NEG_INFINITY;
        let mut y_top = f64::INFINITY;

        for i in 0..n {
            let j = (i + 1) % n;
            let (x0, y0) = (self.vertices[i][0], self.vertices[i][1]);
            let (x1, y1) = (self.vertices[j][0], self.vertices[j][1]);

            // Check horizontal line y = cy
            let min_y = y0.min(y1);
            let max_y = y0.max(y1);
            if cy >= min_y && cy <= max_y && (max_y - min_y).abs() > 1e-9 {
                let t = (cy - y0) / (y1 - y0);
                let x_int = x0 + t * (x1 - x0);
                if x_int <= cx + 1e-6 {
                    x_left = x_left.max(x_int);
                }
                if x_int >= cx - 1e-6 {
                    x_right = x_right.min(x_int);
                }
            }

            // Check vertical line x = cx
            let min_x = x0.min(x1);
            let max_x = x0.max(x1);
            if cx >= min_x && cx <= max_x && (max_x - min_x).abs() > 1e-9 {
                let t = (cx - x0) / (x1 - x0);
                let y_int = y0 + t * (y1 - y0);
                if y_int <= cy + 1e-6 {
                    y_bottom = y_bottom.max(y_int);
                }
                if y_int >= cy - 1e-6 {
                    y_top = y_top.min(y_int);
                }
            }
        }

        let w_ray = if x_left.is_finite() && x_right.is_finite() && x_right > x_left + 1e-3 {
            x_right - x_left
        } else {
            bbox_w
        };

        let h_ray = if y_bottom.is_finite() && y_top.is_finite() && y_top > y_bottom + 1e-3 {
            y_top - y_bottom
        } else {
            bbox_h
        };

        let w_avail = w_ray.min(bbox_w);
        let h_avail = h_ray.min(bbox_h);

        // Safe margin: 78% of interior clearance width, 75% of height
        ((w_avail * 0.78).max(2.0), (h_avail * 0.75).max(2.0))
    }

    /// Checks whether a 2D point [x, y] lies inside the parcel polygon.
    pub fn contains_point(&self, pt: [f64; 2]) -> bool {
        if pt[0] < self.bbox_min[0] - 0.1
            || pt[0] > self.bbox_max[0] + 0.1
            || pt[1] < self.bbox_min[1] - 0.1
            || pt[1] > self.bbox_max[1] + 0.1
        {
            return false;
        }
        let n = self.vertices.len();
        if n < 3 {
            return false;
        }
        let mut inside = false;
        let mut j = n - 1;
        for i in 0..n {
            let (xi, yi) = (self.vertices[i][0], self.vertices[i][1]);
            let (xj, yj) = (self.vertices[j][0], self.vertices[j][1]);
            if ((yi > pt[1]) != (yj > pt[1]))
                && (pt[0] < (xj - xi) * (pt[1] - yi) / (yj - yi) + xi)
            {
                inside = !inside;
            }
            j = i;
        }
        inside
    }

    /// Resolves the current drawing annotation scale ratio (e.g. 100 for 1:100, 500 for 1:500).
    pub fn drawing_scale_ratio(doc: &CadDocument) -> f64 {
        if doc.header.annotation_scale_value > 1e-9
            && (doc.header.annotation_scale_value - 1.0).abs() > 1e-6
        {
            let inv = 1.0 / doc.header.annotation_scale_value;
            if inv.is_finite() && inv > 0.0 {
                return inv.clamp(0.01, 10000.0);
            }
        }

        let name = doc.header.current_annotation_scale.trim();
        if let Some(pos) = name.find(':') {
            let left: f64 = name[..pos].trim().parse().unwrap_or(1.0);
            let right: f64 = name[pos + 1..].trim().parse().unwrap_or(1.0);
            if left > 1e-6 && right > 1e-6 {
                let ratio = right / left;
                if ratio.is_finite() && ratio > 0.0 {
                    return ratio.clamp(0.01, 10000.0);
                }
            }
        }

        1.0
    }
}

/// Formats a 2D vector (dx, dy) into standard Civil 3D Quadrant Bearing format:
/// e.g. "N 45°00'00\" E" or "S 12°34'56\" W"
pub fn quadrant_bearing(dx: f64, dy: f64) -> String {
    let rad = dy.atan2(dx);
    let mut az = (std::f64::consts::FRAC_PI_2 - rad).to_degrees();
    while az < 0.0 {
        az += 360.0;
    }
    while az >= 360.0 {
        az -= 360.0;
    }
    let (quad_pre, angle, quad_suf) = if az <= 90.0 {
        ("N", az, "E")
    } else if az <= 180.0 {
        ("S", 180.0 - az, "E")
    } else if az <= 270.0 {
        ("S", az - 180.0, "W")
    } else {
        ("N", 360.0 - az, "W")
    };
    let deg = angle.floor() as i32;
    let rem_min = (angle - deg as f64) * 60.0;
    let min = rem_min.floor() as i32;
    let sec = ((rem_min - min as f64) * 60.0).round() as i32;
    let (min, sec) = if sec >= 60 { (min + 1, 0) } else { (min, sec) };
    let (deg, min) = if min >= 60 { (deg + 1, 0) } else { (deg, min) };
    format!("{quad_pre} {deg:02}\u{00b0}{min:02}'{sec:02}\" {quad_suf}")
}

/// Helper to wrap text into multiple lines at word boundaries before exceeding safe_width.
pub fn smart_wrap_text(text: &str, safe_width: f64, char_width: f64) -> String {
    let max_chars = ((safe_width / char_width.max(0.05)).floor() as usize).max(6);
    if text.chars().count() <= max_chars {
        return text.to_string();
    }

    let mut result = String::new();
    for orig_line in text.split('\n') {
        let words: Vec<&str> = orig_line.split_whitespace().collect();
        if words.is_empty() {
            if !result.is_empty() {
                result.push('\n');
            }
            continue;
        }

        let mut current_line = String::new();
        let mut current_len = 0;

        for word in words {
            let w_len = word.chars().count();
            if current_len == 0 {
                current_line.push_str(word);
                current_len = w_len;
            } else if current_len + 1 + w_len <= max_chars {
                current_line.push(' ');
                current_line.push_str(word);
                current_len += 1 + w_len;
            } else {
                if !result.is_empty() {
                    result.push('\n');
                }
                result.push_str(&current_line);
                current_line.clear();
                current_line.push_str(word);
                current_len = w_len;
            }
        }
        if !current_line.is_empty() {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(&current_line);
        }
    }

    if result.is_empty() {
        text.to_string()
    } else {
        result
    }
}

impl ParcelData {

    /// Dynamically synchronizes parcel centroid labels (`PC-PARCEL-LABEL`) and table rows (`PC-TABLE`)
    /// across the CAD document when parcel properties or geometry are modified.
    /// Resolves site topology overlaps for an active parcel (cutter):
    /// Any existing parcel in the same site that overlaps `active_handle` is automatically
    /// trimmed / clipped so that parcels within the same site never overlap.
    pub fn resolve_site_topology_overlaps(
        doc: &mut CadDocument,
        active_handle: Option<Handle>,
    ) -> usize {
        let Some(active_h) = active_handle else {
            return Self::resolve_all_site_overlaps(doc);
        };

        let Some(cutter) = Self::from_document_entity(doc, active_h) else {
            return 0;
        };

        if cutter.vertices.len() < 3 || cutter.site_name.trim().is_empty() {
            return 0;
        }

        let all_parcels = Self::all_parcels(doc);
        let mut resolved_count = 0;

        for other in all_parcels {
            if other.handle == active_h {
                continue;
            }
            if !other.site_name.eq_ignore_ascii_case(&cutter.site_name) {
                continue; // Different sites are allowed to overlap!
            }

            // Quick AABB overlap check
            if cutter.bbox_max[0] < other.bbox_min[0]
                || cutter.bbox_min[0] > other.bbox_max[0]
                || cutter.bbox_max[1] < other.bbox_min[1]
                || cutter.bbox_min[1] > other.bbox_max[1]
            {
                continue;
            }

            // Compute difference: other \ cutter
            let diff = polygon_difference(&other.vertices, &cutter.vertices);

            if diff.is_empty() {
                // `other` was completely engulfed by `cutter`!
                let _ = doc.remove_entity(other.handle);
                resolved_count += 1;
            } else {
                let best_poly = diff
                    .into_iter()
                    .max_by(|a, b| calc_polygon_area(a).abs().partial_cmp(&calc_polygon_area(b).abs()).unwrap())
                    .unwrap();

                let old_area = other.area_2d;
                let new_area = calc_polygon_area(&best_poly).abs();

                if (old_area - new_area).abs() > 0.05 && best_poly.len() >= 3 {
                    if let Some(ent) = doc.get_entity_mut(other.handle) {
                        if let EntityType::LwPolyline(lw) = ent {
                            lw.vertices = best_poly
                                .iter()
                                .map(|pt| codec::entities::LwVertex::from_coords(pt[0], pt[1]))
                                .collect();
                            resolved_count += 1;
                        }
                    }
                }
            }
        }

        resolved_count
    }

    /// Resolves all overlapping parcels globally across each site in the drawing.
    pub fn resolve_all_site_overlaps(doc: &mut CadDocument) -> usize {
        let mut total_resolved = 0;
        let mut passes = 0;

        while passes < 5 {
            passes += 1;
            let parcels = Self::all_parcels(doc);
            let mut resolved_in_pass = 0;

            for i in 0..parcels.len() {
                for j in (i + 1)..parcels.len() {
                    let p1 = &parcels[i];
                    let p2 = &parcels[j];

                    if !p1.site_name.eq_ignore_ascii_case(&p2.site_name) {
                        continue;
                    }

                    if p1.bbox_max[0] < p2.bbox_min[0]
                        || p1.bbox_min[0] > p2.bbox_max[0]
                        || p1.bbox_max[1] < p2.bbox_min[1]
                        || p1.bbox_min[1] > p2.bbox_max[1]
                    {
                        continue;
                    }

                    let diff = polygon_difference(&p2.vertices, &p1.vertices);
                    if diff.is_empty() {
                        let _ = doc.remove_entity(p2.handle);
                        resolved_in_pass += 1;
                        break;
                    } else {
                        let best = diff.into_iter().max_by(|a, b| calc_polygon_area(a).abs().partial_cmp(&calc_polygon_area(b).abs()).unwrap()).unwrap();
                        let old_area = p2.area_2d;
                        let new_area = calc_polygon_area(&best).abs();
                        if (old_area - new_area).abs() > 0.05 && best.len() >= 3 {
                            if let Some(ent) = doc.get_entity_mut(p2.handle) {
                                if let EntityType::LwPolyline(lw) = ent {
                                    lw.vertices = best.iter().map(|pt| codec::entities::LwVertex::from_coords(pt[0], pt[1])).collect();
                                    resolved_in_pass += 1;
                                    break;
                                }
                            }
                        }
                    }
                }
                if resolved_in_pass > 0 {
                    break;
                }
            }

            total_resolved += resolved_in_pass;
            if resolved_in_pass == 0 {
                break;
            }
        }

        total_resolved
    }


    pub fn sync_dynamic_parcels(
        doc: &mut CadDocument,
        target_handle: Option<Handle>,
        old_name: Option<&str>,
    ) -> usize {
        // 0. Enforce site topology: resolve any overlaps within the same Site
        if let Some(th) = target_handle {
            Self::resolve_site_topology_overlaps(doc, Some(th));
        } else {
            Self::resolve_all_site_overlaps(doc);
        }

        // 0. Clean up accidental XDATA on explicit non-parcel entities (such as PC-SETBACK or label entities)
        for ent in doc.entities_mut() {
            let layer = ent.common().layer.clone();
            let l = layer.to_ascii_uppercase();
            let is_explicit_non_parcel = l.contains("SETBACK")
                || l.contains("LABEL")
                || l.contains("TABLE")
                || l.contains("SEGMENT")
                || l.contains("PROP-LINE")
                || l.contains("PROP_LINE");

            let is_closed_poly = match ent {
                EntityType::LwPolyline(lw) => lw.is_closed && lw.vertices.len() >= 3,
                _ => false,
            };

            if is_explicit_non_parcel || (!is_parcel_layer(&layer) && !is_closed_poly) {
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
        let mut updated_total = 0;

        let active_names: rustc_hash::FxHashSet<String> = all.iter().map(|p| p.name.clone()).collect();

        // Purge orphan centroid labels, segment labels, and boundary texts whose parcels no longer exist
        if target_handle.is_none() {
            let is_parcel_label_layer = |layer: &str| -> bool {
                let l = layer.to_ascii_uppercase();
                l == "PC-PARCEL-LABEL" || l.starts_with("PC-PARCEL-LABEL")
                    || l == "PC-LABEL" || l.starts_with("PC-LABEL")
                    || l == "PC-SEGMENT-LABEL" || l.starts_with("PC-SEGMENT-LABEL")
                    || l == "C-PROP-LINE" || l.starts_with("C-PROP-LINE")
            };

            let orphan_handles: Vec<Handle> = doc.entities().filter_map(|ent| {
                if !matches!(ent, EntityType::Text(_) | EntityType::MText(_)) {
                    return None;
                }
                let layer = &ent.common().layer;
                if is_parcel_label_layer(layer) {
                    let tag = ent.common().extended_data.get_record("OPENCAD_PARCEL_LABEL")
                        .and_then(|r| r.values.first())
                        .and_then(|v| match v {
                            codec::xdata::XDataValue::String(s) => Some(s.as_str()),
                            _ => None,
                        });
                    if let Some(t) = tag {
                        if !active_names.contains(t) {
                            return Some(ent.common().handle);
                        } else {
                            return None;
                        }
                    }

                    let s_tag = ent.common().extended_data.get_record("OPENCAD_PARCEL_SEGMENT_LABEL")
                        .and_then(|r| r.values.first())
                        .and_then(|v| match v {
                            codec::xdata::XDataValue::String(s) => Some(s.as_str()),
                            _ => None,
                        });
                    if let Some(t) = s_tag {
                        if !active_names.contains(t) {
                            return Some(ent.common().handle);
                        } else {
                            return None;
                        }
                    }

                    let p_tag = ent.common().extended_data.get_record(XDATA_PARCEL)
                        .and_then(|r| r.values.first())
                        .and_then(|v| match v {
                            codec::xdata::XDataValue::String(s) => Some(s.as_str()),
                            _ => None,
                        });
                    if let Some(t) = p_tag {
                        if !active_names.contains(t) {
                            return Some(ent.common().handle);
                        } else {
                            return None;
                        }
                    }

                    // Fallback for un-tagged labels on parcel label layers.
                    // Note: SEGMENT LABELS ARE ON BOUNDARY EDGES (OUTSIDE POLYGON INTERIOR).
                    // NEVER purge segment labels via `contains_point`!
                    let is_segment_layer = layer.eq_ignore_ascii_case("PC-SEGMENT-LABEL")
                        || layer.eq_ignore_ascii_case("C-PROP-LINE");
                    if is_segment_layer {
                        return None;
                    }

                    let (val, pt) = match ent {
                        EntityType::Text(t) => (t.value.trim(), [t.insertion_point.x, t.insertion_point.y]),
                        EntityType::MText(m) => (m.value.trim(), [m.insertion_point.x, m.insertion_point.y]),
                        _ => ("", [0.0, 0.0]),
                    };
                    if !val.is_empty() {
                        let first_line = val.lines().next().unwrap_or("").trim();
                        let is_lot_name = first_line.to_ascii_uppercase().starts_with("LOT")
                            || first_line.starts_with("Lot ");
                        if is_lot_name && !active_names.contains(first_line) {
                            return Some(ent.common().handle);
                        }
                        let is_geom_label = val.contains("m²") || val.contains("m\u{b2}") || val.contains("ac") || val.starts_with("P:");
                        if is_geom_label && !all.iter().any(|p| p.contains_point(pt)) {
                            return Some(ent.common().handle);
                        }
                    }
                }
                None
            }).collect();
            updated_total += orphan_handles.len();
            for oh in orphan_handles {
                let _ = doc.remove_entity(oh);
            }
        }

        let parcels_to_sync: Vec<Self> = if let Some(th) = target_handle {
            all.iter().filter(|p| p.handle == th).cloned().collect()
        } else {
            all.clone()
        };

        if parcels_to_sync.is_empty() {
            return updated_total;
        }

        let mut labeled_edges: rustc_hash::FxHashSet<((i64, i64), (i64, i64))> = rustc_hash::FxHashSet::default();

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

                        // Check explicit lot name matches
                        let text_match = text_val == p_old_name || text_val == parcel.name.as_str();
                        if !text_match && (text_val.starts_with("LOT-") || text_val.starts_with("Lot ")) {
                            return None; // Belongs to another lot!
                        }

                        let dx = pt[0] - parcel.centroid[0];
                        let dy = pt[1] - parcel.centroid[1];
                        let dist = (dx * dx + dy * dy).sqrt();

                        let max_dim = (parcel.bbox_max[0] - parcel.bbox_min[0])
                            .max(parcel.bbox_max[1] - parcel.bbox_min[1]);

                        let in_bbox = pt[0] >= parcel.bbox_min[0] - 0.5
                            && pt[0] <= parcel.bbox_max[0] + 0.5
                            && pt[1] >= parcel.bbox_min[1] - 0.5
                            && pt[1] <= parcel.bbox_max[1] + 0.5;

                        let xdata_match = label_parcel_tag.is_some_and(|s| s == p_old_name || s == parcel.name.as_str());

                        if xdata_match || text_match {
                            // Check if this text is strictly inside ANOTHER parcel
                            let strictly_in_other = all.iter().any(|other| other.handle != parcel.handle && other.contains_point(pt));
                            if !strictly_in_other {
                                return Some((ent.common().handle, text_val, pt));
                            }
                        } else if in_bbox && dist < max_dim * 0.75 {
                            let strictly_in_other = all.iter().any(|other| other.handle != parcel.handle && other.contains_point(pt));
                            let in_this_parcel = parcel.contains_point(pt);
                            if in_this_parcel || (!strictly_in_other && dist < max_dim * 0.5) {
                                return Some((ent.common().handle, text_val, pt));
                            }
                        }
                    }
                    None
                })
                .collect();

            let (safe_w, safe_h) = parcel.interior_clearance();
            let scale_ratio = Self::drawing_scale_ratio(doc);
            let base_paper_h = if doc.header.text_height > 1e-6 {
                doc.header.text_height
            } else {
                2.2
            };

            // Scale text dynamically with drawing scale:
            // Standard paper text height (2.2mm) in drawing meters:
            // 1:1000 -> 2.2m, 1:500 -> 1.1m, 1:200 -> 0.44m, 1:100 -> 0.22m.
            // If scale_ratio is 1.0 (unspecified / 1:1 model space), use base_paper_h directly.
            let mut text_h = if (scale_ratio - 1.0).abs() > 1e-4 {
                (base_paper_h * (scale_ratio / 1000.0)).clamp(0.25, 12.0)
            } else {
                base_paper_h
            };

            // Enforce boundary containment:
            let max_h_for_parcel = (safe_h / 3.4).max(0.25);
            if text_h > max_h_for_parcel {
                text_h = max_h_for_parcel;
            }
            let max_h_for_width = (safe_w / 6.0).max(0.25);
            if text_h > max_h_for_width {
                text_h = max_h_for_width;
            }

            let cx = parcel.centroid[0];
            let cy = parcel.centroid[1];

            let char_w = text_h * 0.65;
            let area_raw = format!(
                "{:.1} m² ({:.3} ac)",
                parcel.area_2d,
                parcel.area_2d * 0.000247105
            );
            let perim_raw = format!("P: {:.2} m", parcel.perimeter);

            let style_norm = parcel.area_label_style.trim().to_ascii_lowercase();
            let raw_lines: Vec<(String, f64, &'static str)> = if style_norm.contains("name only") {
                vec![(parcel.name.clone(), 1.0, "name")]
            } else if style_norm.contains("area only") {
                vec![(area_raw.clone(), 0.9, "area")]
            } else if style_norm.contains("square meters") || style_norm.contains("acres") {
                vec![
                    (format!("{:.1} m²", parcel.area_2d), 0.95, "area"),
                    (format!("{:.3} Acres", parcel.area_2d * 0.000247105), 0.85, "acres"),
                ]
            } else if style_norm == "parcel name & area" || (style_norm.contains("name") && style_norm.contains("area") && !style_norm.contains("perim")) {
                vec![
                    (parcel.name.clone(), 1.0, "name"),
                    (area_raw.clone(), 0.9, "area"),
                ]
            } else {
                // "Parcel Name, Area & Perimeter" (default)
                vec![
                    (parcel.name.clone(), 1.0, "name"),
                    (area_raw.clone(), 0.9, "area"),
                    (perim_raw.clone(), 0.85, "perim"),
                ]
            };

            let line_info: Vec<(String, f64, f64, &'static str)> = raw_lines
                .into_iter()
                .map(|(raw, scale, kind)| {
                    let wrapped = smart_wrap_text(&raw, safe_w, char_w);
                    let n_sub = wrapped.split('\n').count().max(1);
                    let line_pitch = (text_h * scale) * 1.3;
                    let line_h = (n_sub as f64) * line_pitch;
                    (wrapped, scale, line_h, kind)
                })
                .collect();

            let count = line_info.len();
            let gap = text_h * 0.35;
            let total_lines_h: f64 = line_info.iter().map(|(_, _, h, _)| *h).sum();
            let total_needed_h = total_lines_h + (count.saturating_sub(1) as f64) * gap;

            if total_needed_h > safe_h && total_needed_h > 1e-4 {
                let scale_down = (safe_h / total_needed_h).clamp(0.2, 1.0);
                text_h = (text_h * scale_down).max(0.2);
            }

            let mut line_positions: Vec<Vector3> = Vec::new();
            if count == 1 {
                line_positions.push(Vector3::new(cx, cy, 0.0));
            } else if count == 2 {
                let half_span = (line_info[0].2 + line_info[1].2) * 0.25 + gap * 0.5;
                line_positions.push(Vector3::new(cx, cy + half_span, 0.0));
                line_positions.push(Vector3::new(cx, cy - half_span, 0.0));
            } else {
                let top_y = cy + (line_info[0].2 + line_info[1].2) * 0.5 + gap;
                let mid_y = cy;
                let bot_y = cy - (line_info[1].2 + line_info[2].2) * 0.5 - gap;
                line_positions.push(Vector3::new(cx, top_y, 0.0));
                line_positions.push(Vector3::new(cx, mid_y, 0.0));
                line_positions.push(Vector3::new(cx, bot_y, 0.0));
            }

            if label_candidates.is_empty() {
                for (idx, (wrapped_text, scale, _, _)) in line_info.iter().enumerate() {
                    let pos = line_positions[idx];
                    let mut m = MText::new();
                    m.value = wrapped_text.clone();
                    m.height = text_h * scale;
                    m.attachment_point = AttachmentPoint::MiddleCenter;
                    m.rectangle_width = safe_w;
                    m.insertion_point = pos;
                    m.is_annotative = true;
                    let mut ent = EntityType::MText(Box::new(m));
                    ent.common_mut().layer = "PC-PARCEL-LABEL".to_string();
                    let mut rec = ExtendedDataRecord::new("OPENCAD_PARCEL_LABEL");
                    rec.add_value(XDataValue::String(parcel.name.clone()));
                    rec.add_value(XDataValue::String(wrapped_text.clone()));
                    ent.common_mut().extended_data.add_record(rec);
                    let _ = doc.add_entity(ent);
                    labels_updated += 1;
                }
            } else {
                let mut kept_candidates = Vec::new();
                for (idx, cand) in label_candidates.into_iter().enumerate() {
                    if idx < count {
                        kept_candidates.push(cand);
                    } else {
                        let _ = doc.remove_entity(cand.0);
                    }
                }
                for (idx, (h, _, _)) in kept_candidates.iter().enumerate() {
                    let (wrapped_text, scale, _, _) = &line_info[idx];
                    let pos = line_positions[idx];
                    if let Some(ent) = doc.get_entity_mut(*h) {
                        match ent {
                            EntityType::Text(t) => {
                                t.value = wrapped_text.clone();
                                t.horizontal_alignment = TextHorizontalAlignment::Center;
                                t.vertical_alignment = TextVerticalAlignment::Middle;
                                t.insertion_point = pos;
                                t.alignment_point = Some(pos);
                                t.height = text_h * scale;
                            }
                            EntityType::MText(m) => {
                                m.value = wrapped_text.clone();
                                m.attachment_point = AttachmentPoint::MiddleCenter;
                                m.insertion_point = pos;
                                m.rectangle_width = safe_w;
                                m.height = text_h * scale;
                                m.is_annotative = true;
                            }
                            _ => {}
                        }
                        let mut rec = ExtendedDataRecord::new("OPENCAD_PARCEL_LABEL");
                        rec.add_value(XDataValue::String(parcel.name.clone()));
                        rec.add_value(XDataValue::String(wrapped_text.clone()));
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
                }
                let has_only_text = kept_candidates.iter().all(|(h, _, _)| {
                    matches!(doc.get_entity(*h), Some(EntityType::Text(_)))
                });
                for idx in kept_candidates.len()..count {
                    let (wrapped_text, scale, _, _) = &line_info[idx];
                    let pos = line_positions[idx];
                    let ent = if has_only_text && !kept_candidates.is_empty() {
                        let mut t = Text::with_value(wrapped_text.clone(), pos);
                        t.horizontal_alignment = TextHorizontalAlignment::Center;
                        t.vertical_alignment = TextVerticalAlignment::Middle;
                        t.alignment_point = Some(pos);
                        t.height = text_h * scale;
                        let mut e = EntityType::Text(t);
                        e.common_mut().layer = "PC-PARCEL-LABEL".to_string();
                        let mut rec = ExtendedDataRecord::new("OPENCAD_PARCEL_LABEL");
                        rec.add_value(XDataValue::String(parcel.name.clone()));
                        rec.add_value(XDataValue::String(wrapped_text.clone()));
                        e.common_mut().extended_data.add_record(rec);
                        e
                    } else {
                        let mut m = MText::new();
                        m.value = wrapped_text.clone();
                        m.height = text_h * scale;
                        m.attachment_point = AttachmentPoint::MiddleCenter;
                        m.rectangle_width = safe_w;
                        m.insertion_point = pos;
                        m.is_annotative = true;
                        let mut e = EntityType::MText(Box::new(m));
                        e.common_mut().layer = "PC-PARCEL-LABEL".to_string();
                        let mut rec = ExtendedDataRecord::new("OPENCAD_PARCEL_LABEL");
                        rec.add_value(XDataValue::String(parcel.name.clone()));
                        rec.add_value(XDataValue::String(wrapped_text.clone()));
                        e.common_mut().extended_data.add_record(rec);
                        e
                    };
                    let _ = doc.add_entity(ent);
                    labels_updated += 1;
                }
            }

            // ── 2. Update Segment Labels along Boundary Edges ───────────────────
            let seg_layer = if parcel.segment_layer.is_empty() {
                "C-PROP-LINE".to_string()
            } else {
                parcel.segment_layer.clone()
            };

            let old_seg_handles: Vec<Handle> = doc.entities().filter_map(|ent| {
                if !matches!(ent, EntityType::Text(_) | EntityType::MText(_)) {
                    return None;
                }
                let layer = &ent.common().layer;
                let is_seg_layer = layer.eq_ignore_ascii_case(&seg_layer)
                    || layer.eq_ignore_ascii_case("PC-SEGMENT-LABEL")
                    || layer.eq_ignore_ascii_case("C-PROP-LINE");
                if !is_seg_layer {
                    return None;
                }
                let seg_tag = ent.common().extended_data.get_record("OPENCAD_PARCEL_SEGMENT_LABEL")
                    .and_then(|r| r.values.first())
                    .and_then(|v| match v {
                        codec::xdata::XDataValue::String(s) => Some(s.as_str()),
                        _ => None,
                    });
                if let Some(t) = seg_tag {
                    if t == p_old_name || t == parcel.name.as_str() {
                        return Some(ent.common().handle);
                    }
                }
                None
            }).collect();

            for h in old_seg_handles {
                let _ = doc.remove_entity(h);
            }

            if parcel.auto_add_segment_labels && parcel.vertices.len() >= 3 {
                let n_verts = parcel.vertices.len();
                let line_style_norm = parcel.line_label_style.trim().to_ascii_lowercase();
                let curve_style_norm = parcel.curve_label_style.trim().to_ascii_lowercase();

                for i in 0..n_verts {
                    let j = (i + 1) % n_verts;
                    let p1 = parcel.vertices[i];
                    let p2 = parcel.vertices[j];
                    let bulge = parcel.bulges.get(i).copied().unwrap_or(0.0);

                    let dx = p2[0] - p1[0];
                    let dy = p2[1] - p1[1];
                    let chord_len = (dx * dx + dy * dy).sqrt();
                    if chord_len < 1e-4 {
                        continue;
                    }

                    let key_p1 = ((p1[0] * 100.0).round() as i64, (p1[1] * 100.0).round() as i64);
                    let key_p2 = ((p2[0] * 100.0).round() as i64, (p2[1] * 100.0).round() as i64);
                    let edge_key = if key_p1 <= key_p2 { (key_p1, key_p2) } else { (key_p2, key_p1) };
                    if labeled_edges.contains(&edge_key) {
                        continue;
                    }
                    labeled_edges.insert(edge_key);

                    let nx = -dy / chord_len;
                    let ny = dx / chord_len;

                    let mid_x = (p1[0] + p2[0]) * 0.5;
                    let mid_y = (p1[1] + p2[1]) * 0.5;

                    let (arc_mid_x, arc_mid_y, is_curve, curve_len, radius, delta_deg) = if bulge.abs() > 1e-5 {
                        let sagitta = (chord_len * 0.5) * bulge;
                        let mx = mid_x + nx * sagitta;
                        let my = mid_y + ny * sagitta;
                        let delta = 4.0 * bulge.atan();
                        let r = (chord_len / (2.0 * (delta * 0.5).sin().abs())).abs();
                        let clen = r * delta.abs();
                        let ddeg = delta.abs().to_degrees();
                        (mx, my, true, clen, r, ddeg)
                    } else {
                        (mid_x, mid_y, false, chord_len, 0.0, 0.0)
                    };

                    let offset_dist = (text_h * 0.65).clamp(0.25, 4.0);
                    let test_pt = [arc_mid_x + nx * offset_dist, arc_mid_y + ny * offset_dist];
                    let (out_nx, out_ny) = if parcel.contains_point(test_pt) {
                        (-nx, -ny)
                    } else {
                        (nx, ny)
                    };

                    let insert_x = arc_mid_x + out_nx * offset_dist;
                    let insert_y = arc_mid_y + out_ny * offset_dist;

                    let mut rot = dy.atan2(dx);
                    if rot > std::f64::consts::FRAC_PI_2 || rot < -std::f64::consts::FRAC_PI_2 {
                        rot += std::f64::consts::PI;
                    }
                    while rot > std::f64::consts::PI {
                        rot -= 2.0 * std::f64::consts::PI;
                    }

                    let label_text = if is_curve {
                        if curve_style_norm.contains("delta") {
                            format!("\u{0394}={:.1}\u{00b0}\nL={:.2}m", delta_deg, curve_len)
                        } else if curve_style_norm.contains("length only") {
                            format!("L={:.2}m", curve_len)
                        } else if curve_style_norm.contains("radius only") {
                            format!("R={:.2}m", radius)
                        } else {
                            format!("L={:.2}m\nR={:.2}m", curve_len, radius)
                        }
                    } else {
                        let bearing = quadrant_bearing(dx, dy);
                        if line_style_norm.contains("distance only") {
                            format!("{:.2}m", chord_len)
                        } else if line_style_norm.contains("bearing only") {
                            bearing
                        } else {
                            format!("{}\n{:.2}m", bearing, chord_len)
                        }
                    };

                    let mut m = MText::new();
                    m.value = label_text.clone();
                    m.height = text_h * 0.72;
                    m.attachment_point = AttachmentPoint::MiddleCenter;
                    m.rotation = rot;
                    m.insertion_point = Vector3::new(insert_x, insert_y, 0.0);
                    m.is_annotative = true;

                    let mut ent_seg = EntityType::MText(Box::new(m));
                    ent_seg.common_mut().layer = seg_layer.clone();

                    let mut rec_seg = ExtendedDataRecord::new("OPENCAD_PARCEL_SEGMENT_LABEL");
                    rec_seg.add_value(XDataValue::String(parcel.name.clone()));
                    rec_seg.add_value(XDataValue::String(i.to_string()));
                    rec_seg.add_value(XDataValue::String(label_text));
                    ent_seg.common_mut().extended_data.add_record(rec_seg);

                    let _ = doc.add_entity(ent_seg);
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
                    let is_header = col0_val.eq_ignore_ascii_case("LOT")
                        || col0_val.eq_ignore_ascii_case("PARCEL")
                        || col0_val.eq_ignore_ascii_case("LOT NAME")
                        || col0_val.eq_ignore_ascii_case("TAX ID");
                    if !is_header && !active_names.contains(col0_val) && (col0_val.to_ascii_uppercase().starts_with("LOT") || col0_val.starts_with("Lot ")) {
                        for cell in &cells {
                            let _ = doc.remove_entity(cell.0);
                        }
                        table_updated = true;
                        continue;
                    }

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

/// Formats a 2D direction (dx, dy) as standard surveying Quadrant Bearing (e.g. N 45°12'30" E)
pub fn quadrant_bearing_str(dx: f64, dy: f64) -> String {
    let mut az = dx.atan2(dy) * 180.0 / std::f64::consts::PI;
    if az < 0.0 {
        az += 360.0;
    }
    let (quad_pre, angle, quad_suf) = if az <= 90.0 {
        ("N", az, "E")
    } else if az <= 180.0 {
        ("S", 180.0 - az, "E")
    } else if az <= 270.0 {
        ("S", az - 180.0, "W")
    } else {
        ("N", 360.0 - az, "W")
    };
    let deg = angle.floor() as i32;
    let rem_min = (angle - deg as f64) * 60.0;
    let min = rem_min.floor() as i32;
    let sec = ((rem_min - min as f64) * 60.0).round() as i32;
    format!("{} {:02}°{:02}'{:02}\" {}", quad_pre, deg, min, sec, quad_suf)
}

/// Draws a Civil 3D Line Segment Table for a parcel at insertion point `origin`
pub fn draw_parcel_line_table(
    doc: &mut CadDocument,
    parcel: &ParcelData,
    origin: [f64; 2],
    text_h: f64,
) {
    let row_h = text_h * 2.2;
    let col_widths = [text_h * 6.0, text_h * 15.0, text_h * 13.0];
    let total_w: f64 = col_widths.iter().sum();
    let mut cur_y = origin[1];

    let add_line = |doc: &mut CadDocument, p1: [f64; 2], p2: [f64; 2]| {
        let mut l = codec::Line::new();
        l.start = codec::Vector3::new(p1[0], p1[1], 0.0);
        l.end = codec::Vector3::new(p2[0], p2[1], 0.0);
        let mut ent = codec::EntityType::Line(l);
        ent.common_mut().layer = "PC-TABLE".to_string();
        let _ = doc.add_entity(ent);
    };

    let add_text = |doc: &mut CadDocument, txt: &str, pos: [f64; 2], h: f64| {
        let t = codec::Text::with_value(txt.to_string(), codec::Vector3::new(pos[0], pos[1], 0.0))
            .with_height(h);
        let mut ent = codec::EntityType::Text(t);
        ent.common_mut().layer = "PC-TABLE".to_string();
        let _ = doc.add_entity(ent);
    };

    // Title Row
    let title = format!("LINE TABLE - {}", parcel.name);
    add_text(doc, &title, [origin[0] + text_h * 0.5, cur_y - row_h * 0.7], text_h * 1.1);
    add_line(doc, [origin[0], cur_y], [origin[0] + total_w, cur_y]);
    cur_y -= row_h;

    // Header Row
    add_line(doc, [origin[0], cur_y], [origin[0] + total_w, cur_y]);
    let headers = ["Tag", "Bearing", "Distance (m)"];
    let mut cur_x = origin[0];
    for (i, h_text) in headers.iter().enumerate() {
        add_text(doc, h_text, [cur_x + text_h * 0.3, cur_y - row_h * 0.7], text_h * 0.9);
        cur_x += col_widths[i];
    }
    cur_y -= row_h;

    // Data Rows
    let n = parcel.vertices.len();
    for i in 0..n {
        let p1 = parcel.vertices[i];
        let p2 = parcel.vertices[(i + 1) % n];
        let dx = p2[0] - p1[0];
        let dy = p2[1] - p1[1];
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-6 {
            continue;
        }

        let tag = format!("L{}", i + 1);
        let bearing = quadrant_bearing_str(dx, dy);
        add_line(doc, [origin[0], cur_y], [origin[0] + total_w, cur_y]);
        let vals = [tag, bearing, format!("{:.3}", len)];
        let mut row_x = origin[0];
        for (col_idx, v) in vals.iter().enumerate() {
            add_text(doc, v, [row_x + text_h * 0.3, cur_y - row_h * 0.7], text_h * 0.85);
            row_x += col_widths[col_idx];
        }
        cur_y -= row_h;
    }

    add_line(doc, [origin[0], cur_y], [origin[0] + total_w, cur_y]);

    // Vertical borders
    let table_bottom = cur_y;
    add_line(doc, [origin[0], origin[1]], [origin[0], table_bottom]);
    add_line(doc, [origin[0] + total_w, origin[1]], [origin[0] + total_w, table_bottom]);
    let mut v_x = origin[0];
    for w in &col_widths[..col_widths.len() - 1] {
        v_x += w;
        add_line(doc, [v_x, origin[1] - row_h], [v_x, table_bottom]);
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

    #[test]
    fn test_parcel_label_centering_wrapping_and_scaling() {
        let mut doc = CadDocument::new();

        // 1. Create a 25m wide x 40m high parcel with a long name
        let mut poly = LwPolyline::from_points(vec![
            Vector2::new(0.0, 0.0),
            Vector2::new(25.0, 0.0),
            Vector2::new(25.0, 40.0),
            Vector2::new(0.0, 40.0),
        ]);
        poly.is_closed = true;
        let mut ent_poly = EntityType::LwPolyline(poly);
        ent_poly.common_mut().layer = "PC-PARCEL".to_string();

        let mut rec = ExtendedDataRecord::new(XDATA_PARCEL);
        rec.add_value(XDataValue::String("Residential Lot 104 - Sector North West Subdiv".to_string()));
        rec.add_value(XDataValue::String("Site 1".to_string()));
        rec.add_value(XDataValue::String("104".to_string()));
        rec.add_value(XDataValue::String("Single-Family Residential (R-1)".to_string()));
        ent_poly.common_mut().extended_data.add_record(rec);

        let poly_h = doc.add_entity(ent_poly).unwrap();

        // 2. Set scale to 1:500 (annotation_scale_value = 0.002)
        doc.header.annotation_scale_value = 0.002;
        doc.header.current_annotation_scale = "1:500".to_string();

        // 3. Sync parcel -> generates dynamic labels
        let updated = ParcelData::sync_dynamic_parcels(&mut doc, Some(poly_h), None);
        assert!(updated > 0, "Must create parcel labels");

        // 4. Inspect generated labels on PC-PARCEL-LABEL
        let labels: Vec<MText> = doc.entities().filter_map(|e| {
            if e.common().layer == "PC-PARCEL-LABEL" {
                if let EntityType::MText(m) = e {
                    Some((**m).clone())
                } else {
                    None
                }
            } else {
                None
            }
        }).collect();

        assert_eq!(labels.len(), 3, "Must generate Name, Area, and Perimeter MTexts");

        for m in &labels {
            // Requirement 1: MiddleCenter alignment with zero bias
            assert_eq!(
                m.attachment_point,
                AttachmentPoint::MiddleCenter,
                "Parcel label must be MiddleCenter aligned without bias"
            );

            // Requirement 2: Wrap width set within safe boundaries
            assert!(
                m.rectangle_width > 0.0 && m.rectangle_width <= 25.0 * 0.85,
                "Wrap width ({}) must be inside parcel safe width (<= {})",
                m.rectangle_width,
                25.0 * 0.85
            );

            // Requirement 3: Text is annotative
            assert!(m.is_annotative, "Parcel label must be annotative");

            // Requirement 4: Text does not exceed boundary edges
            assert!(
                (m.insertion_point.x - 12.5).abs() < 1e-4,
                "Insertion X ({}) must be centered at parcel centroid (12.5)",
                m.insertion_point.x
            );
        }

        // Long name was wrapped before parcel boundary
        let name_label = labels.iter().find(|m| m.value.contains("Sector")).expect("Name label found");
        assert!(
            name_label.value.contains('\n'),
            "Long name must jump to next line before reaching parcel boundaries: {:?}",
            name_label.value
        );

        // 5. Change scale to 1:200 (annotation_scale_value = 0.005)
        doc.header.annotation_scale_value = 0.005;
        doc.header.current_annotation_scale = "1:200".to_string();

        let updated2 = ParcelData::sync_dynamic_parcels(&mut doc, Some(poly_h), None);
        assert!(updated2 > 0);

        // Verify height scaled dynamically
        let scaled_labels: Vec<MText> = doc.entities().filter_map(|e| {
            if e.common().layer == "PC-PARCEL-LABEL" {
                if let EntityType::MText(m) = e {
                    Some((**m).clone())
                } else {
                    None
                }
            } else {
                None
            }
        }).collect();

        // 1:200 text height (2.2 * 0.2 = 0.44m) is smaller than 1:500 text height (2.2 * 0.5 = 1.1m)
        assert!(
            scaled_labels[0].height < labels[0].height,
            "Text height must adapt dynamically to scale (1:200 height {} < 1:500 height {})",
            scaled_labels[0].height,
            labels[0].height
        );
    }
}

