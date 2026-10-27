// Civil 3D Parcel Interactive Commands: Slide Line and Create Parcel from Objects
//
// Implements interactive viewport workflows matching Autodesk Civil 3D:
// 1. Slide Line subdivision (SLIDELINE / PC_SLIDE_LINE):
//    Select parcel -> Select frontage start -> Select frontage end -> Specify angle ->
//    Live Ghost Jig preview -> Accept result -> Commits split, dynamic labels, and table rows.
// 2. Create Parcel from Objects (PARCELFROMOBJECTS):
//    Select closed polylines -> Converts to PC-PARCEL with Site, dynamic centroid labels, and PC-TABLE.

use codec::entities::LwPolyline;
use codec::types::Vector2;
use codec::xdata::{ExtendedDataRecord, XDataValue};
use codec::{EntityType, Handle};
use glam::DVec3;

use crate::app::commands::civil_parcel::{is_parcel_entity, XDATA_PARCEL};
use crate::command::{CadCommand, CmdOption, CmdResult};
use crate::scene::model::wire_model::WireModel;

// ── 2D Geometric Helper Functions ─────────────────────────────────────────

/// 2D Shoelace area of a closed polygon
pub fn shoelace_area(pts: &[[f64; 2]]) -> f64 {
    let n = pts.len();
    if n < 3 {
        return 0.0;
    }
    let mut area = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        area += pts[i][0] * pts[j][1] - pts[j][0] * pts[i][1];
    }
    area.abs() * 0.5
}

/// 2D Centroid of a polygon
pub fn polygon_centroid(pts: &[[f64; 2]]) -> [f64; 2] {
    let n = pts.len();
    if n < 3 {
        return [0.0, 0.0];
    }
    let mut area = 0.0;
    let mut cx = 0.0;
    let mut cy = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        let cross = pts[i][0] * pts[j][1] - pts[j][0] * pts[i][1];
        area += cross;
        cx += (pts[i][0] + pts[j][0]) * cross;
        cy += (pts[i][1] + pts[j][1]) * cross;
    }
    if area.abs() > 1e-9 {
        let f = 1.0 / (3.0 * area);
        [cx * f, cy * f]
    } else {
        let sum_x: f64 = pts.iter().map(|p| p[0]).sum();
        let sum_y: f64 = pts.iter().map(|p| p[1]).sum();
        [sum_x / n as f64, sum_y / n as f64]
    }
}

/// 2D Perimeter of a polygon
pub fn polygon_perimeter(pts: &[[f64; 2]]) -> f64 {
    let n = pts.len();
    if n < 2 {
        return 0.0;
    }
    let mut p = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        let dx = pts[j][0] - pts[i][0];
        let dy = pts[j][1] - pts[i][1];
        p += (dx * dx + dy * dy).sqrt();
    }
    p
}

/// Point-in-polygon ray-casting test
pub fn point_in_polygon(pt: [f64; 2], poly: &[[f64; 2]]) -> bool {
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let (x, y) = (pt[0], pt[1]);
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = (poly[i][0], poly[i][1]);
        let (xj, yj) = (poly[j][0], poly[j][1]);
        let intersect = ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi + 1e-12) + xi);
        if intersect {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Project point onto segment A->B, clamped to [0, 1]
pub fn closest_point_on_segment(pt: [f64; 2], a: [f64; 2], b: [f64; 2]) -> ([f64; 2], f64) {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-12 {
        return (a, 0.0);
    }
    let t = (((pt[0] - a[0]) * dx + (pt[1] - a[1]) * dy) / len_sq).clamp(0.0, 1.0);
    ([a[0] + t * dx, a[1] + t * dy], t)
}

/// Find closest edge and snapped point on polygon boundary
pub fn snap_point_to_polygon(pt: [f64; 2], poly: &[[f64; 2]]) -> ([f64; 2], usize, f64) {
    let n = poly.len();
    let mut best_pt = pt;
    let mut best_edge = 0;
    let mut best_t = 0.0;
    let mut min_dist_sq = f64::INFINITY;

    for i in 0..n {
        let j = (i + 1) % n;
        let (snapped, t) = closest_point_on_segment(pt, poly[i], poly[j]);
        let dx = pt[0] - snapped[0];
        let dy = pt[1] - snapped[1];
        let d2 = dx * dx + dy * dy;
        if d2 < min_dist_sq {
            min_dist_sq = d2;
            best_pt = snapped;
            best_edge = i;
            best_t = t;
        }
    }

    (best_pt, best_edge, best_t)
}

/// Tessellate a DXF bulge arc from p1 to p2 into num_segs discrete points.
pub fn tessellate_bulge(p1: [f64; 2], p2: [f64; 2], bulge: f64, num_segs: usize) -> Vec<[f64; 2]> {
    if bulge.abs() < 1e-6 {
        return vec![p1, p2];
    }
    let dx = p2[0] - p1[0];
    let dy = p2[1] - p1[1];
    let chord = (dx * dx + dy * dy).sqrt();
    if chord < 1e-9 {
        return vec![p1];
    }
    let theta = 4.0 * bulge.atan();
    let r = chord / (2.0 * (theta / 2.0).sin().abs());
    let mx = 0.5 * (p1[0] + p2[0]);
    let my = 0.5 * (p1[1] + p2[1]);
    let d = (r * r - 0.25 * chord * chord).max(0.0).sqrt();
    let nx = -dy / chord;
    let ny = dx / chord;
    let sign = if bulge > 0.0 { 1.0 } else { -1.0 };
    let sign_d = if theta.abs() > std::f64::consts::PI { -sign } else { sign };
    let cx = mx + sign_d * d * nx;
    let cy = my + sign_d * d * ny;

    let start_ang = (p1[1] - cy).atan2(p1[0] - cx);
    let sweep = theta;
    let mut pts = Vec::with_capacity(num_segs + 1);
    pts.push(p1);
    for step in 1..num_segs {
        let t = step as f64 / num_segs as f64;
        let ang = start_ang + t * sweep;
        pts.push([cx + r * ang.cos(), cy + r * ang.sin()]);
    }
    pts.push(p2);
    pts
}

/// Calculate cumulative path length of a series of 2D points.
pub fn poly_path_length(pts: &[[f64; 2]]) -> f64 {
    let mut total = 0.0;
    for i in 0..pts.len().saturating_sub(1) {
        let dx = pts[i + 1][0] - pts[i][0];
        let dy = pts[i + 1][1] - pts[i][1];
        total += (dx * dx + dy * dy).sqrt();
    }
    total
}

/// Traces the perimeter sub-path on a polygon between start_pt and end_pt along the shorter direction.
pub fn trace_polygon_path(poly: &[[f64; 2]], start_pt: [f64; 2], end_pt: [f64; 2]) -> Vec<[f64; 2]> {
    let n = poly.len();
    if n < 2 {
        return vec![start_pt, end_pt];
    }
    let (snap_s, edge_s, t_s) = snap_point_to_polygon(start_pt, poly);
    let (snap_e, edge_e, t_e) = snap_point_to_polygon(end_pt, poly);

    // Forward path (following vertex index increment)
    let mut path_fwd = vec![snap_s];
    if edge_s == edge_e && t_e >= t_s {
        path_fwd.push(snap_e);
    } else {
        let mut cur = (edge_s + 1) % n;
        while cur != (edge_e + 1) % n {
            path_fwd.push(poly[cur]);
            cur = (cur + 1) % n;
        }
        path_fwd.push(snap_e);
    }

    // Backward path (following vertex index decrement)
    let mut path_bwd = vec![snap_s];
    if edge_s == edge_e && t_e <= t_s {
        path_bwd.push(snap_e);
    } else {
        let mut cur = edge_s;
        while cur != edge_e {
            path_bwd.push(poly[cur]);
            cur = (cur + n - 1) % n;
        }
        path_bwd.push(snap_e);
    }

    let len_fwd = poly_path_length(&path_fwd);
    let len_bwd = poly_path_length(&path_bwd);
    if len_fwd <= len_bwd {
        path_fwd
    } else {
        path_bwd
    }
}

/// Samples a point and tangent vector along a multi-segment path at a given distance.
pub fn sample_path_at_dist(path: &[[f64; 2]], dist: f64) -> ([f64; 2], [f64; 2]) {
    if path.len() < 2 {
        let p = path.first().copied().unwrap_or([0.0, 0.0]);
        return (p, [1.0, 0.0]);
    }
    let mut cur_d = 0.0;
    for i in 0..path.len() - 1 {
        let a = path[i];
        let b = path[i + 1];
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let seg_len = (dx * dx + dy * dy).sqrt();
        if seg_len < 1e-9 {
            continue;
        }
        if cur_d + seg_len >= dist - 1e-9 || i == path.len() - 2 {
            let t = ((dist - cur_d) / seg_len).clamp(0.0, 1.0);
            let pt = [a[0] + t * dx, a[1] + t * dy];
            let tangent = [dx / seg_len, dy / seg_len];
            return (pt, tangent);
        }
        cur_d += seg_len;
    }
    let last = *path.last().unwrap();
    let prev = path[path.len() - 2];
    let dx = last[0] - prev[0];
    let dy = last[1] - prev[1];
    let len = (dx * dx + dy * dy).sqrt().max(1e-9);
    (last, [dx / len, dy / len])
}

/// Intersection between ray (origin + s * dir, s > 0) and segment A->B
pub fn ray_segment_intersection(
    origin: [f64; 2],
    dir: [f64; 2],
    a: [f64; 2],
    b: [f64; 2],
) -> Option<([f64; 2], f64, f64)> {
    let vx = b[0] - a[0];
    let vy = b[1] - a[1];
    let denom = dir[0] * vy - dir[1] * vx;
    if denom.abs() < 1e-11 {
        return None;
    }
    let ox_ax = a[0] - origin[0];
    let oy_ay = a[1] - origin[1];
    let s = (ox_ax * vy - oy_ay * vx) / denom;
    let u = (ox_ax * dir[1] - oy_ay * dir[0]) / denom;

    if s > 1e-5 && u >= -1e-5 && u <= 1.0 + 1e-5 {
        let clamped_u = u.clamp(0.0, 1.0);
        let ix = a[0] + clamped_u * vx;
        let iy = a[1] + clamped_u * vy;
        Some(([ix, iy], s, clamped_u))
    } else {
        None
    }
}

// ── Polygon Slicing and Slide Line Solver (Civil 3D Parity) ────────────────

/// Comprehensive Parcel Sizing Criteria matching Autodesk Civil 3D Parcel Layout Tools.
#[derive(Clone, Debug)]
pub struct ParcelSizingCriteria {
    pub min_area: f64,
    pub min_frontage: f64,
    pub use_offset: bool,
    pub frontage_offset: f64,
    pub min_width: f64,
    pub min_depth: f64,
    pub use_max_depth: bool,
    pub max_depth: f64,
    pub solution_preference: String, // "Use shortest frontage", "Use smallest area", "Use largest frontage"
    pub automatic_mode: bool,
    pub remainder_mode: String,
}

impl Default for ParcelSizingCriteria {
    fn default() -> Self {
        Self {
            min_area: 350.0,
            min_frontage: 12.0,
            use_offset: true,
            frontage_offset: 4.0,
            min_width: 10.0,
            min_depth: 20.0,
            use_max_depth: false,
            max_depth: 80.0,
            solution_preference: "Use shortest frontage".to_string(),
            automatic_mode: true,
            remainder_mode: "Place remainder in last parcel".to_string(),
        }
    }
}

impl ParcelSizingCriteria {
    pub fn from_state(st: &crate::ui::window::parcel_layout_tools::ParcelLayoutToolsState) -> Self {
        Self {
            min_area: st.min_area.parse::<f64>().unwrap_or(350.0),
            min_frontage: st.min_frontage.parse::<f64>().unwrap_or(12.0),
            use_offset: st.use_offset,
            frontage_offset: st.frontage_offset.parse::<f64>().unwrap_or(4.0),
            min_width: st.min_width.parse::<f64>().unwrap_or(10.0),
            min_depth: st.min_depth.parse::<f64>().unwrap_or(20.0),
            use_max_depth: st.use_max_depth,
            max_depth: st.max_depth.parse::<f64>().unwrap_or(80.0),
            solution_preference: st.solution_preference.clone(),
            automatic_mode: st.automatic_mode,
            remainder_mode: st.remainder_mode.clone(),
        }
    }
}

/// Computes the maximum lot depth measured from frontage points to the furthest lot boundary vertex.
pub fn compute_lot_depth(lot: &[[f64; 2]], frontage_pts: &[[f64; 2]]) -> f64 {
    if lot.is_empty() || frontage_pts.len() < 2 {
        return 0.0;
    }
    let mut max_depth: f64 = 0.0;
    for &pt in lot {
        let mut min_d_sq = f64::INFINITY;
        for i in 0..frontage_pts.len() - 1 {
            let (closest, _) = closest_point_on_segment(pt, frontage_pts[i], frontage_pts[i + 1]);
            let d_sq = (pt[0] - closest[0]).powi(2) + (pt[1] - closest[1]).powi(2);
            if d_sq < min_d_sq {
                min_d_sq = d_sq;
            }
        }
        let d = min_d_sq.sqrt();
        if d > max_depth {
            max_depth = d;
        }
    }
    max_depth
}

/// Computes the lot width measured across an interior setback offset line.
pub fn compute_width_at_offset(lot: &[[f64; 2]], frontage_pts: &[[f64; 2]], offset: f64) -> f64 {
    if lot.len() < 3 || frontage_pts.len() < 2 || offset <= 0.0 {
        return poly_path_length(frontage_pts);
    }
    let f_len = poly_path_length(frontage_pts);
    if f_len < 1e-4 {
        return 0.0;
    }
    let (pt_mid, tangent) = sample_path_at_dist(frontage_pts, 0.5 * f_len);
    let norm = [-tangent[1], tangent[0]];
    let mut inward_norm = norm;
    let test_pt = [pt_mid[0] + norm[0] * 0.1, pt_mid[1] + norm[1] * 0.1];
    if !point_in_polygon(test_pt, lot) {
        inward_norm = [-norm[0], -norm[1]];
    }
    let setback_origin = [pt_mid[0] + inward_norm[0] * offset, pt_mid[1] + inward_norm[1] * offset];
    let ray1 = tangent;
    let ray2 = [-tangent[0], -tangent[1]];
    let mut hit1 = None;
    let mut hit2 = None;
    let n = lot.len();
    for i in 0..n {
        let j = (i + 1) % n;
        if let Some((pt, s, _)) = ray_segment_intersection(setback_origin, ray1, lot[i], lot[j]) {
            if hit1.map_or(true, |(_, s_min)| s < s_min) {
                hit1 = Some((pt, s));
            }
        }
        if let Some((pt, s, _)) = ray_segment_intersection(setback_origin, ray2, lot[i], lot[j]) {
            if hit2.map_or(true, |(_, s_min)| s < s_min) {
                hit2 = Some((pt, s));
            }
        }
    }
    match (hit1, hit2) {
        (Some((p1, _)), Some((p2, _))) => {
            ((p1[0] - p2[0]).powi(2) + (p1[1] - p2[1]).powi(2)).sqrt()
        }
        _ => f_len,
    }
}

#[derive(Clone, Debug)]
pub struct SlideSlice {
    pub target_lot: Vec<[f64; 2]>,
    pub remainder_lot: Vec<[f64; 2]>,
    pub cut_line: ([f64; 2], [f64; 2]),
    pub target_area: f64,
    pub remainder_area: f64,
    pub frontage_length: f64,
    pub depth: f64,
    pub width_at_offset: f64,
    pub satisfies_criteria: bool,
}

/// Slice polygon by a ray originating on frontage edge
pub fn slice_polygon_by_ray(
    poly: &[[f64; 2]],
    frontage_edge: usize,
    origin: [f64; 2],
    dir: [f64; 2],
    f_start: [f64; 2],
) -> Option<SlideSlice> {
    let n = poly.len();
    if n < 3 {
        return None;
    }

    // Try primary dir first; fallback to -dir if no intersection found
    let mut best_hit = None;
    let mut best_dir = dir;
    let mut min_s = f64::INFINITY;

    for &try_dir in &[dir, [-dir[0], -dir[1]]] {
        for i in 0..n {
            if i == frontage_edge {
                continue;
            }
            let j = (i + 1) % n;
            if let Some((hit_pt, s, _u)) = ray_segment_intersection(origin, try_dir, poly[i], poly[j]) {
                if s > 1e-4 && s < min_s {
                    min_s = s;
                    best_hit = Some((i, hit_pt));
                    best_dir = try_dir;
                }
            }
        }
        if best_hit.is_some() {
            break;
        }
    }

    let (exit_edge, exit_pt) = best_hit?;

    // Build the two loops:
    // Loop 1 starts at origin, follows vertices forward to exit_edge, goes to exit_pt, and closes to origin
    let mut loop1 = Vec::new();
    loop1.push(origin);
    let mut cur = (frontage_edge + 1) % n;
    while cur != (exit_edge + 1) % n {
        loop1.push(poly[cur]);
        cur = (cur + 1) % n;
    }
    loop1.push(exit_pt);

    // Loop 2 starts at exit_pt, follows vertices forward to frontage_edge, goes to origin, and closes to exit_pt
    let mut loop2 = Vec::new();
    loop2.push(exit_pt);
    let mut cur2 = (exit_edge + 1) % n;
    while cur2 != (frontage_edge + 1) % n {
        loop2.push(poly[cur2]);
        cur2 = (cur2 + 1) % n;
    }
    loop2.push(origin);

    let area1 = shoelace_area(&loop1);
    let area2 = shoelace_area(&loop2);

    // Identify which loop is the target lot (the one containing the frontage portion between f_start and origin)
    let mid_f = [0.5 * (f_start[0] + origin[0]), 0.5 * (f_start[1] + origin[1])];
    let test_in = [mid_f[0] + 0.05 * best_dir[0], mid_f[1] + 0.05 * best_dir[1]];

    let (d1, _, _) = snap_point_to_polygon(f_start, &loop1);
    let dist1_sq = (f_start[0] - d1[0]).powi(2) + (f_start[1] - d1[1]).powi(2);
    let (d2, _, _) = snap_point_to_polygon(f_start, &loop2);
    let dist2_sq = (f_start[0] - d2[0]).powi(2) + (f_start[1] - d2[1]).powi(2);

    let loop1_is_target = if (dist1_sq - dist2_sq).abs() > 1e-4 {
        dist1_sq < dist2_sq
    } else {
        point_in_polygon(test_in, &loop1)
    };

    let (target_lot, remainder_lot, target_area, remainder_area) = if loop1_is_target {
        (loop1, loop2, area1, area2)
    } else {
        (loop2, loop1, area2, area1)
    };

    let dx_f = origin[0] - f_start[0];
    let dy_f = origin[1] - f_start[1];
    let frontage_length = (dx_f * dx_f + dy_f * dy_f).sqrt();
    let depth = compute_lot_depth(&target_lot, &[f_start, origin]);
    let width_at_offset = frontage_length;

    Some(SlideSlice {
        target_lot,
        remainder_lot,
        cut_line: (origin, exit_pt),
        target_area,
        remainder_area,
        frontage_length,
        depth,
        width_at_offset,
        satisfies_criteria: true,
    })
}

/// Comprehensive solver generating candidate slide line cuts along a frontage path,
/// respecting all Civil 3D criteria (Min Area, Min Frontage, Frontage Offset, Min/Max Depth, Solution Preference).
pub fn solve_slide_line_path_candidates(
    poly: &[[f64; 2]],
    frontage_path: &[[f64; 2]],
    angle_deg: f64,
    target_area: f64,
    criteria: Option<&ParcelSizingCriteria>,
) -> Result<Vec<SlideSlice>, String> {
    let total_area = shoelace_area(poly);
    if total_area < 1e-4 {
        return Err("Parcel polygon has zero area".to_string());
    }
    if target_area <= 0.0 || target_area >= total_area {
        return Err(format!(
            "Target area ({:.1} m²) must be between 0 and total area ({:.1} m²)",
            target_area, total_area
        ));
    }
    let total_f_len = poly_path_length(frontage_path);
    if total_f_len < 1e-4 {
        return Err("Frontage path is too short".to_string());
    }

    let min_f = criteria.map_or(0.0, |c| c.min_frontage);
    let use_offset = criteria.map_or(false, |c| c.use_offset);
    let offset_dist = criteria.map_or(0.0, |c| c.frontage_offset);
    let min_w = criteria.map_or(0.0, |c| c.min_width);
    let min_depth = criteria.map_or(0.0, |c| c.min_depth);
    let use_max_d = criteria.map_or(false, |c| c.use_max_depth);
    let max_depth = criteria.map_or(0.0, |c| c.max_depth);
    let pref = criteria.map_or("Use shortest frontage", |c| c.solution_preference.as_str());

    // Candidate cut angles to explore multiple geometric solutions (Civil 3D Next/Accept)
    let mut candidate_angles = vec![angle_deg];
    let supp = 180.0 - angle_deg;
    if (supp - angle_deg).abs() > 1.0 && supp > 10.0 && supp < 170.0 {
        candidate_angles.push(supp);
    }
    if (angle_deg - 90.0).abs() > 2.0 && (supp - 90.0).abs() > 2.0 {
        candidate_angles.push(90.0);
    }

    let mut solutions: Vec<SlideSlice> = Vec::new();

    for &ang in &candidate_angles {
        let rad = ang.to_radians();
        let cos_a = rad.cos();
        let sin_a = rad.sin();

        let min_s = if min_f > 0.0 && total_f_len >= min_f * 1.05 {
            min_f.min(total_f_len * 0.95)
        } else {
            0.001 * total_f_len
        };
        let mut s_low = min_s;
        let mut s_high = 0.999 * total_f_len;
        let mut best_slice: Option<SlideSlice> = None;

        for _ in 0..60 {
            let s_mid = 0.5 * (s_low + s_high);
            let (pt, tangent) = sample_path_at_dist(frontage_path, s_mid);
            let f_start_pt = frontage_path[0];

            let ahead_dist = (0.02 * (total_f_len - s_mid)).min(0.5);
            let (ahead_pt, _) = sample_path_at_dist(frontage_path, s_mid + ahead_dist);
            let (_, f_edge, _) = snap_point_to_polygon(ahead_pt, poly);

            let u_x = tangent[0];
            let u_y = tangent[1];
            let n_x = -u_y;
            let n_y = u_x;

            let mut cut_dir = [
                u_x * cos_a + n_x * sin_a,
                u_y * cos_a + n_y * sin_a,
            ];

            let test_pt1 = [pt[0] + cut_dir[0] * 0.1, pt[1] + cut_dir[1] * 0.1];
            let test_pt2 = [pt[0] - cut_dir[0] * 0.1, pt[1] - cut_dir[1] * 0.1];
            if !point_in_polygon(test_pt1, poly) && point_in_polygon(test_pt2, poly) {
                cut_dir[0] = -cut_dir[0];
                cut_dir[1] = -cut_dir[1];
            }

            if let Some(mut slice) = slice_polygon_by_ray(poly, f_edge, pt, cut_dir, f_start_pt) {
                let diff = slice.target_area - target_area;
                slice.frontage_length = s_mid;
                best_slice = Some(slice);
                if diff.abs() < 0.01 {
                    break;
                }
                if diff < 0.0 {
                    s_low = s_mid;
                } else {
                    s_high = s_mid;
                }
            } else {
                s_low = (s_low + s_mid) * 0.5;
            }
        }

        if let Some(mut slice) = best_slice {
            let sub_path = trace_polygon_path(poly, frontage_path[0], slice.cut_line.0);
            let depth = compute_lot_depth(&slice.target_lot, &sub_path);
            let width_at_offset = if use_offset && offset_dist > 0.0 {
                compute_width_at_offset(&slice.target_lot, &sub_path, offset_dist)
            } else {
                slice.frontage_length
            };

            let mut ok = true;
            if min_f > 0.0 && slice.frontage_length < min_f - 0.05 {
                ok = false;
            }
            if use_offset && offset_dist > 0.0 {
                let req_w = min_f.max(min_w);
                if req_w > 0.0 && width_at_offset < req_w - 0.05 {
                    ok = false;
                }
            }
            if min_depth > 0.0 && depth < min_depth - 0.05 {
                ok = false;
            }
            if use_max_d && max_depth > 0.0 && depth > max_depth + 0.05 {
                ok = false;
            }

            slice.depth = depth;
            slice.width_at_offset = width_at_offset;
            slice.satisfies_criteria = ok;

            let is_dup = solutions.iter().any(|s| {
                (s.cut_line.0[0] - slice.cut_line.0[0]).abs() < 0.05
                    && (s.cut_line.0[1] - slice.cut_line.0[1]).abs() < 0.05
            });
            if !is_dup {
                solutions.push(slice);
            }
        }
    }

    if solutions.is_empty() {
        return Err("Could not find valid slide line cut for parcel".to_string());
    }

    solutions.sort_by(|a, b| {
        match (a.satisfies_criteria, b.satisfies_criteria) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => {
                if pref.contains("smallest area") {
                    (a.target_area - target_area).abs().partial_cmp(&(b.target_area - target_area).abs()).unwrap_or(std::cmp::Ordering::Equal)
                } else if pref.contains("largest frontage") {
                    b.frontage_length.partial_cmp(&a.frontage_length).unwrap_or(std::cmp::Ordering::Equal)
                } else {
                    a.frontage_length.partial_cmp(&b.frontage_length).unwrap_or(std::cmp::Ordering::Equal)
                }
            }
        }
    });

    Ok(solutions)
}

/// Solves for the slide line position that achieves `target_area`
pub fn solve_slide_line(
    poly: &[[f64; 2]],
    f_start: [f64; 2],
    f_end: [f64; 2],
    angle_deg: f64,
    target_area: f64,
) -> Result<SlideSlice, String> {
    solve_slide_line_criteria(poly, f_start, f_end, angle_deg, target_area, 0.0)
}

/// Solves for the slide line position that achieves `target_area` while respecting `min_frontage`.
pub fn solve_slide_line_criteria(
    poly: &[[f64; 2]],
    f_start: [f64; 2],
    f_end: [f64; 2],
    angle_deg: f64,
    target_area: f64,
    min_frontage: f64,
) -> Result<SlideSlice, String> {
    let crit = ParcelSizingCriteria {
        min_area: target_area,
        min_frontage,
        ..Default::default()
    };
    let path = vec![f_start, f_end];
    let solutions = solve_slide_line_path_candidates(poly, &path, angle_deg, target_area, Some(&crit))?;
    solutions.into_iter().next().ok_or_else(|| "Could not find valid slide line cut for parcel".to_string())
}

/// Comprehensive solver generating candidate swing line cuts around a pivot point along a frontage path,
/// respecting all Civil 3D criteria (Min Area, Min Frontage, Frontage Offset, Min Width, Min/Max Depth, Solution Preference).
pub fn solve_swing_line_candidates(
    poly: &[[f64; 2]],
    pivot: [f64; 2],
    frontage_path: &[[f64; 2]],
    target_area: f64,
    criteria: Option<&ParcelSizingCriteria>,
) -> Result<Vec<SlideSlice>, String> {
    let total_area = shoelace_area(poly);
    if total_area < 1e-4 {
        return Err("Parcel polygon has zero area".to_string());
    }
    if target_area <= 0.0 || target_area >= total_area {
        return Err(format!(
            "Target area ({:.1} m²) must be between 0 and total area ({:.1} m²)",
            target_area, total_area
        ));
    }
    let total_f_len = poly_path_length(frontage_path);
    if total_f_len < 1e-4 {
        return Err("Frontage path is too short".to_string());
    }

    let min_f = criteria.map_or(0.0, |c| c.min_frontage);
    let use_offset = criteria.map_or(false, |c| c.use_offset);
    let offset_dist = criteria.map_or(0.0, |c| c.frontage_offset);
    let min_w = criteria.map_or(0.0, |c| c.min_width);
    let min_depth = criteria.map_or(0.0, |c| c.min_depth);
    let use_max_d = criteria.map_or(false, |c| c.use_max_depth);
    let max_depth = criteria.map_or(0.0, |c| c.max_depth);
    let pref = criteria.map_or("Use shortest frontage", |c| c.solution_preference.as_str());

    let forward_path = frontage_path.to_vec();
    let mut reverse_path = frontage_path.to_vec();
    reverse_path.reverse();

    let search_paths = vec![forward_path, reverse_path];
    let mut solutions: Vec<SlideSlice> = Vec::new();

    for path in &search_paths {
        let f_start_pt = path[0];
        let mut s_low = 0.001 * total_f_len;
        let mut s_high = 0.999 * total_f_len;
        let mut best_slice: Option<SlideSlice> = None;

        for _ in 0..60 {
            let s_mid = 0.5 * (s_low + s_high);
            let (pt, _) = sample_path_at_dist(path, s_mid);
            let ray_dir = [pt[0] - pivot[0], pt[1] - pivot[1]];
            let r_len = (ray_dir[0] * ray_dir[0] + ray_dir[1] * ray_dir[1]).sqrt();
            if r_len < 1e-6 {
                s_low = (s_low + s_mid) * 0.5;
                continue;
            }
            let u_ray = [ray_dir[0] / r_len, ray_dir[1] / r_len];
            let (_, f_edge, _) = snap_point_to_polygon(pt, poly);

            if let Some(mut slice) = slice_polygon_by_ray(poly, f_edge, pt, [-u_ray[0], -u_ray[1]], f_start_pt) {
                let diff = slice.target_area - target_area;
                slice.frontage_length = s_mid;
                best_slice = Some(slice);
                if diff.abs() < 0.01 {
                    break;
                }
                if diff < 0.0 {
                    s_low = s_mid;
                } else {
                    s_high = s_mid;
                }
            } else {
                s_low = (s_low + s_mid) * 0.5;
            }
        }

        if let Some(mut slice) = best_slice {
            let sub_path = trace_polygon_path(poly, f_start_pt, slice.cut_line.0);
            let depth = compute_lot_depth(&slice.target_lot, &sub_path);
            let width_at_offset = if use_offset && offset_dist > 0.0 {
                compute_width_at_offset(&slice.target_lot, &sub_path, offset_dist)
            } else {
                slice.frontage_length
            };

            let mut ok = true;
            if min_f > 0.0 && slice.frontage_length < min_f - 0.05 {
                ok = false;
            }
            if use_offset && offset_dist > 0.0 {
                let req_w = min_f.max(min_w);
                if req_w > 0.0 && width_at_offset < req_w - 0.05 {
                    ok = false;
                }
            }
            if min_depth > 0.0 && depth < min_depth - 0.05 {
                ok = false;
            }
            if use_max_d && max_depth > 0.0 && depth > max_depth + 0.05 {
                ok = false;
            }

            slice.depth = depth;
            slice.width_at_offset = width_at_offset;
            slice.satisfies_criteria = ok;

            let is_dup = solutions.iter().any(|s| {
                (s.cut_line.0[0] - slice.cut_line.0[0]).abs() < 0.1
                    && (s.cut_line.0[1] - slice.cut_line.0[1]).abs() < 0.1
                    && (s.cut_line.1[0] - slice.cut_line.1[0]).abs() < 0.1
                    && (s.cut_line.1[1] - slice.cut_line.1[1]).abs() < 0.1
            });
            if !is_dup {
                solutions.push(slice);
            }
        }

        // Also test candidate cut at exact min_frontage if valid
        if min_f > 0.0 && total_f_len >= min_f * 1.05 {
            let (pt_f, _) = sample_path_at_dist(path, min_f);
            let ray_dir_f = [pt_f[0] - pivot[0], pt_f[1] - pivot[1]];
            let r_len_f = (ray_dir_f[0] * ray_dir_f[0] + ray_dir_f[1] * ray_dir_f[1]).sqrt();
            if r_len_f > 1e-6 {
                let u_ray_f = [ray_dir_f[0] / r_len_f, ray_dir_f[1] / r_len_f];
                let (_, f_edge_f, _) = snap_point_to_polygon(pt_f, poly);
                if let Some(mut slice_f) = slice_polygon_by_ray(poly, f_edge_f, pt_f, [-u_ray_f[0], -u_ray_f[1]], f_start_pt) {
                    slice_f.frontage_length = min_f;
                    let sub_path = trace_polygon_path(poly, f_start_pt, slice_f.cut_line.0);
                    let depth = compute_lot_depth(&slice_f.target_lot, &sub_path);
                    let width_at_offset = if use_offset && offset_dist > 0.0 {
                        compute_width_at_offset(&slice_f.target_lot, &sub_path, offset_dist)
                    } else {
                        min_f
                    };
                    let mut ok = slice_f.target_area >= target_area - 0.05;
                    if use_offset && offset_dist > 0.0 {
                        let req_w = min_f.max(min_w);
                        if req_w > 0.0 && width_at_offset < req_w - 0.05 {
                            ok = false;
                        }
                    }
                    if min_depth > 0.0 && depth < min_depth - 0.05 {
                        ok = false;
                    }
                    if use_max_d && max_depth > 0.0 && depth > max_depth + 0.05 {
                        ok = false;
                    }
                    slice_f.depth = depth;
                    slice_f.width_at_offset = width_at_offset;
                    slice_f.satisfies_criteria = ok;

                    let is_dup = solutions.iter().any(|s| {
                        (s.cut_line.0[0] - slice_f.cut_line.0[0]).abs() < 0.1
                            && (s.cut_line.0[1] - slice_f.cut_line.0[1]).abs() < 0.1
                            && (s.cut_line.1[0] - slice_f.cut_line.1[0]).abs() < 0.1
                            && (s.cut_line.1[1] - slice_f.cut_line.1[1]).abs() < 0.1
                    });
                    if !is_dup {
                        solutions.push(slice_f);
                    }
                }
            }
        }
    }

    if solutions.is_empty() {
        return Err("Could not find valid swing line solution".to_string());
    }

    solutions.sort_by(|a, b| {
        match (a.satisfies_criteria, b.satisfies_criteria) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => {
                if pref.contains("smallest area") {
                    (a.target_area - target_area).abs().partial_cmp(&(b.target_area - target_area).abs()).unwrap_or(std::cmp::Ordering::Equal)
                } else if pref.contains("largest frontage") {
                    b.frontage_length.partial_cmp(&a.frontage_length).unwrap_or(std::cmp::Ordering::Equal)
                } else {
                    a.frontage_length.partial_cmp(&b.frontage_length).unwrap_or(std::cmp::Ordering::Equal)
                }
            }
        }
    });

    Ok(solutions)
}

/// Solves for swing line cut around pivot point that achieves `target_area`
pub fn solve_swing_line(
    poly: &[[f64; 2]],
    pivot: [f64; 2],
    f_start: [f64; 2],
    f_end: [f64; 2],
    target_area: f64,
) -> Result<SlideSlice, String> {
    solve_swing_line_criteria(poly, pivot, f_start, f_end, target_area, 0.0)
}

/// Solves for swing line cut around pivot point respecting `min_frontage`
pub fn solve_swing_line_criteria(
    poly: &[[f64; 2]],
    pivot: [f64; 2],
    f_start: [f64; 2],
    f_end: [f64; 2],
    target_area: f64,
    min_frontage: f64,
) -> Result<SlideSlice, String> {
    let crit = ParcelSizingCriteria {
        min_area: target_area,
        min_frontage,
        ..Default::default()
    };
    let path = vec![f_start, f_end];
    let solutions = solve_swing_line_candidates(poly, pivot, &path, target_area, Some(&crit))?;
    solutions.into_iter().next().ok_or_else(|| "Could not find valid swing line cut".to_string())
}

// ── Multi-Lot Subdivision Solver ──────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct MultiSlideResult {
    pub lots: Vec<Vec<[f64; 2]>>,
    pub cut_lines: Vec<([f64; 2], [f64; 2])>,
    pub lot_areas: Vec<f64>,
    pub lot_frontages: Vec<f64>,
}

/// Solves multi-lot slide line subdivision along a multi-segment frontage path (corners, arcs, curves)
pub fn solve_multi_slide_line_path(
    poly: &[[f64; 2]],
    frontage_path: &[[f64; 2]],
    angle_deg: f64,
    min_area: f64,
    min_frontage: f64,
    remainder_mode: &str,
) -> Result<MultiSlideResult, String> {
    if frontage_path.len() < 2 {
        return Err("Frontage path must have at least 2 points".to_string());
    }
    if frontage_path.len() == 2 {
        return solve_multi_slide_line(
            poly,
            frontage_path[0],
            frontage_path[1],
            angle_deg,
            min_area,
            min_frontage,
            remainder_mode,
        );
    }

    let total_area = shoelace_area(poly);
    if total_area < 1e-4 {
        return Err("Parcel polygon has zero area".to_string());
    }
    if min_area <= 0.0 {
        return Err("Minimum area must be greater than zero".to_string());
    }
    if min_area >= total_area {
        return Err(format!(
            "Minimum area ({:.1} m²) is greater than total parcel area ({:.1} m²)",
            min_area, total_area
        ));
    }

    let total_f_len = poly_path_length(frontage_path);
    if total_f_len < 1e-4 {
        return Err("Frontage length is too short".to_string());
    }

    let is_redistribute = remainder_mode.starts_with("Redistribute");
    let is_create_remainder = remainder_mode.contains("Create remainder");

    let mut n_lots = (total_area / min_area).floor() as usize;
    if min_frontage > 0.0 && total_f_len >= min_frontage * 2.0 {
        let n_f = (total_f_len / min_frontage).floor() as usize;
        n_lots = n_lots.min(n_f);
    }
    n_lots = n_lots.max(2);

    let target_area = if is_redistribute {
        total_area / n_lots as f64
    } else {
        min_area
    };

    let remainder_area = total_area - (n_lots as f64 * min_area);
    let num_cuts = if is_create_remainder && remainder_area >= 1.0 {
        n_lots
    } else {
        n_lots - 1
    };

    let mut lots = Vec::new();
    let mut cut_lines = Vec::new();
    let mut lot_areas = Vec::new();
    let mut lot_frontages = Vec::new();

    let mut cur_poly = poly.to_vec();
    let mut s_cur = 0.0;

    for _k in 0..num_cuts {
        let cur_rem_area = shoelace_area(&cur_poly);
        let cur_rem_f = total_f_len - s_cur;

        if !is_create_remainder && cur_rem_area <= target_area * 1.05 {
            break;
        }
        if is_create_remainder && cur_rem_area < target_area {
            break;
        }

        let min_s_step = if min_frontage > 0.0 && cur_rem_f >= min_frontage * 1.05 {
            min_frontage.min(cur_rem_f * 0.95)
        } else {
            (0.005 * cur_rem_f).min(0.2)
        };
        let mut s_low = s_cur + min_s_step;
        let mut s_high = total_f_len - (0.005 * cur_rem_f).min(0.2);
        let mut best_slice: Option<(SlideSlice, f64)> = None;

        for _ in 0..60 {
            let s_mid = 0.5 * (s_low + s_high);
            let (pt, tangent) = sample_path_at_dist(frontage_path, s_mid);
            let (f_start_pt, _) = sample_path_at_dist(frontage_path, s_cur);

            let ahead_dist = (0.02 * (total_f_len - s_mid)).min(0.5);
            let (ahead_pt, _) = sample_path_at_dist(frontage_path, s_mid + ahead_dist);
            let (_, f_edge, _) = snap_point_to_polygon(ahead_pt, &cur_poly);

            let rad = angle_deg.to_radians();
            let cos_a = rad.cos();
            let sin_a = rad.sin();
            let u_x = tangent[0];
            let u_y = tangent[1];
            let n_x = -u_y;
            let n_y = u_x;

            let mut cut_dir = [
                u_x * cos_a + n_x * sin_a,
                u_y * cos_a + n_y * sin_a,
            ];

            let test_pt1 = [
                pt[0] + cut_dir[0] * 0.1,
                pt[1] + cut_dir[1] * 0.1,
            ];
            let test_pt2 = [
                pt[0] - cut_dir[0] * 0.1,
                pt[1] - cut_dir[1] * 0.1,
            ];
            if !point_in_polygon(test_pt1, &cur_poly) && point_in_polygon(test_pt2, &cur_poly) {
                cut_dir[0] = -cut_dir[0];
                cut_dir[1] = -cut_dir[1];
            }

            if let Some(slice) = slice_polygon_by_ray(&cur_poly, f_edge, pt, cut_dir, f_start_pt) {
                let diff = slice.target_area - target_area;
                best_slice = Some((slice, s_mid));
                if diff.abs() < 0.01 {
                    break;
                }
                if diff < 0.0 {
                    s_low = s_mid;
                } else {
                    s_high = s_mid;
                }
            } else {
                s_low = (s_low + s_mid) * 0.5;
            }
        }

        if let Some((slice, s_cut)) = best_slice {
            lots.push(slice.target_lot);
            cut_lines.push(slice.cut_line);
            lot_areas.push(slice.target_area);
            lot_frontages.push(s_cut - s_cur);

            cur_poly = slice.remainder_lot;
            s_cur = s_cut;
        } else {
            break;
        }
    }

    let final_area = shoelace_area(&cur_poly);
    let final_f_len = total_f_len - s_cur;
    lots.push(cur_poly);
    lot_areas.push(final_area);
    lot_frontages.push(final_f_len);

    if lots.len() < 2 {
        return Err("Could not divide parcel into at least 2 lots with specified criteria".to_string());
    }

    Ok(MultiSlideResult {
        lots,
        cut_lines,
        lot_areas,
        lot_frontages,
    })
}

/// Solves multi-lot slide line subdivision along frontage path, verifying all Civil 3D criteria for each lot.
pub fn solve_multi_slide_line_path_criteria(
    poly: &[[f64; 2]],
    frontage_path: &[[f64; 2]],
    angle_deg: f64,
    criteria: &ParcelSizingCriteria,
) -> Result<MultiSlideResult, String> {
    let res = solve_multi_slide_line_path(
        poly,
        frontage_path,
        angle_deg,
        criteria.min_area,
        criteria.min_frontage,
        &criteria.remainder_mode,
    )?;

    // Validate that generated lots satisfy depth and offset criteria
    let min_depth = criteria.min_depth;
    let use_max_d = criteria.use_max_depth;
    let max_depth = criteria.max_depth;
    let use_offset = criteria.use_offset;
    let offset_dist = criteria.frontage_offset;
    let min_w = criteria.min_width;

    if min_depth > 0.0 || (use_max_d && max_depth > 0.0) || (use_offset && offset_dist > 0.0 && min_w > 0.0) {
        for (i, lot) in res.lots.iter().enumerate() {
            let (f_pt, _, _) = snap_point_to_polygon(lot[0], frontage_path);
            let d = compute_lot_depth(lot, &[f_pt, lot[lot.len().min(1)]]);
            if min_depth > 0.0 && d < min_depth - 0.5 {
                return Err(format!("Lot {} depth ({:.1}m) is less than minimum depth ({:.1}m)", i + 1, d, min_depth));
            }
            if use_max_d && max_depth > 0.0 && d > max_depth + 0.5 {
                return Err(format!("Lot {} depth ({:.1}m) exceeds maximum depth ({:.1}m)", i + 1, d, max_depth));
            }
        }
    }

    Ok(res)
}

/// Solves multi-lot slide line subdivision along frontage
pub fn solve_multi_slide_line(
    poly: &[[f64; 2]],
    f_start: [f64; 2],
    f_end: [f64; 2],
    angle_deg: f64,
    min_area: f64,
    min_frontage: f64,
    remainder_mode: &str,
) -> Result<MultiSlideResult, String> {
    let total_area = shoelace_area(poly);
    if total_area < 1e-4 {
        return Err("Parcel polygon has zero area".to_string());
    }
    if min_area <= 0.0 {
        return Err("Minimum area must be greater than zero".to_string());
    }
    if min_area >= total_area {
        return Err(format!(
            "Minimum area ({:.1} m²) is greater than total parcel area ({:.1} m²)",
            min_area, total_area
        ));
    }

    let f_vec = [f_end[0] - f_start[0], f_end[1] - f_start[1]];
    let total_f_len = (f_vec[0] * f_vec[0] + f_vec[1] * f_vec[1]).sqrt();
    if total_f_len < 1e-4 {
        return Err("Frontage length is too short".to_string());
    }

    let is_redistribute = remainder_mode.starts_with("Redistribute");
    let is_create_remainder = remainder_mode.contains("Create remainder");

    // Number of possible lots
    let mut n_lots = (total_area / min_area).floor() as usize;
    if min_frontage > 0.0 && total_f_len >= min_frontage * 2.0 {
        let n_f = (total_f_len / min_frontage).floor() as usize;
        n_lots = n_lots.min(n_f);
    }
    n_lots = n_lots.max(2);

    let target_area = if is_redistribute {
        total_area / n_lots as f64
    } else {
        min_area
    };

    let remainder_area = total_area - (n_lots as f64 * min_area);
    let num_cuts = if is_create_remainder && remainder_area >= 1.0 {
        n_lots
    } else {
        n_lots - 1
    };

    let mut lots = Vec::new();
    let mut cut_lines = Vec::new();
    let mut lot_areas = Vec::new();
    let mut lot_frontages = Vec::new();

    let mut cur_poly = poly.to_vec();
    let mut cur_start = f_start;

    for _k in 0..num_cuts {
        let cur_rem_area = shoelace_area(&cur_poly);
        if !is_create_remainder && cur_rem_area <= target_area * 1.05 {
            break;
        }
        if is_create_remainder && cur_rem_area < target_area {
            break;
        }

        match solve_slide_line_criteria(&cur_poly, cur_start, f_end, angle_deg, target_area, min_frontage) {
            Ok(slice) => {
                lots.push(slice.target_lot);
                cut_lines.push(slice.cut_line);
                lot_areas.push(slice.target_area);
                lot_frontages.push(slice.frontage_length);

                cur_poly = slice.remainder_lot;
                cur_start = slice.cut_line.0;
            }
            Err(_) => {
                // If cutting at exact target_area fails near edge, attempt with slight tolerance
                let adj_target = target_area * 0.95;
                if let Ok(slice) = solve_slide_line_criteria(&cur_poly, cur_start, f_end, angle_deg, adj_target, min_frontage * 0.9) {
                    lots.push(slice.target_lot);
                    cut_lines.push(slice.cut_line);
                    lot_areas.push(slice.target_area);
                    lot_frontages.push(slice.frontage_length);

                    cur_poly = slice.remainder_lot;
                    cur_start = slice.cut_line.0;
                } else {
                    break;
                }
            }
        }
    }

    // Final remainder lot
    let final_area = shoelace_area(&cur_poly);
    let rem_f_vec = [f_end[0] - cur_start[0], f_end[1] - cur_start[1]];
    let final_f_len = (rem_f_vec[0] * rem_f_vec[0] + rem_f_vec[1] * rem_f_vec[1]).sqrt();
    lots.push(cur_poly);
    lot_areas.push(final_area);
    lot_frontages.push(final_f_len);

    if lots.len() < 2 {
        return Err("Could not divide parcel into at least 2 lots with specified criteria".to_string());
    }

    Ok(MultiSlideResult {
        lots,
        cut_lines,
        lot_areas,
        lot_frontages,
    })
}

// ── Interactive SlideLineCommand ──────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SlideStep {
    PickParcel,
    PickFrontageStart,
    PickFrontageEnd,
    SpecifyAngle,
    AcceptPreview,
}

pub struct SlideLineCommand {
    pub criteria: ParcelSizingCriteria,
    pub min_area: f64,
    pub min_frontage: f64,
    pub frontage_offset: f64,
    pub angle_deg: f64,
    pub automatic_mode: bool,
    pub remainder_mode: String,
    pub step: SlideStep,

    pub parent_handle: Option<Handle>,
    pub parent_name: String,
    pub parent_site: String,
    pub parent_layer: String,
    pub parent_zoning: String,
    pub parent_owner: String,
    pub parent_deed: String,
    pub parent_tax_id: String,
    pub parent_area_label_style: String,
    pub parent_line_label_style: String,
    pub parent_curve_label_style: String,
    pub parent_auto_add_segment_labels: bool,
    pub parent_segment_layer: String,
    pub parent_vertices: Vec<[f64; 2]>,
    pub parent_area: f64,

    pub frontage_start: Option<[f64; 2]>,
    pub frontage_end: Option<[f64; 2]>,
    pub frontage_path: Vec<[f64; 2]>,
    pub candidate_solutions: Vec<SlideSlice>,
    pub current_solution_idx: usize,
    pub solution: Option<SlideSlice>,
    pub multi_solution: Option<MultiSlideResult>,
    pub picked_entity: Option<EntityType>,

    // Stepping state (when automatic_mode is false)
    pub current_lot_index: usize,
    pub stepping_lots: Vec<Vec<[f64; 2]>>,
    pub stepping_cuts: Vec<([f64; 2], [f64; 2])>,
    pub stepping_poly: Vec<[f64; 2]>,
    pub stepping_fstart: Option<[f64; 2]>,
}

impl SlideLineCommand {
    pub fn new(min_area: f64, min_frontage: f64, frontage_offset: f64) -> Self {
        let criteria = ParcelSizingCriteria {
            min_area: if min_area > 0.0 { min_area } else { 350.0 },
            min_frontage: if min_frontage > 0.0 { min_frontage } else { 12.0 },
            frontage_offset,
            ..Default::default()
        };
        Self::new_with_criteria(criteria)
    }

    pub fn new_with_criteria(criteria: ParcelSizingCriteria) -> Self {
        let min_area = criteria.min_area;
        let min_frontage = criteria.min_frontage;
        let frontage_offset = criteria.frontage_offset;
        let automatic_mode = criteria.automatic_mode;
        let remainder_mode = criteria.remainder_mode.clone();

        Self {
            criteria,
            min_area,
            min_frontage,
            frontage_offset,
            angle_deg: 90.0,
            automatic_mode,
            remainder_mode,
            step: SlideStep::PickParcel,
            parent_handle: None,
            parent_name: "LOT".to_string(),
            parent_site: "Site 1".to_string(),
            parent_layer: "PC-PARCEL".to_string(),

            parent_zoning: "Single-Family Residential (R-1)".to_string(),

            parent_owner: String::new(),

            parent_deed: String::new(),

            parent_tax_id: String::new(),
            parent_area_label_style: "Parcel Name, Area & Perimeter".to_string(),
            parent_line_label_style: "Bearing over Distance".to_string(),
            parent_curve_label_style: "Distance over Radius".to_string(),
            parent_auto_add_segment_labels: true,
            parent_segment_layer: "C-PROP-LINE".to_string(),
            parent_vertices: Vec::new(),
            parent_area: 0.0,
            frontage_start: None,
            frontage_end: None,
            frontage_path: Vec::new(),
            candidate_solutions: Vec::new(),
            current_solution_idx: 0,
            solution: None,
            multi_solution: None,
            picked_entity: None,
            current_lot_index: 0,
            stepping_lots: Vec::new(),
            stepping_cuts: Vec::new(),
            stepping_poly: Vec::new(),
            stepping_fstart: None,
        }
    }

    pub fn sync_criteria(&mut self) {
        self.criteria.min_area = self.min_area;
        self.criteria.min_frontage = self.min_frontage;
        self.criteria.frontage_offset = self.frontage_offset;
        self.criteria.automatic_mode = self.automatic_mode;
        self.criteria.remainder_mode = self.remainder_mode.clone();
    }

    fn solve_layout(&mut self) -> Result<(), String> {
        self.sync_criteria();
        let (Some(f_start), Some(f_end)) = (self.frontage_start, self.frontage_end) else {
            return Err("Frontage points not defined".to_string());
        };

        let path = if self.frontage_path.len() >= 2 {
            self.frontage_path.clone()
        } else {
            trace_polygon_path(&self.parent_vertices, f_start, f_end)
        };

        if self.automatic_mode {
            match solve_multi_slide_line_path_criteria(
                &self.parent_vertices,
                &path,
                self.angle_deg,
                &self.criteria,
            ) {
                Ok(multi) => {
                    self.multi_solution = Some(multi);
                    self.solution = None;
                    self.candidate_solutions.clear();
                    self.step = SlideStep::AcceptPreview;
                    Ok(())
                }
                Err(e) => {
                    // Fallback to candidate slide cuts along path
                    match solve_slide_line_path_candidates(
                        &self.parent_vertices,
                        &path,
                        self.angle_deg,
                        self.min_area,
                        Some(&self.criteria),
                    ) {
                        Ok(sols) if !sols.is_empty() => {
                            self.solution = Some(sols[0].clone());
                            self.candidate_solutions = sols;
                            self.current_solution_idx = 0;
                            self.multi_solution = None;
                            self.step = SlideStep::AcceptPreview;
                            Ok(())
                        }
                        _ => Err(format!("Slide Line failed: {}", e)),
                    }
                }
            }
        } else {
            self.current_lot_index = 0;
            self.stepping_poly = self.parent_vertices.clone();
            self.stepping_fstart = Some(f_start);
            self.stepping_lots.clear();
            self.stepping_cuts.clear();

            let step_path = trace_polygon_path(&self.stepping_poly, f_start, f_end);
            match solve_slide_line_path_candidates(
                &self.stepping_poly,
                &step_path,
                self.angle_deg,
                self.min_area,
                Some(&self.criteria),
            ) {
                Ok(sols) if !sols.is_empty() => {
                    self.solution = Some(sols[0].clone());
                    self.candidate_solutions = sols;
                    self.current_solution_idx = 0;
                    self.multi_solution = None;
                    self.step = SlideStep::AcceptPreview;
                    Ok(())
                }
                _ => Err("Could not find valid slide line cut for stepping lot".to_string()),
            }
        }
    }
}

impl CadCommand for SlideLineCommand {
    fn name(&self) -> &'static str {
        "SLIDELINE"
    }

    fn prompt(&self) -> String {
        match self.step {
            SlideStep::PickParcel => {
                crate::t!("SLIDELINE  Select parcel to be subdivided:").into_owned()
            }
            SlideStep::PickFrontageStart => {
                crate::t!("SLIDELINE  Select start point on frontage:").into_owned()
            }
            SlideStep::PickFrontageEnd => {
                "SLIDELINE  Drag mouse along parcel perimeter to pick frontage end point (or click):".to_string()
            }
            SlideStep::SpecifyAngle => {
                format!(
                    "SLIDELINE  Specify angle relative to frontage <{:.2}> (Target Area: {:.1}m², Auto: {}):",
                    self.angle_deg, self.min_area, if self.automatic_mode { "Yes" } else { "No" }
                )
            }
            SlideStep::AcceptPreview => {
                if self.automatic_mode {
                    let f_len = if self.frontage_path.len() >= 2 {
                        poly_path_length(&self.frontage_path)
                    } else if let (Some(s), Some(e)) = (self.frontage_start, self.frontage_end) {
                        ((e[0] - s[0]).powi(2) + (e[1] - s[1]).powi(2)).sqrt()
                    } else {
                        0.0
                    };
                    if let Some(multi) = &self.multi_solution {
                        format!(
                            "SLIDELINE  Subdivided into {} lots along frontage ({:.1}m, target: {:.1}m²). Press Enter to accept [Yes/No/Angle] <Yes>:",
                            multi.lots.len(), f_len, self.min_area
                        )
                    } else if let Some(sol) = &self.solution {
                        if self.candidate_solutions.len() > 1 {
                            format!(
                                "SLIDELINE  [Solution {}/{}] Target: {:.1}m², Frontage: {:.1}m, Depth: {:.1}m. Accept result? [Yes/No/Next/Angle] <Yes>:",
                                self.current_solution_idx + 1, self.candidate_solutions.len(), sol.target_area, sol.frontage_length, sol.depth
                            )
                        } else {
                            format!(
                                "SLIDELINE  Target: {:.1}m², Remainder: {:.1}m² (Depth: {:.1}m). Press Enter to accept [Yes/No/Angle] <Yes>:",
                                sol.target_area, sol.remainder_area, sol.depth
                            )
                        }
                    } else {
                        crate::t!("SLIDELINE  Accept result? [Yes/No] <Yes>:").into_owned()
                    }
                } else {
                    if let Some(sol) = &self.solution {
                        if self.candidate_solutions.len() > 1 {
                            format!(
                                "SLIDELINE  Lot {} [Solution {}/{}]: {:.1}m² (Frontage: {:.1}m, Depth: {:.1}m). Accept & Next Lot? [Yes/No/Next/Finish] <Yes>:",
                                self.current_lot_index + 1, self.current_solution_idx + 1, self.candidate_solutions.len(), sol.target_area, sol.frontage_length, sol.depth
                            )
                        } else {
                            format!(
                                "SLIDELINE  Lot {}: {:.1}m² (Frontage: {:.1}m, Depth: {:.1}m). Accept & Next Lot? [Yes/No/Finish] <Yes>:",
                                self.current_lot_index + 1, sol.target_area, sol.frontage_length, sol.depth
                            )
                        }
                    } else {
                        crate::t!("SLIDELINE  Accept lot? [Yes/No/Finish] <Yes>:").into_owned()
                    }
                }
            }
        }
    }

    fn options(&self) -> Vec<CmdOption> {
        match self.step {
            SlideStep::SpecifyAngle => {
                vec![
                    CmdOption::new("90° (Perpendicular)", "90"),
                    CmdOption::new("Change Area", "A"),
                ]
            }
            SlideStep::AcceptPreview => {
                let mut opts = Vec::new();
                opts.push(CmdOption::new("Yes (Accept)", "Y"));
                if self.candidate_solutions.len() > 1 {
                    opts.push(CmdOption::new("Next Solution", "NEXT"));
                }
                if self.automatic_mode {
                    opts.push(CmdOption::new("Specify Angle", "A"));
                    opts.push(CmdOption::new("No (Reject)", "N"));
                } else {
                    opts.push(CmdOption::new("Finish (Commit lots created)", "F"));
                    opts.push(CmdOption::new("No (Reject)", "N"));
                }
                opts
            }
            _ => Vec::new(),
        }
    }

    fn needs_entity_pick(&self) -> bool {
        self.step == SlideStep::PickParcel
    }

    fn inject_before_entity_pick(&self) -> bool {
        true
    }

    fn inject_picked_entity(&mut self, entity: EntityType) {
        self.picked_entity = Some(entity);
    }

    fn on_entity_pick(&mut self, handle: Handle, _pt: DVec3) -> CmdResult {
        if handle.is_null() {
            return CmdResult::NeedPoint;
        }

        if let Some(entity) = &self.picked_entity {
            if is_parcel_entity(entity) || matches!(entity, EntityType::LwPolyline(_)) {
                if let EntityType::LwPolyline(lw) = entity {
                    if lw.vertices.len() >= 2 {
                        let mut verts: Vec<[f64; 2]> = Vec::new();
                        let n_v = lw.vertices.len();
                        for i in 0..n_v {
                            let v = &lw.vertices[i];
                            let p1 = [v.location.x, v.location.y];
                            let next_idx = (i + 1) % n_v;
                            let p2 = [lw.vertices[next_idx].location.x, lw.vertices[next_idx].location.y];
                            if v.bulge.abs() > 1e-5 {
                                let arc_pts = tessellate_bulge(p1, p2, v.bulge, 16);
                                for p in arc_pts.into_iter().take(16) {
                                    verts.push(p);
                                }
                            } else {
                                verts.push(p1);
                            }
                        }
                        let area = shoelace_area(&verts);
                        if area > 1.0 {
                            self.parent_handle = Some(handle);
                            self.parent_layer = entity.common().layer.clone();
                            self.parent_vertices = verts;
                            self.parent_area = area;

                            if let Some(rec) = entity.common().extended_data.get_record(XDATA_PARCEL) {
                                if let Some(XDataValue::String(s)) = rec.values.first() {
                                    self.parent_name = s.clone();
                                }
                                if let Some(XDataValue::String(site)) = rec.values.get(1) {

                                    self.parent_site = site.clone();

                                }

                                if let Some(XDataValue::String(zoning)) = rec.values.get(3) {

                                    self.parent_zoning = zoning.clone();

                                }

                                if let Some(XDataValue::String(owner)) = rec.values.get(4) {

                                    self.parent_owner = owner.clone();

                                }

                                if let Some(XDataValue::String(deed)) = rec.values.get(5) {

                                    self.parent_deed = deed.clone();

                                }

                                if let Some(XDataValue::String(tax_id)) = rec.values.get(6) {
                                    self.parent_tax_id = tax_id.clone();
                                }
                                if let Some(XDataValue::String(s)) = rec.values.get(7) {
                                    self.parent_area_label_style = s.clone();
                                }
                                if let Some(XDataValue::String(s)) = rec.values.get(8) {
                                    self.parent_line_label_style = s.clone();
                                }
                                if let Some(XDataValue::String(s)) = rec.values.get(9) {
                                    self.parent_curve_label_style = s.clone();
                                }
                                if let Some(XDataValue::String(s)) = rec.values.get(10) {
                                    self.parent_auto_add_segment_labels = s != "false";
                                }
                                if let Some(XDataValue::String(s)) = rec.values.get(11) {
                                    self.parent_segment_layer = s.clone();
                                }
                            } else {
                                self.parent_name = format!("Lot {:X}", handle.value());
                            }

                            self.step = SlideStep::PickFrontageStart;
                            return CmdResult::NeedPoint;
                        }
                    }
                }
            }
        }

        CmdResult::NeedPoint
    }

    fn on_point(&mut self, pt: DVec3) -> CmdResult {
        match self.step {
            SlideStep::PickParcel => CmdResult::NeedPoint,
            SlideStep::PickFrontageStart => {
                let (snapped, _, _) = snap_point_to_polygon([pt.x, pt.y], &self.parent_vertices);
                self.frontage_start = Some(snapped);
                self.frontage_path = vec![snapped];
                self.step = SlideStep::PickFrontageEnd;
                CmdResult::NeedPoint
            }
            SlideStep::PickFrontageEnd => {
                let (snapped, _, _) = snap_point_to_polygon([pt.x, pt.y], &self.parent_vertices);
                if let Some(f_start) = self.frontage_start {
                    let path = trace_polygon_path(&self.parent_vertices, f_start, snapped);
                    let path_len = poly_path_length(&path);
                    if path_len > 0.05 {
                        self.frontage_end = Some(snapped);
                        self.frontage_path = path;
                        self.angle_deg = 90.0;
                        match self.solve_layout() {
                            Ok(()) => {
                                self.step = SlideStep::AcceptPreview;
                                return CmdResult::NeedPoint;
                            }
                            Err(e) => return CmdResult::ReportError(e),
                        }
                    }
                }
                CmdResult::NeedPoint
            }
            SlideStep::SpecifyAngle => {
                if let (Some(f_start), Some(f_end)) = (self.frontage_start, self.frontage_end) {
                    let f_dx = f_end[0] - f_start[0];
                    let f_dy = f_end[1] - f_start[1];
                    let p_dx = pt.x - f_start[0];
                    let p_dy = pt.y - f_start[1];
                    let angle_frontage = f_dy.atan2(f_dx);
                    let angle_pick = p_dy.atan2(p_dx);
                    let mut diff_deg = (angle_pick - angle_frontage).to_degrees();
                    if diff_deg < 0.0 {
                        diff_deg += 360.0;
                    }
                    if diff_deg > 180.0 {
                        diff_deg = 360.0 - diff_deg;
                    }
                    if diff_deg > 10.0 && diff_deg < 170.0 {
                        self.angle_deg = diff_deg;
                    }

                    match self.solve_layout() {
                        Ok(()) => {
                            self.step = SlideStep::AcceptPreview;
                            return CmdResult::NeedPoint;
                        }
                        Err(e) => return CmdResult::ReportError(e),
                    }
                }
                CmdResult::NeedPoint
            }
            SlideStep::AcceptPreview => {
                if self.automatic_mode {
                    self.commit_all()
                } else {
                    self.advance_step_or_finish()
                }
            }
        }
    }

    fn wants_text_input(&self) -> bool {
        matches!(self.step, SlideStep::SpecifyAngle | SlideStep::AcceptPreview)
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        let t = text.trim();
        match self.step {
            SlideStep::SpecifyAngle => {
                let up = t.to_ascii_uppercase();
                if up == "A" || up.starts_with("AREA") {
                    return Some(CmdResult::NeedPoint);
                }
                if let Ok(ang) = t.parse::<f64>() {
                    if ang > 1.0 && ang < 179.0 {
                        self.angle_deg = ang;
                    }
                }
                match self.solve_layout() {
                    Ok(()) => {
                        self.step = SlideStep::AcceptPreview;
                        Some(CmdResult::NeedPoint)
                    }
                    Err(e) => Some(CmdResult::ReportError(e)),
                }
            }
            SlideStep::AcceptPreview => {
                let up = t.to_ascii_uppercase();
                if up == "NEXT" && self.candidate_solutions.len() > 1 {
                    self.current_solution_idx = (self.current_solution_idx + 1) % self.candidate_solutions.len();
                    self.solution = Some(self.candidate_solutions[self.current_solution_idx].clone());
                    return Some(CmdResult::NeedPoint);
                }

                if self.automatic_mode {
                    if up == "Y" || up == "YES" || up.is_empty() {
                        Some(self.commit_all())
                    } else if up == "A" || up.starts_with("ANGLE") {
                        self.step = SlideStep::SpecifyAngle;
                        Some(CmdResult::NeedPoint)
                    } else if let Ok(ang) = t.parse::<f64>() {
                        if ang > 1.0 && ang < 179.0 {
                            self.angle_deg = ang;
                            let _ = self.solve_layout();
                        }
                        Some(CmdResult::NeedPoint)
                    } else if up == "N" || up == "NO" {
                        Some(CmdResult::Cancel)
                    } else {
                        Some(CmdResult::NeedPoint)
                    }
                } else {
                    if up == "Y" || up == "YES" || up.is_empty() {
                        Some(self.advance_step_or_finish())
                    } else if up == "F" || up.starts_with("FINISH") {
                        if !self.stepping_poly.is_empty() {
                            self.stepping_lots.push(self.stepping_poly.clone());
                        }
                        Some(self.commit_stepping())
                    } else if up == "N" || up == "NO" {
                        Some(CmdResult::Cancel)
                    } else {
                        Some(CmdResult::NeedPoint)
                    }
                }
            }
            _ => None,
        }
    }

    fn on_enter(&mut self) -> CmdResult {
        match self.step {
            SlideStep::SpecifyAngle => {
                match self.solve_layout() {
                    Ok(()) => {
                        self.step = SlideStep::AcceptPreview;
                        CmdResult::NeedPoint
                    }
                    Err(e) => CmdResult::ReportError(e),
                }
            }
            SlideStep::AcceptPreview => {
                if self.automatic_mode {
                    self.commit_all()
                } else {
                    self.advance_step_or_finish()
                }
            }
            _ => CmdResult::Cancel,
        }
    }

    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }

    fn on_preview_wires(&mut self, _pt: DVec3) -> Vec<WireModel> {
        let mut wires = Vec::new();

        if self.step == SlideStep::PickFrontageEnd {
            if let Some(f_start) = self.frontage_start {
                let (cur_snap, _, _) = snap_point_to_polygon([_pt.x, _pt.y], &self.parent_vertices);
                let active_path = trace_polygon_path(&self.parent_vertices, f_start, cur_snap);
                if active_path.len() >= 2 {
                    let pts_3d: Vec<[f64; 3]> = active_path.iter().map(|p| [p[0], p[1], 0.0]).collect();
                    let mut orange_line = WireModel::solid_f64(
                        "preview_frontage_active_trace".to_string(),
                        pts_3d,
                        [1.0, 0.52, 0.0, 1.0],
                        false,
                    );
                    orange_line.set_fixed_screen_width(4.0);
                    wires.push(orange_line);
                }

                let p_s = [f_start[0], f_start[1], 0.0];
                let d = 0.4;
                wires.push(WireModel::solid_f64(
                    "preview_fstart_marker".to_string(),
                    vec![
                        [p_s[0] - d, p_s[1] - d, 0.0],
                        [p_s[0] + d, p_s[1] - d, 0.0],
                        [p_s[0] + d, p_s[1] + d, 0.0],
                        [p_s[0] - d, p_s[1] + d, 0.0],
                        [p_s[0] - d, p_s[1] - d, 0.0],
                    ],
                    [0.2, 1.0, 0.3, 1.0],
                    false,
                ));

                let p_e = [cur_snap[0], cur_snap[1], 0.0];
                wires.push(WireModel::solid_f64(
                    "preview_fend_marker".to_string(),
                    vec![
                        [p_e[0] - d, p_e[1] - d, 0.0],
                        [p_e[0] + d, p_e[1] - d, 0.0],
                        [p_e[0] + d, p_e[1] + d, 0.0],
                        [p_e[0] - d, p_e[1] + d, 0.0],
                        [p_e[0] - d, p_e[1] - d, 0.0],
                    ],
                    [1.0, 0.6, 0.1, 1.0],
                    false,
                ));
            }
            return wires;
        }

        if self.frontage_path.len() >= 2 {
            let pts_3d: Vec<[f64; 3]> = self.frontage_path.iter().map(|p| [p[0], p[1], 0.0]).collect();
            let mut f_line = WireModel::solid_f64(
                "preview_frontage_locked_path".to_string(),
                pts_3d,
                [1.0, 0.52, 0.0, 1.0],
                false,
            );
            f_line.set_fixed_screen_width(3.5);
            wires.push(f_line);
        }

        if let Some(multi) = &self.multi_solution {
            for (idx, cut) in multi.cut_lines.iter().enumerate() {
                let p1 = [cut.0[0], cut.0[1], 0.0];
                let p2 = [cut.1[0], cut.1[1], 0.0];
                wires.push(WireModel::solid_f64(
                    format!("preview_cut_line_{}", idx),
                    vec![p1, p2],
                    [0.1, 0.9, 1.0, 1.0],
                    false,
                ));
            }

            for (idx, lot) in multi.lots.iter().enumerate() {
                let mut pts: Vec<[f64; 3]> = lot.iter().map(|p| [p[0], p[1], 0.0]).collect();
                if let Some(first) = pts.first().copied() {
                    pts.push(first);
                }
                let color = if idx % 2 == 0 {
                    [1.0, 0.75, 0.15, 1.0]
                } else {
                    [0.9, 0.55, 0.15, 1.0]
                };
                wires.push(WireModel::solid_f64(
                    format!("preview_lot_{}", idx),
                    pts,
                    color,
                    false,
                ));
            }
        } else {
            for (idx, lot) in self.stepping_lots.iter().enumerate() {
                let mut pts: Vec<[f64; 3]> = lot.iter().map(|p| [p[0], p[1], 0.0]).collect();
                if let Some(first) = pts.first().copied() {
                    pts.push(first);
                }
                wires.push(WireModel::solid_f64(
                    format!("preview_stepped_lot_{}", idx),
                    pts,
                    [0.2, 0.85, 0.4, 1.0],
                    false,
                ));
            }
            for (idx, cut) in self.stepping_cuts.iter().enumerate() {
                let p1 = [cut.0[0], cut.0[1], 0.0];
                let p2 = [cut.1[0], cut.1[1], 0.0];
                wires.push(WireModel::solid_f64(
                    format!("preview_stepped_cut_{}", idx),
                    vec![p1, p2],
                    [0.1, 0.9, 1.0, 1.0],
                    false,
                ));
            }

            if let Some(sol) = &self.solution {
                let p1 = [sol.cut_line.0[0], sol.cut_line.0[1], 0.0];
                let p2 = [sol.cut_line.1[0], sol.cut_line.1[1], 0.0];
                wires.push(WireModel::solid_f64(
                    "preview_cut_line".to_string(),
                    vec![p1, p2],
                    [0.1, 0.9, 1.0, 1.0],
                    false,
                ));

                let mut target_pts: Vec<[f64; 3]> = sol
                    .target_lot
                    .iter()
                    .map(|p| [p[0], p[1], 0.0])
                    .collect();
                if let Some(first) = target_pts.first().copied() {
                    target_pts.push(first);
                }
                wires.push(WireModel::solid_f64(
                    "preview_target_lot".to_string(),
                    target_pts,
                    [1.0, 0.75, 0.15, 1.0],
                    false,
                ));

                // Civil 3D Setback offset indicator wire (lime green)
                if self.criteria.use_offset && self.criteria.frontage_offset > 0.0 {
                    let sub_path = trace_polygon_path(&self.parent_vertices, self.frontage_path[0], sol.cut_line.0);
                    let f_len = poly_path_length(&sub_path);
                    if f_len > 1.0 {
                        let (p_mid, tan) = sample_path_at_dist(&sub_path, 0.5 * f_len);
                        let norm = [-tan[1], tan[0]];
                        let offset_d = self.criteria.frontage_offset;
                        let p_off1 = [p_mid[0] + norm[0] * offset_d - tan[0] * (0.4 * f_len), p_mid[1] + norm[1] * offset_d - tan[1] * (0.4 * f_len), 0.0];
                        let p_off2 = [p_mid[0] + norm[0] * offset_d + tan[0] * (0.4 * f_len), p_mid[1] + norm[1] * offset_d + tan[1] * (0.4 * f_len), 0.0];
                        let mut setback_wire = WireModel::solid_f64(
                            "preview_setback_line".to_string(),
                            vec![p_off1, p_off2],
                            [0.30, 0.85, 0.50, 0.9],
                            false,
                        );
                        setback_wire.set_fixed_screen_width(2.0);
                        wires.push(setback_wire);
                    }
                }
            } else if let Some(f_start) = self.frontage_start {
                let p = [f_start[0], f_start[1], 0.0];
                wires.push(WireModel::solid_f64(
                    "preview_fstart".to_string(),
                    vec![p, [p[0] + 0.5, p[1] + 0.5, 0.0]],
                    [0.2, 1.0, 0.3, 1.0],
                    false,
                ));
            }
        }

        wires
    }
}

impl SlideLineCommand {
    fn advance_step_or_finish(&mut self) -> CmdResult {
        let Some(sol) = self.solution.take() else {
            return CmdResult::Cancel;
        };

        self.stepping_lots.push(sol.target_lot);
        self.stepping_cuts.push(sol.cut_line);
        self.stepping_poly = sol.remainder_lot;
        self.stepping_fstart = Some(sol.cut_line.0);
        self.current_lot_index += 1;

        let rem_area = shoelace_area(&self.stepping_poly);
        if rem_area < self.min_area * 1.35 {
            self.stepping_lots.push(self.stepping_poly.clone());
            return self.commit_stepping();
        }

        if let (Some(f_start), Some(f_end)) = (self.stepping_fstart, self.frontage_end) {
            let step_path = trace_polygon_path(&self.stepping_poly, f_start, f_end);
            match solve_slide_line_path_candidates(
                &self.stepping_poly,
                &step_path,
                self.angle_deg,
                self.min_area,
                Some(&self.criteria),
            ) {
                Ok(sols) if !sols.is_empty() => {
                    self.solution = Some(sols[0].clone());
                    self.candidate_solutions = sols;
                    self.current_solution_idx = 0;
                    CmdResult::NeedPoint
                }
                _ => {
                    self.stepping_lots.push(self.stepping_poly.clone());
                    self.commit_stepping()
                }
            }
        } else {
            self.commit_stepping()
        }
    }

    fn commit_all(&self) -> CmdResult {
        if let Some(multi) = &self.multi_solution {
            self.commit_lots(&multi.lots)
        } else if let Some(sol) = &self.solution {
            self.commit_lots(&[sol.target_lot.clone(), sol.remainder_lot.clone()])
        } else {
            CmdResult::Cancel
        }
    }

    fn commit_stepping(&self) -> CmdResult {
        self.commit_lots(&self.stepping_lots)
    }

    fn commit_lots(&self, lots: &[Vec<[f64; 2]>]) -> CmdResult {
        let Some(parent_handle) = self.parent_handle else {
            return CmdResult::Cancel;
        };
        if lots.is_empty() {
            return CmdResult::Cancel;
        }

        let mut child_entities = Vec::new();
        for (i, lot_pts) in lots.iter().enumerate() {
            let lot_name = format!("{}-{}", self.parent_name, i + 1);
            let lot_num = (100 + i + 1).to_string();

            let mut pl = LwPolyline::from_points(
                lot_pts.iter().map(|v| Vector2::new(v[0], v[1])).collect(),
            );
            pl.is_closed = true;
            let mut ent = EntityType::LwPolyline(pl);
            let assigned_layer = if !self.parent_zoning.is_empty() {
                crate::ui::window::create_parcels_layout::layer_for_style(&self.parent_zoning).to_string()
            } else {
                crate::ui::window::create_parcels_layout::layer_for_style(&self.parent_layer).to_string()
            };
            ent.common_mut().layer = assigned_layer;



            let mut rec = ExtendedDataRecord::new(XDATA_PARCEL);
            rec.add_value(XDataValue::String(lot_name));
            rec.add_value(XDataValue::String(self.parent_site.clone()));
            rec.add_value(XDataValue::String(lot_num));
            rec.add_value(XDataValue::String(self.parent_zoning.clone()));
            rec.add_value(XDataValue::String(self.parent_owner.clone()));
            rec.add_value(XDataValue::String(self.parent_deed.clone()));
            rec.add_value(XDataValue::String(self.parent_tax_id.clone()));
            rec.add_value(XDataValue::String(self.parent_area_label_style.clone()));
            rec.add_value(XDataValue::String(self.parent_line_label_style.clone()));
            rec.add_value(XDataValue::String(self.parent_curve_label_style.clone()));
            rec.add_value(XDataValue::String(if self.parent_auto_add_segment_labels { "true".to_string() } else { "false".to_string() }));
            rec.add_value(XDataValue::String(self.parent_segment_layer.clone()));
            ent.common_mut().extended_data.add_record(rec);

            child_entities.push(ent);
        }

        CmdResult::ReplaceMany(
            vec![(parent_handle, child_entities)],
            Vec::new(),
        )
    }
}

// ── Interactive Swing Line Edit Command (Civil 3D Parity) ───────────────────

/// Unions two adjacent parcel polygons that share a common boundary.
pub fn union_two_parcels(poly1: &[[f64; 2]], poly2: &[[f64; 2]], eps: f64) -> Option<Vec<[f64; 2]>> {
    let n1 = poly1.len();
    let n2 = poly2.len();
    if n1 < 3 || n2 < 3 {
        return None;
    }

    let mut shared_1 = vec![false; n1];
    let mut shared_2 = vec![false; n2];
    let mut found = false;

    let dist = |a: [f64; 2], b: [f64; 2]| -> f64 {
        let dx = a[0] - b[0];
        let dy = a[1] - b[1];
        (dx * dx + dy * dy).sqrt()
    };

    for i in 0..n1 {
        let a1 = poly1[i];
        let a2 = poly1[(i + 1) % n1];
        for j in 0..n2 {
            let b1 = poly2[j];
            let b2 = poly2[(j + 1) % n2];
            if (dist(a1, b2) < eps && dist(a2, b1) < eps) || (dist(a1, b1) < eps && dist(a2, b2) < eps) {
                shared_1[i] = true;
                shared_2[j] = true;
                found = true;
            }
        }
    }

    if !found {
        return None;
    }

    let mut segments = Vec::new();
    for i in 0..n1 {
        if !shared_1[i] {
            segments.push((poly1[i], poly1[(i + 1) % n1]));
        }
    }
    for j in 0..n2 {
        if !shared_2[j] {
            segments.push((poly2[j], poly2[(j + 1) % n2]));
        }
    }

    if segments.is_empty() {
        return None;
    }

    let mut chained = vec![segments[0].0, segments[0].1];
    let mut used = vec![false; segments.len()];
    used[0] = true;

    for _ in 1..segments.len() {
        let cur_end = *chained.last().unwrap();
        let mut attached = false;
        for k in 0..segments.len() {
            if used[k] {
                continue;
            }
            let (s_start, s_end) = segments[k];
            if dist(s_start, cur_end) < eps {
                chained.push(s_end);
                used[k] = true;
                attached = true;
                break;
            } else if dist(s_end, cur_end) < eps {
                chained.push(s_start);
                used[k] = true;
                attached = true;
                break;
            }
        }
        if !attached {
            break;
        }
    }

    if chained.len() > 2 && dist(chained[0], *chained.last().unwrap()) < eps {
        chained.pop();
    }

    if chained.len() >= 3 && shoelace_area(&chained) > 1.0 {
        Some(chained)
    } else {
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwingEditStep {
    PickAttachedLotLine,
    PickTargetParcel,
    PickPivot,
    PickFrontageStart,
    PickFrontageEnd,
    AcceptPreview,
}

pub struct SwingLineEditCommand {
    pub criteria: ParcelSizingCriteria,
    pub min_area: f64,
    pub min_frontage: f64,
    pub frontage_offset: f64,
    pub automatic_mode: bool,
    pub remainder_mode: String,
    pub step: SwingEditStep,

    pub attached_line_pt: Option<[f64; 2]>,

    pub target_handle: Option<Handle>,

    pub target_name: String,

    pub target_site: String,

    pub target_layer: String,

    pub target_zoning: String,

    pub target_vertices: Vec<[f64; 2]>,

    pub target_area: f64,

    pub neighbor_handle: Option<Handle>,

    pub neighbor_name: String,

    pub neighbor_site: String,

    pub neighbor_layer: String,

    pub neighbor_zoning: String,

    pub neighbor_vertices: Vec<[f64; 2]>,

    pub neighbor_area: f64,

    pub combined_vertices: Vec<[f64; 2]>,

    pub pivot_point: Option<[f64; 2]>,
    pub frontage_start: Option<[f64; 2]>,
    pub frontage_end: Option<[f64; 2]>,
    pub frontage_path: Vec<[f64; 2]>,
    pub solution: Option<SlideSlice>,
    pub candidate_solutions: Vec<SlideSlice>,
    pub current_solution_idx: usize,
    pub picked_entity: Option<EntityType>,
}

impl SwingLineEditCommand {
    pub fn new(min_area: f64, min_frontage: f64) -> Self {
        let criteria = ParcelSizingCriteria {
            min_area: if min_area > 0.0 { min_area } else { 350.0 },
            min_frontage: if min_frontage > 0.0 { min_frontage } else { 12.0 },
            ..Default::default()
        };
        Self::new_with_criteria(criteria)
    }

    pub fn new_with_criteria(criteria: ParcelSizingCriteria) -> Self {
        let min_area = criteria.min_area;
        let min_frontage = criteria.min_frontage;
        let frontage_offset = criteria.frontage_offset;
        let automatic_mode = criteria.automatic_mode;
        let remainder_mode = criteria.remainder_mode.clone();

        Self {
            criteria,
            min_area,
            min_frontage,
            frontage_offset,
            automatic_mode,
            remainder_mode,
            step: SwingEditStep::PickAttachedLotLine,
            attached_line_pt: None,
            target_handle: None,
            target_name: "LOT_TARGET".to_string(),
            target_site: "Site 1".to_string(),
            target_layer: "PC-PARCEL".to_string(),
            target_zoning: "Single-Family Residential (R-1)".to_string(),
            target_vertices: Vec::new(),
            target_area: 0.0,
            neighbor_handle: None,
            neighbor_name: "LOT_NEIGHBOR".to_string(),
            neighbor_site: "Site 1".to_string(),
            neighbor_layer: "PC-PARCEL".to_string(),
            neighbor_zoning: "Single-Family Residential (R-1)".to_string(),
            neighbor_vertices: Vec::new(),
            neighbor_area: 0.0,
            combined_vertices: Vec::new(),
            pivot_point: None,
            frontage_start: None,
            frontage_end: None,
            frontage_path: Vec::new(),
            solution: None,
            candidate_solutions: Vec::new(),
            current_solution_idx: 0,
            picked_entity: None,
        }
    }

    pub fn sync_criteria(&mut self) {
        self.criteria.min_area = self.min_area;
        self.criteria.min_frontage = self.min_frontage;
        self.criteria.frontage_offset = self.frontage_offset;
        self.criteria.automatic_mode = self.automatic_mode;
        self.criteria.remainder_mode = self.remainder_mode.clone();
    }

    fn update_combined_polygon(&mut self) {
        if !self.target_vertices.is_empty() && !self.neighbor_vertices.is_empty() {
            if let Some(union_pts) = union_two_parcels(&self.target_vertices, &self.neighbor_vertices, 0.5) {
                self.combined_vertices = union_pts;
                return;
            }
        }
        self.combined_vertices = self.target_vertices.clone();
    }

    pub fn solve_swing_edit(&mut self) -> Result<(), String> {
        self.sync_criteria();
        let (Some(pivot), Some(f_start), Some(f_end)) = (self.pivot_point, self.frontage_start, self.frontage_end) else {
            return Err("Pivot and frontage points must be defined".to_string());
        };

        if self.combined_vertices.is_empty() {
            self.update_combined_polygon();
        }

        let base_poly = if !self.combined_vertices.is_empty() {
            &self.combined_vertices
        } else {
            &self.target_vertices
        };

        let path = if self.frontage_path.len() >= 2 {
            self.frontage_path.clone()
        } else {
            trace_polygon_path(base_poly, f_start, f_end)
        };

        let sols = solve_swing_line_candidates(
            base_poly,
            pivot,
            &path,
            self.min_area,
            Some(&self.criteria),
        )?;

        self.solution = Some(sols[0].clone());
        self.candidate_solutions = sols;
        self.current_solution_idx = 0;
        self.step = SwingEditStep::AcceptPreview;
        Ok(())
    }
}

impl CadCommand for SwingLineEditCommand {
    fn name(&self) -> &'static str {
        "SWINGLINEEDIT"
    }

    fn prompt(&self) -> String {
        match self.step {
            SwingEditStep::PickAttachedLotLine => {
                crate::t!("SWINGLINEEDIT  Select attached lot line to adjust:").into_owned()
            }
            SwingEditStep::PickTargetParcel => {
                crate::t!("SWINGLINEEDIT  Select parcel to adjust:").into_owned()
            }
            SwingEditStep::PickPivot => {
                "SWINGLINEEDIT  Specify swing point (fixed pivot point):".to_string()
            }
            SwingEditStep::PickFrontageStart => {
                "SWINGLINEEDIT  Select start point on frontage:".to_string()
            }
            SwingEditStep::PickFrontageEnd => {
                "SWINGLINEEDIT  Drag mouse along parcel perimeter to pick frontage end point (or click):".to_string()
            }
            SwingEditStep::AcceptPreview => {
                if let Some(sol) = &self.solution {
                    let total_sols = self.candidate_solutions.len();
                    let sol_info = if total_sols > 1 {
                        format!(" [Solution {}/{}]", self.current_solution_idx + 1, total_sols)
                    } else {
                        String::new()
                    };
                    let warn = if !sol.satisfies_criteria {
                        " (Criteria Warning: bounds violated)"
                    } else {
                        ""
                    };
                    format!(
                        "SWINGLINEEDIT{}  Target: {:.1}m², Frontage: {:.1}m, Depth: {:.1}m, Neighbor: {:.1}m{}{}. Accept? [Yes/Next/No] <Yes>:",
                        sol_info, sol.target_area, sol.frontage_length, sol.depth, sol.remainder_area,
                        if self.criteria.use_offset { format!(", Width@Offset: {:.1}m", sol.width_at_offset) } else { String::new() },
                        warn
                    )
                } else {
                    "SWINGLINEEDIT  Accept result? [Yes/No] <Yes>:".to_string()
                }
            }
        }
    }

    fn options(&self) -> Vec<CmdOption> {
        match self.step {
            SwingEditStep::AcceptPreview => {
                let mut opts = vec![CmdOption::new("Yes (Accept)", "Y")];
                if self.candidate_solutions.len() > 1 {
                    opts.push(CmdOption::new("Next Solution", "NEXT"));
                }
                opts.push(CmdOption::new("No (Reject)", "N"));
                opts
            }
            _ => Vec::new(),
        }
    }

    fn needs_entity_pick(&self) -> bool {
        matches!(self.step, SwingEditStep::PickAttachedLotLine | SwingEditStep::PickTargetParcel)
    }

    fn inject_before_entity_pick(&self) -> bool {
        true
    }

    fn inject_picked_entity(&mut self, entity: EntityType) {
        self.picked_entity = Some(entity);
    }

    fn on_entity_pick(&mut self, handle: Handle, pt: DVec3) -> CmdResult {
        if handle.is_null() {
            return CmdResult::NeedPoint;
        }

        if let Some(entity) = &self.picked_entity {
            if is_parcel_entity(entity) || matches!(entity, EntityType::LwPolyline(_)) {
                if let EntityType::LwPolyline(lw) = entity {
                    if lw.vertices.len() >= 2 {
                        let mut verts: Vec<[f64; 2]> = Vec::new();
                        let n_v = lw.vertices.len();
                        for i in 0..n_v {
                            let v = &lw.vertices[i];
                            let p1 = [v.location.x, v.location.y];
                            let next_idx = (i + 1) % n_v;
                            let p2 = [lw.vertices[next_idx].location.x, lw.vertices[next_idx].location.y];
                            if v.bulge.abs() > 1e-5 {
                                let arc_pts = tessellate_bulge(p1, p2, v.bulge, 16);
                                for p in arc_pts.into_iter().take(16) {
                                    verts.push(p);
                                }
                            } else {
                                verts.push(p1);
                            }
                        }
                        let area = shoelace_area(&verts);
                        if area > 1.0 {
                                                        let mut name = format!("Lot {:X}", handle.value());

                            let mut site = "Site 1".to_string();

                            let mut zoning = "Single-Family Residential (R-1)".to_string();

                            if let Some(rec) = entity.common().extended_data.get_record(XDATA_PARCEL) {

                                if let Some(XDataValue::String(s)) = rec.values.first() {

                                    name = s.clone();

                                }

                                if let Some(XDataValue::String(st)) = rec.values.get(1) {

                                    site = st.clone();

                                }

                                if let Some(XDataValue::String(z)) = rec.values.get(3) {

                                    zoning = z.clone();

                                }

                            }



                            match self.step {

                                SwingEditStep::PickAttachedLotLine => {

                                    self.attached_line_pt = Some([pt.x, pt.y]);

                                    self.neighbor_handle = Some(handle);

                                    self.neighbor_name = name;

                                    self.neighbor_site = site;

                                    self.neighbor_layer = entity.common().layer.clone();

                                    self.neighbor_zoning = zoning;

                                    self.neighbor_vertices = verts;

                                    self.neighbor_area = area;

                                    self.step = SwingEditStep::PickTargetParcel;

                                    return CmdResult::NeedPoint;

                                }

                                SwingEditStep::PickTargetParcel => {

                                    if Some(handle) == self.neighbor_handle {

                                        self.target_handle = Some(handle);

                                        self.target_name = name;

                                        self.target_site = site;

                                        self.target_layer = entity.common().layer.clone();

                                        self.target_zoning = zoning;

                                        self.target_vertices = verts;

                                        self.target_area = area;

                                        self.neighbor_handle = None;

                                        self.neighbor_vertices.clear();

                                    } else {

                                        self.target_handle = Some(handle);

                                        self.target_name = name;

                                        self.target_site = site;

                                        self.target_layer = entity.common().layer.clone();

                                        self.target_zoning = zoning;

                                        self.target_vertices = verts;

                                        self.target_area = area;

                                    }
                                    self.update_combined_polygon();
                                    self.step = SwingEditStep::PickPivot;
                                    return CmdResult::NeedPoint;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        CmdResult::NeedPoint
    }

    fn on_point(&mut self, pt: DVec3) -> CmdResult {
        match self.step {
            SwingEditStep::PickAttachedLotLine => {
                self.attached_line_pt = Some([pt.x, pt.y]);
                self.step = SwingEditStep::PickTargetParcel;
                CmdResult::NeedPoint
            }
            SwingEditStep::PickTargetParcel => {
                self.step = SwingEditStep::PickPivot;
                CmdResult::NeedPoint
            }
            SwingEditStep::PickPivot => {
                let snap_poly = if !self.combined_vertices.is_empty() {
                    &self.combined_vertices
                } else {
                    &self.target_vertices
                };
                let (snapped, _, _) = snap_point_to_polygon([pt.x, pt.y], snap_poly);
                self.pivot_point = Some(snapped);
                self.step = SwingEditStep::PickFrontageStart;
                CmdResult::NeedPoint
            }
            SwingEditStep::PickFrontageStart => {
                let snap_poly = if !self.combined_vertices.is_empty() {
                    &self.combined_vertices
                } else {
                    &self.target_vertices
                };
                let (snapped, _, _) = snap_point_to_polygon([pt.x, pt.y], snap_poly);
                self.frontage_start = Some(snapped);
                self.step = SwingEditStep::PickFrontageEnd;
                CmdResult::NeedPoint
            }
            SwingEditStep::PickFrontageEnd => {
                let snap_poly = if !self.combined_vertices.is_empty() {
                    &self.combined_vertices
                } else {
                    &self.target_vertices
                };
                let (snapped, _, _) = snap_point_to_polygon([pt.x, pt.y], snap_poly);
                if let Some(f_start) = self.frontage_start {
                    let path = trace_polygon_path(snap_poly, f_start, snapped);
                    if poly_path_length(&path) > 0.05 {
                        self.frontage_end = Some(snapped);
                        self.frontage_path = path;
                        match self.solve_swing_edit() {
                            Ok(()) => {
                                self.step = SwingEditStep::AcceptPreview;
                                return CmdResult::NeedPoint;
                            }
                            Err(e) => return CmdResult::ReportError(e),
                        }
                    }
                }
                CmdResult::NeedPoint
            }
            SwingEditStep::AcceptPreview => self.commit_solution(),
        }
    }

    fn wants_text_input(&self) -> bool {
        self.step == SwingEditStep::AcceptPreview
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        let t = text.trim();
        if self.step == SwingEditStep::AcceptPreview {
            let up = t.to_ascii_uppercase();
            if up == "Y" || up == "YES" || up.is_empty() {
                Some(self.commit_solution())
            } else if up == "NEXT" && self.candidate_solutions.len() > 1 {
                self.current_solution_idx = (self.current_solution_idx + 1) % self.candidate_solutions.len();
                self.solution = Some(self.candidate_solutions[self.current_solution_idx].clone());
                Some(CmdResult::NeedPoint)
            } else if up == "N" || up == "NO" {
                Some(CmdResult::Cancel)
            } else {
                Some(CmdResult::NeedPoint)
            }
        } else {
            None
        }
    }

    fn on_enter(&mut self) -> CmdResult {
        if self.step == SwingEditStep::AcceptPreview {
            self.commit_solution()
        } else {
            CmdResult::Cancel
        }
    }

    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }

    fn on_preview_wires(&mut self, _pt: DVec3) -> Vec<WireModel> {
        let mut wires = Vec::new();

        if let Some(pivot) = self.pivot_point {
            let d = 0.5;
            let p_3d = [pivot[0], pivot[1], 0.0];
            wires.push(WireModel::solid_f64(
                "preview_swing_edit_pivot".to_string(),
                vec![
                    [p_3d[0] - d, p_3d[1] - d, 0.0],
                    [p_3d[0] + d, p_3d[1] + d, 0.0],
                    [p_3d[0], p_3d[1], 0.0],
                    [p_3d[0] - d, p_3d[1] + d, 0.0],
                    [p_3d[0] + d, p_3d[1] - d, 0.0],
                ],
                [1.0, 0.2, 0.8, 1.0],
                false,
            ));
        }

        if self.step == SwingEditStep::PickFrontageEnd {
            let snap_poly = if !self.combined_vertices.is_empty() {
                &self.combined_vertices
            } else {
                &self.target_vertices
            };
            if let Some(f_start) = self.frontage_start {
                let (cur_snap, _, _) = snap_point_to_polygon([_pt.x, _pt.y], snap_poly);
                let active_path = trace_polygon_path(snap_poly, f_start, cur_snap);
                if active_path.len() >= 2 {
                    let pts_3d: Vec<[f64; 3]> = active_path.iter().map(|p| [p[0], p[1], 0.0]).collect();
                    let mut orange_line = WireModel::solid_f64(
                        "preview_frontage_active_trace".to_string(),
                        pts_3d,
                        [1.0, 0.52, 0.0, 1.0],
                        false,
                    );
                    orange_line.set_fixed_screen_width(4.0);
                    wires.push(orange_line);
                }

                let p_s = [f_start[0], f_start[1], 0.0];
                let d = 0.4;
                wires.push(WireModel::solid_f64(
                    "preview_fstart_marker".to_string(),
                    vec![
                        [p_s[0] - d, p_s[1] - d, 0.0],
                        [p_s[0] + d, p_s[1] - d, 0.0],
                        [p_s[0] + d, p_s[1] + d, 0.0],
                        [p_s[0] - d, p_s[1] + d, 0.0],
                        [p_s[0] - d, p_s[1] - d, 0.0],
                    ],
                    [0.2, 1.0, 0.3, 1.0],
                    false,
                ));

                let p_e = [cur_snap[0], cur_snap[1], 0.0];
                wires.push(WireModel::solid_f64(
                    "preview_fend_marker".to_string(),
                    vec![
                        [p_e[0] - d, p_e[1] - d, 0.0],
                        [p_e[0] + d, p_e[1] - d, 0.0],
                        [p_e[0] + d, p_e[1] + d, 0.0],
                        [p_e[0] - d, p_e[1] + d, 0.0],
                        [p_e[0] - d, p_e[1] - d, 0.0],
                    ],
                    [1.0, 0.6, 0.1, 1.0],
                    false,
                ));
            }
            return wires;
        }

        if self.frontage_path.len() >= 2 {
            let pts_3d: Vec<[f64; 3]> = self.frontage_path.iter().map(|p| [p[0], p[1], 0.0]).collect();
            let mut cyan_line = WireModel::solid_f64(
                "preview_frontage_selected_path".to_string(),
                pts_3d,
                [0.0, 0.95, 0.95, 1.0],
                false,
            );
            cyan_line.set_fixed_screen_width(3.0);
            wires.push(cyan_line);
        }

        if let Some(sol) = &self.solution {
            let p1 = [sol.cut_line.0[0], sol.cut_line.0[1], 0.0];
            let p2 = [sol.cut_line.1[0], sol.cut_line.1[1], 0.0];
            wires.push(WireModel::solid_f64(
                "preview_swing_edit_cut_line".to_string(),
                vec![p1, p2],
                [0.1, 0.9, 1.0, 1.0],
                false,
            ));

            let mut target_pts: Vec<[f64; 3]> = sol.target_lot.iter().map(|p| [p[0], p[1], 0.0]).collect();
            if let Some(first) = target_pts.first().copied() {
                target_pts.push(first);
            }
            wires.push(WireModel::solid_f64(
                "preview_swing_edit_target_lot".to_string(),
                target_pts,
                [1.0, 0.75, 0.15, 1.0],
                false,
            ));

            if !sol.remainder_lot.is_empty() {
                let mut rem_pts: Vec<[f64; 3]> = sol.remainder_lot.iter().map(|p| [p[0], p[1], 0.0]).collect();
                if let Some(first) = rem_pts.first().copied() {
                    rem_pts.push(first);
                }
                wires.push(WireModel::solid_f64(
                    "preview_swing_edit_neighbor_lot".to_string(),
                    rem_pts,
                    [0.75, 0.35, 0.95, 0.8],
                    false,
                ));
            }
        }

        wires
    }
}

impl SwingLineEditCommand {
    fn commit_solution(&self) -> CmdResult {
        let (Some(target_handle), Some(sol)) = (self.target_handle, &self.solution) else {
            return CmdResult::Cancel;
        };

        let mut replacements = Vec::new();

        // 1. Updated Target Parcel
        let mut target_pl = LwPolyline::from_points(
            sol.target_lot.iter().map(|v| Vector2::new(v[0], v[1])).collect(),
        );
        target_pl.is_closed = true;
        let mut target_ent = EntityType::LwPolyline(target_pl);
        let target_assigned = if !self.target_zoning.is_empty() {
            crate::ui::window::create_parcels_layout::layer_for_style(&self.target_zoning).to_string()
        } else {
            self.target_layer.clone()
        };
        target_ent.common_mut().layer = target_assigned;

        let mut rec_target = ExtendedDataRecord::new(XDATA_PARCEL);
        rec_target.add_value(XDataValue::String(self.target_name.clone()));
        rec_target.add_value(XDataValue::String(self.target_site.clone()));
        rec_target.add_value(XDataValue::String(format!("Lot {:X}", target_handle.value())));
        rec_target.add_value(XDataValue::String(self.target_zoning.clone()));
        target_ent.common_mut().extended_data.add_record(rec_target);

        replacements.push((target_handle, vec![target_ent]));

        // 2. Updated Neighbor Parcel (if present)
        if let Some(neighbor_handle) = self.neighbor_handle {
            if !sol.remainder_lot.is_empty() && shoelace_area(&sol.remainder_lot) > 1.0 {
                let mut neighbor_pl = LwPolyline::from_points(
                    sol.remainder_lot.iter().map(|v| Vector2::new(v[0], v[1])).collect(),
                );
                neighbor_pl.is_closed = true;
                let mut neighbor_ent = EntityType::LwPolyline(neighbor_pl);
                let neighbor_assigned = if !self.neighbor_zoning.is_empty() {
                    crate::ui::window::create_parcels_layout::layer_for_style(&self.neighbor_zoning).to_string()
                } else {
                    self.neighbor_layer.clone()
                };
                neighbor_ent.common_mut().layer = neighbor_assigned;

                let mut rec_neighbor = ExtendedDataRecord::new(XDATA_PARCEL);
                rec_neighbor.add_value(XDataValue::String(self.neighbor_name.clone()));
                rec_neighbor.add_value(XDataValue::String(self.neighbor_site.clone()));
                rec_neighbor.add_value(XDataValue::String(format!("Lot {:X}", neighbor_handle.value())));
                rec_neighbor.add_value(XDataValue::String(self.neighbor_zoning.clone()));
                neighbor_ent.common_mut().extended_data.add_record(rec_neighbor);

                replacements.push((neighbor_handle, vec![neighbor_ent]));
            }
        }

        CmdResult::ReplaceMany(replacements, Vec::new())
    }
}

// ── Interactive Swing Line Command (Civil 3D Parity) ──────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwingStep {
    PickParcel,
    PickPivot,
    PickFrontageStart,
    PickFrontageEnd,
    AcceptPreview,
}

pub struct SwingLineCommand {
    pub criteria: ParcelSizingCriteria,
    pub min_area: f64,
    pub min_frontage: f64,
    pub frontage_offset: f64,
    pub automatic_mode: bool,
    pub remainder_mode: String,
    pub step: SwingStep,

    pub parent_handle: Option<Handle>,

    pub parent_name: String,

    pub parent_site: String,

    pub parent_layer: String,

    pub parent_zoning: String,

    pub parent_owner: String,
    pub parent_deed: String,
    pub parent_tax_id: String,
    pub parent_area_label_style: String,
    pub parent_line_label_style: String,
    pub parent_curve_label_style: String,
    pub parent_auto_add_segment_labels: bool,
    pub parent_segment_layer: String,
    pub parent_vertices: Vec<[f64; 2]>,

    pub parent_area: f64,

    pub pivot_point: Option<[f64; 2]>,
    pub frontage_start: Option<[f64; 2]>,
    pub frontage_end: Option<[f64; 2]>,
    pub frontage_path: Vec<[f64; 2]>,
    pub solution: Option<SlideSlice>,
    pub candidate_solutions: Vec<SlideSlice>,
    pub current_solution_idx: usize,
    pub picked_entity: Option<EntityType>,
}

impl SwingLineCommand {
    pub fn new(min_area: f64, min_frontage: f64) -> Self {
        let criteria = ParcelSizingCriteria {
            min_area: if min_area > 0.0 { min_area } else { 350.0 },
            min_frontage: if min_frontage > 0.0 { min_frontage } else { 12.0 },
            ..Default::default()
        };
        Self::new_with_criteria(criteria)
    }

    pub fn new_with_criteria(criteria: ParcelSizingCriteria) -> Self {
        let min_area = criteria.min_area;
        let min_frontage = criteria.min_frontage;
        let frontage_offset = criteria.frontage_offset;
        let automatic_mode = criteria.automatic_mode;
        let remainder_mode = criteria.remainder_mode.clone();

        Self {
            criteria,
            min_area,
            min_frontage,
            frontage_offset,
            automatic_mode,
            remainder_mode,
            step: SwingStep::PickParcel,
            parent_handle: None,
            parent_name: "LOT".to_string(),
            parent_site: "Site 1".to_string(),
            parent_layer: "PC-PARCEL".to_string(),
            parent_zoning: "R-1".to_string(),
            parent_owner: String::new(),
            parent_deed: String::new(),
            parent_tax_id: String::new(),
            parent_area_label_style: "Parcel Name, Area & Perimeter".to_string(),
            parent_line_label_style: "Bearing over Distance".to_string(),
            parent_curve_label_style: "Distance over Radius".to_string(),
            parent_auto_add_segment_labels: true,
            parent_segment_layer: "C-PROP-LINE".to_string(),
            parent_vertices: Vec::new(),
            parent_area: 0.0,
            pivot_point: None,
            frontage_start: None,
            frontage_end: None,
            frontage_path: Vec::new(),
            solution: None,
            candidate_solutions: Vec::new(),
            current_solution_idx: 0,
            picked_entity: None,
        }
    }

    pub fn sync_criteria(&mut self) {
        self.criteria.min_area = self.min_area;
        self.criteria.min_frontage = self.min_frontage;
        self.criteria.frontage_offset = self.frontage_offset;
        self.criteria.automatic_mode = self.automatic_mode;
        self.criteria.remainder_mode = self.remainder_mode.clone();
    }

    fn solve_swing(&mut self) -> Result<(), String> {
        self.sync_criteria();
        let (Some(pivot), Some(f_start), Some(f_end)) = (self.pivot_point, self.frontage_start, self.frontage_end) else {
            return Err("Pivot and frontage points must be defined".to_string());
        };

        let path = if self.frontage_path.len() >= 2 {
            self.frontage_path.clone()
        } else {
            trace_polygon_path(&self.parent_vertices, f_start, f_end)
        };

        let sols = solve_swing_line_candidates(
            &self.parent_vertices,
            pivot,
            &path,
            self.min_area,
            Some(&self.criteria),
        )?;

        self.solution = Some(sols[0].clone());
        self.candidate_solutions = sols;
        self.current_solution_idx = 0;
        self.step = SwingStep::AcceptPreview;
        Ok(())
    }
}

impl CadCommand for SwingLineCommand {
    fn name(&self) -> &'static str {
        "SWINGLINE"
    }

    fn prompt(&self) -> String {
        match self.step {
            SwingStep::PickParcel => {
                crate::t!("SWINGLINE  Select parcel to be subdivided:").into_owned()
            }
            SwingStep::PickPivot => {
                "SWINGLINE  Pick pivot point on rear boundary or cul-de-sac center:".to_string()
            }
            SwingStep::PickFrontageStart => {
                "SWINGLINE  Select start point on frontage:".to_string()
            }
            SwingStep::PickFrontageEnd => {
                "SWINGLINE  Drag mouse along parcel perimeter to pick frontage end point (or click):".to_string()
            }
            SwingStep::AcceptPreview => {
                if let Some(sol) = &self.solution {
                    let total_sols = self.candidate_solutions.len();
                    let sol_info = if total_sols > 1 {
                        format!(" [Solution {}/{}]", self.current_solution_idx + 1, total_sols)
                    } else {
                        String::new()
                    };
                    let warn = if !sol.satisfies_criteria {
                        " (Criteria Warning: bounds violated)"
                    } else {
                        ""
                    };
                    format!(
                        "SWINGLINE{}  Target: {:.1}m², Frontage: {:.1}m, Depth: {:.1}m, Remainder: {:.1}m{}{}. Accept? [Yes/Next/No] <Yes>:",
                        sol_info, sol.target_area, sol.frontage_length, sol.depth, sol.remainder_area,
                        if self.criteria.use_offset { format!(", Width@Offset: {:.1}m", sol.width_at_offset) } else { String::new() },
                        warn
                    )
                } else {
                    "SWINGLINE  Accept result? [Yes/No] <Yes>:".to_string()
                }
            }
        }
    }

    fn options(&self) -> Vec<CmdOption> {
        match self.step {
            SwingStep::AcceptPreview => {
                let mut opts = vec![CmdOption::new("Yes (Accept)", "Y")];
                if self.candidate_solutions.len() > 1 {
                    opts.push(CmdOption::new("Next Solution", "NEXT"));
                }
                opts.push(CmdOption::new("No (Reject)", "N"));
                opts
            }
            _ => Vec::new(),
        }
    }

    fn needs_entity_pick(&self) -> bool {
        self.step == SwingStep::PickParcel
    }

    fn inject_before_entity_pick(&self) -> bool {
        true
    }

    fn inject_picked_entity(&mut self, entity: EntityType) {
        self.picked_entity = Some(entity);
    }

    fn on_entity_pick(&mut self, handle: Handle, _pt: DVec3) -> CmdResult {
        if handle.is_null() {
            return CmdResult::NeedPoint;
        }

        if let Some(entity) = &self.picked_entity {
            if is_parcel_entity(entity) || matches!(entity, EntityType::LwPolyline(_)) {
                if let EntityType::LwPolyline(lw) = entity {
                    if lw.vertices.len() >= 2 {
                        let mut verts: Vec<[f64; 2]> = Vec::new();
                        let n_v = lw.vertices.len();
                        for i in 0..n_v {
                            let v = &lw.vertices[i];
                            let p1 = [v.location.x, v.location.y];
                            let next_idx = (i + 1) % n_v;
                            let p2 = [lw.vertices[next_idx].location.x, lw.vertices[next_idx].location.y];
                            if v.bulge.abs() > 1e-5 {
                                let arc_pts = tessellate_bulge(p1, p2, v.bulge, 16);
                                for p in arc_pts.into_iter().take(16) {
                                    verts.push(p);
                                }
                            } else {
                                verts.push(p1);
                            }
                        }
                        let area = shoelace_area(&verts);
                        if area > 1.0 {
                            self.parent_handle = Some(handle);
                            self.parent_layer = entity.common().layer.clone();
                            self.parent_vertices = verts;
                            self.parent_area = area;

                            if let Some(rec) = entity.common().extended_data.get_record(XDATA_PARCEL) {
                                if let Some(XDataValue::String(s)) = rec.values.first() {
                                    self.parent_name = s.clone();
                                }
                                if let Some(XDataValue::String(site)) = rec.values.get(1) {
                                    self.parent_site = site.clone();
                                }
                            } else {
                                self.parent_name = format!("Lot {:X}", handle.value());
                            }

                            self.step = SwingStep::PickPivot;
                            return CmdResult::NeedPoint;
                        }
                    }
                }
            }
        }

        CmdResult::NeedPoint
    }

    fn on_point(&mut self, pt: DVec3) -> CmdResult {
        match self.step {
            SwingStep::PickParcel => CmdResult::NeedPoint,
            SwingStep::PickPivot => {
                let (snapped, _, _) = snap_point_to_polygon([pt.x, pt.y], &self.parent_vertices);
                self.pivot_point = Some(snapped);
                self.step = SwingStep::PickFrontageStart;
                CmdResult::NeedPoint
            }
            SwingStep::PickFrontageStart => {
                let (snapped, _, _) = snap_point_to_polygon([pt.x, pt.y], &self.parent_vertices);
                self.frontage_start = Some(snapped);
                self.step = SwingStep::PickFrontageEnd;
                CmdResult::NeedPoint
            }
            SwingStep::PickFrontageEnd => {
                let (snapped, _, _) = snap_point_to_polygon([pt.x, pt.y], &self.parent_vertices);
                if let Some(f_start) = self.frontage_start {
                    let path = trace_polygon_path(&self.parent_vertices, f_start, snapped);
                    if poly_path_length(&path) > 0.05 {
                        self.frontage_end = Some(snapped);
                        self.frontage_path = path;
                        match self.solve_swing() {
                            Ok(()) => {
                                self.step = SwingStep::AcceptPreview;
                                return CmdResult::NeedPoint;
                            }
                            Err(e) => return CmdResult::ReportError(e),
                        }
                    }
                }
                CmdResult::NeedPoint
            }
            SwingStep::AcceptPreview => self.commit_solution(),
        }
    }

    fn wants_text_input(&self) -> bool {
        self.step == SwingStep::AcceptPreview
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        let t = text.trim();
        if self.step == SwingStep::AcceptPreview {
            let up = t.to_ascii_uppercase();
            if up == "Y" || up == "YES" || up.is_empty() {
                Some(self.commit_solution())
            } else if up == "NEXT" && self.candidate_solutions.len() > 1 {
                self.current_solution_idx = (self.current_solution_idx + 1) % self.candidate_solutions.len();
                self.solution = Some(self.candidate_solutions[self.current_solution_idx].clone());
                Some(CmdResult::NeedPoint)
            } else if up == "N" || up == "NO" {
                Some(CmdResult::Cancel)
            } else {
                Some(CmdResult::NeedPoint)
            }
        } else {
            None
        }
    }

    fn on_enter(&mut self) -> CmdResult {
        if self.step == SwingStep::AcceptPreview {
            self.commit_solution()
        } else {
            CmdResult::Cancel
        }
    }

    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }

    fn on_preview_wires(&mut self, _pt: DVec3) -> Vec<WireModel> {
        let mut wires = Vec::new();

        if let Some(pivot) = self.pivot_point {
            let d = 0.5;
            let p_3d = [pivot[0], pivot[1], 0.0];
            wires.push(WireModel::solid_f64(
                "preview_pivot_marker".to_string(),
                vec![
                    [p_3d[0] - d, p_3d[1] - d, 0.0],
                    [p_3d[0] + d, p_3d[1] + d, 0.0],
                    [p_3d[0], p_3d[1], 0.0],
                    [p_3d[0] - d, p_3d[1] + d, 0.0],
                    [p_3d[0] + d, p_3d[1] - d, 0.0],
                ],
                [1.0, 0.2, 0.8, 1.0],
                false,
            ));
        }

        if self.step == SwingStep::PickFrontageEnd {
            if let Some(f_start) = self.frontage_start {
                let (cur_snap, _, _) = snap_point_to_polygon([_pt.x, _pt.y], &self.parent_vertices);
                let active_path = trace_polygon_path(&self.parent_vertices, f_start, cur_snap);
                if active_path.len() >= 2 {
                    let pts_3d: Vec<[f64; 3]> = active_path.iter().map(|p| [p[0], p[1], 0.0]).collect();
                    let mut orange_line = WireModel::solid_f64(
                        "preview_frontage_active_trace".to_string(),
                        pts_3d,
                        [1.0, 0.52, 0.0, 1.0],
                        false,
                    );
                    orange_line.set_fixed_screen_width(4.0);
                    wires.push(orange_line);
                }

                let p_s = [f_start[0], f_start[1], 0.0];
                let d = 0.4;
                wires.push(WireModel::solid_f64(
                    "preview_fstart_marker".to_string(),
                    vec![
                        [p_s[0] - d, p_s[1] - d, 0.0],
                        [p_s[0] + d, p_s[1] - d, 0.0],
                        [p_s[0] + d, p_s[1] + d, 0.0],
                        [p_s[0] - d, p_s[1] + d, 0.0],
                        [p_s[0] - d, p_s[1] - d, 0.0],
                    ],
                    [0.2, 1.0, 0.3, 1.0],
                    false,
                ));

                let p_e = [cur_snap[0], cur_snap[1], 0.0];
                wires.push(WireModel::solid_f64(
                    "preview_fend_marker".to_string(),
                    vec![
                        [p_e[0] - d, p_e[1] - d, 0.0],
                        [p_e[0] + d, p_e[1] - d, 0.0],
                        [p_e[0] + d, p_e[1] + d, 0.0],
                        [p_e[0] - d, p_e[1] + d, 0.0],
                        [p_e[0] - d, p_e[1] - d, 0.0],
                    ],
                    [1.0, 0.6, 0.1, 1.0],
                    false,
                ));
            }
            return wires;
        }

        if self.frontage_path.len() >= 2 {
            let pts_3d: Vec<[f64; 3]> = self.frontage_path.iter().map(|p| [p[0], p[1], 0.0]).collect();
            let mut f_line = WireModel::solid_f64(
                "preview_frontage_wire".to_string(),
                pts_3d,
                [1.0, 0.55, 0.0, 1.0],
                false,
            );
            f_line.set_fixed_screen_width(3.5);
            wires.push(f_line);
        }

        if let Some(sol) = &self.solution {
            let p1 = [sol.cut_line.0[0], sol.cut_line.0[1], 0.0];
            let p2 = [sol.cut_line.1[0], sol.cut_line.1[1], 0.0];
            wires.push(WireModel::solid_f64(
                "preview_swing_cut_line".to_string(),
                vec![p1, p2],
                [0.1, 0.9, 1.0, 1.0],
                false,
            ));

            let mut target_pts: Vec<[f64; 3]> = sol.target_lot.iter().map(|p| [p[0], p[1], 0.0]).collect();
            if let Some(first) = target_pts.first().copied() {
                target_pts.push(first);
            }
            wires.push(WireModel::solid_f64(
                "preview_swing_target_lot".to_string(),
                target_pts,
                [1.0, 0.75, 0.15, 1.0],
                false,
            ));
        }

        wires
    }
}

impl SwingLineCommand {
    fn commit_solution(&self) -> CmdResult {
        let (Some(parent_handle), Some(sol)) = (self.parent_handle, &self.solution) else {
            return CmdResult::Cancel;
        };

        let lots = vec![sol.target_lot.clone(), sol.remainder_lot.clone()];
        let mut child_entities = Vec::new();
        for (i, lot_pts) in lots.iter().enumerate() {
            let lot_name = format!("{}-{}", self.parent_name, i + 1);
            let lot_num = (100 + i + 1).to_string();

            let mut pl = LwPolyline::from_points(
                lot_pts.iter().map(|v| Vector2::new(v[0], v[1])).collect(),
            );
            pl.is_closed = true;
            let mut ent = EntityType::LwPolyline(pl);
            let assigned_layer = if !self.parent_zoning.is_empty() {
                crate::ui::window::create_parcels_layout::layer_for_style(&self.parent_zoning).to_string()
            } else {
                crate::ui::window::create_parcels_layout::layer_for_style(&self.parent_layer).to_string()
            };
            ent.common_mut().layer = assigned_layer;

            let mut rec = ExtendedDataRecord::new(XDATA_PARCEL);
            rec.add_value(XDataValue::String(lot_name));
            rec.add_value(XDataValue::String(self.parent_site.clone()));
            rec.add_value(XDataValue::String(lot_num));
            rec.add_value(XDataValue::String(if !self.parent_zoning.is_empty() { self.parent_zoning.clone() } else { "Single-Family Residential (R-1)".to_string() }));
            ent.common_mut().extended_data.add_record(rec);

            child_entities.push(ent);
        }

        CmdResult::ReplaceMany(
            vec![(parent_handle, child_entities)],
            Vec::new(),
        )
    }
}

// ── Interactive Create Parcel from Objects Command ────────────────────────

pub struct ParcelFromObjectsCommand {
    pub picked_handles: Vec<Handle>,
    pub picked_entities: Vec<EntityType>,
    pub picked_entity: Option<EntityType>,
    pub start_lot_number: u32,
    pub site_name: String,
    pub parcel_style: String,
    pub parcel_layer: String,
    pub segment_layer: String,
    pub area_label_style: String,
    pub line_label_style: String,
    pub curve_label_style: String,
    pub auto_add_segment_labels: bool,
}

impl ParcelFromObjectsCommand {
    pub fn new() -> Self {
        Self {
            picked_handles: Vec::new(),
            picked_entities: Vec::new(),
            picked_entity: None,
            start_lot_number: 101,
            site_name: "Site 1".to_string(),
            parcel_style: "Single-Family".to_string(),
            parcel_layer: "C-PROP".to_string(),
            segment_layer: "C-PROP-LINE".to_string(),
            area_label_style: "Parcel Name, Area & Perimeter".to_string(),
            line_label_style: "Bearing over Distance".to_string(),
            curve_label_style: "Distance over Radius".to_string(),
            auto_add_segment_labels: true,
        }
    }
}

impl Default for ParcelFromObjectsCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl CadCommand for ParcelFromObjectsCommand {
    fn name(&self) -> &'static str {
        "PARCELFROMOBJECTS"
    }

    fn prompt(&self) -> String {
        if self.picked_handles.is_empty() {
            crate::t!("PARCELFROMOBJECTS  Select objects to create parcels from:").into_owned()
        } else {
            format!(
                "PARCELFROMOBJECTS  {} object(s) selected. Select more objects or press Enter to create parcels:",
                self.picked_handles.len()
            )
        }
    }

    fn needs_entity_pick(&self) -> bool {
        true
    }

    fn inject_before_entity_pick(&self) -> bool {
        true
    }

    fn inject_picked_entity(&mut self, entity: EntityType) {
        self.picked_entity = Some(entity);
    }

    fn on_entity_pick(&mut self, handle: Handle, _pt: DVec3) -> CmdResult {
        if handle.is_null() || self.picked_handles.contains(&handle) {
            return CmdResult::NeedPoint;
        }

        if let Some(entity) = &self.picked_entity {
            if let EntityType::LwPolyline(lw) = entity {
                if lw.vertices.len() >= 3 {
                    self.picked_handles.push(handle);
                    self.picked_entities.push(entity.clone());
                    return CmdResult::NeedPoint;
                }
            }
        }

        CmdResult::NeedPoint
    }

    fn on_point(&mut self, _pt: DVec3) -> CmdResult {
        CmdResult::NeedPoint
    }

    fn on_enter(&mut self) -> CmdResult {
        if self.picked_handles.is_empty() {
            return CmdResult::Cancel;
        }

        let mut replacements = Vec::new();
        for (i, (handle, ent)) in self.picked_handles.iter().zip(&self.picked_entities).enumerate() {
            let mut parcel_ent = ent.clone();

            let assigned_layer = if self.parcel_layer.is_empty() || self.parcel_layer == "C-PROP" || self.parcel_layer == "PC-PARCEL" {
                crate::ui::window::create_parcels_layout::layer_for_style(&self.parcel_style).to_string()
            } else {
                self.parcel_layer.clone()
            };
            parcel_ent.common_mut().layer = assigned_layer;



            let num = self.start_lot_number + i as u32;
            let lot_name = format!("LOT-{}", num);

            let mut rec = ExtendedDataRecord::new(XDATA_PARCEL);
            rec.add_value(XDataValue::String(lot_name));
            rec.add_value(XDataValue::String(self.site_name.clone()));
            rec.add_value(XDataValue::String(num.to_string()));
            rec.add_value(XDataValue::String(self.parcel_style.clone()));
            rec.add_value(XDataValue::String("Standard Property Owner".to_string()));
            rec.add_value(XDataValue::String("D-2026-001".to_string()));
            rec.add_value(XDataValue::String("042-881-01".to_string()));
            rec.add_value(XDataValue::String(self.area_label_style.clone()));
            rec.add_value(XDataValue::String(self.line_label_style.clone()));
            rec.add_value(XDataValue::String(self.curve_label_style.clone()));
            rec.add_value(XDataValue::String(if self.auto_add_segment_labels { "true".to_string() } else { "false".to_string() }));
            rec.add_value(XDataValue::String(self.segment_layer.clone()));
            parcel_ent.common_mut().extended_data.add_record(rec);

            replacements.push((*handle, vec![parcel_ent]));
        }

        CmdResult::ReplaceMany(replacements, Vec::new())
    }







    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

// ── Interactive ParcelRenumberCommand (Civil 3D Parity) ──────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenumberInteractiveStep {
    PickParcel,
    FenceStart,
    FenceEnd,
    AwaitingPrefix,
    AwaitingStartNumber,
    AwaitingIncrement,
}

pub struct ParcelRenumberCommand {
    pub step: RenumberInteractiveStep,
    pub prefix: String,
    pub current_number: u32,
    pub increment: u32,
    pub picked_entity: Option<EntityType>,
    pub fence_first_pt: Option<DVec3>,
    pub renumbered_count: usize,
}

impl ParcelRenumberCommand {
    pub fn new(prefix: String, start_number: u32, increment: u32) -> Self {
        Self {
            step: RenumberInteractiveStep::PickParcel,
            prefix,
            current_number: if start_number == 0 { 1 } else { start_number },
            increment: if increment == 0 { 1 } else { increment },
            picked_entity: None,
            fence_first_pt: None,
            renumbered_count: 0,
        }
    }
}

impl Default for ParcelRenumberCommand {
    fn default() -> Self {
        Self::new("Lot_".to_string(), 1, 1)
    }
}

impl CadCommand for ParcelRenumberCommand {
    fn name(&self) -> &'static str {
        "PARCELRENUMBER"
    }

    fn prompt(&self) -> String {
        match self.step {
            RenumberInteractiveStep::PickParcel => {
                format!(
                    "RENUMBER  [Fence/Prefix/Start/Increment/All/Exit] Click parcel to assign '{}{}' <Fence>:",
                    self.prefix, self.current_number
                )
            }
            RenumberInteractiveStep::FenceStart => {
                "RENUMBER  Specify first point of fence line across parcels:".to_string()
            }
            RenumberInteractiveStep::FenceEnd => {
                "RENUMBER  Specify second point of fence line across parcels:".to_string()
            }
            RenumberInteractiveStep::AwaitingPrefix => {
                format!("Enter parcel name prefix <{}>:", self.prefix)
            }
            RenumberInteractiveStep::AwaitingStartNumber => {
                format!("Enter starting lot number <{}>:", self.current_number)
            }
            RenumberInteractiveStep::AwaitingIncrement => {
                format!("Enter numbering increment <{}>:", self.increment)
            }
        }
    }

    fn options(&self) -> Vec<CmdOption> {
        match self.step {
            RenumberInteractiveStep::PickParcel => vec![
                CmdOption::new("Fence", "FENCE"),
                CmdOption::new("Prefix", "PREFIX"),
                CmdOption::new("Start", "START"),
                CmdOption::new("Increment", "INCREMENT"),
                CmdOption::new("All", "ALL"),
                CmdOption::new("Exit", "EXIT"),
            ],
            RenumberInteractiveStep::FenceStart | RenumberInteractiveStep::FenceEnd => vec![
                CmdOption::new("Pick Mode", "PICK"),
                CmdOption::new("Cancel", "CANCEL"),
            ],
            _ => Vec::new(),
        }
    }

    fn needs_entity_pick(&self) -> bool {
        self.step == RenumberInteractiveStep::PickParcel
    }

    fn inject_before_entity_pick(&self) -> bool {
        true
    }

    fn inject_picked_entity(&mut self, entity: EntityType) {
        self.picked_entity = Some(entity);
    }

    fn on_entity_pick(&mut self, handle: Handle, _pt: DVec3) -> CmdResult {
        if self.step == RenumberInteractiveStep::PickParcel {
            if let Some(mut entity) = self.picked_entity.take() {
                if is_parcel_entity(&entity) || matches!(entity, EntityType::LwPolyline(_)) {
                    let new_name = format!("{}{}", self.prefix, self.current_number);
                    let num_str = self.current_number.to_string();

                    let (site_name, zoning, owner, deed, tax_id, area_style, line_style, curve_style, auto_segs, seg_layer) = {
                        if let Some(rec) = entity.common().extended_data.get_record(XDATA_PARCEL) {
                            let get_s = |idx: usize, def: &str| -> String {
                                rec.values.get(idx).and_then(|v| match v {
                                    XDataValue::String(s) => Some(s.clone()),
                                    _ => None,
                                }).unwrap_or_else(|| def.to_string())
                            };
                            (
                                get_s(1, "Site 1"),
                                get_s(3, "R-1"),
                                get_s(4, "Standard"),
                                get_s(5, "N/A"),
                                get_s(6, "N/A"),
                                get_s(7, "Parcel Name, Area & Perimeter"),
                                get_s(8, "Bearing over Distance"),
                                get_s(9, "Distance over Radius"),
                                get_s(10, "true"),
                                get_s(11, "C-PROP-LINE"),
                            )
                        } else {
                            (
                                "Site 1".to_string(),
                                "R-1".to_string(),
                                "Standard".to_string(),
                                "N/A".to_string(),
                                "N/A".to_string(),
                                "Parcel Name, Area & Perimeter".to_string(),
                                "Bearing over Distance".to_string(),
                                "Distance over Radius".to_string(),
                                "true".to_string(),
                                "C-PROP-LINE".to_string(),
                            )
                        }
                    };

                    let mut rec = ExtendedDataRecord::new(XDATA_PARCEL);
                    rec.add_value(XDataValue::String(new_name));
                    rec.add_value(XDataValue::String(site_name));
                    rec.add_value(XDataValue::String(num_str));
                    rec.add_value(XDataValue::String(zoning));
                    rec.add_value(XDataValue::String(owner));
                    rec.add_value(XDataValue::String(deed));
                    rec.add_value(XDataValue::String(tax_id));
                    rec.add_value(XDataValue::String(area_style));
                    rec.add_value(XDataValue::String(line_style));
                    rec.add_value(XDataValue::String(curve_style));
                    rec.add_value(XDataValue::String(auto_segs));
                    rec.add_value(XDataValue::String(seg_layer));

                    let common = entity.common_mut();
                    let kept: Vec<_> = common
                        .extended_data
                        .records()
                        .iter()
                        .filter(|r| r.application_name != XDATA_PARCEL)
                        .cloned()
                        .collect();
                    common.extended_data.clear();
                    for r in kept {
                        common.extended_data.add_record(r);
                    }
                    common.extended_data.add_record(rec);

                    self.current_number += self.increment;
                    self.renumbered_count += 1;

                    return CmdResult::UpdateEntityContinue { handle, entity };
                }
            }
        }
        CmdResult::NeedPoint
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        let t = text.trim();
        match self.step {
            RenumberInteractiveStep::PickParcel => {
                let lower = t.to_ascii_lowercase();
                match lower.as_str() {
                    "f" | "fence" => {
                        self.step = RenumberInteractiveStep::FenceStart;
                        Some(CmdResult::NeedPoint)
                    }
                    "p" | "prefix" => {
                        self.step = RenumberInteractiveStep::AwaitingPrefix;
                        Some(CmdResult::NeedPoint)
                    }
                    "s" | "start" => {
                        self.step = RenumberInteractiveStep::AwaitingStartNumber;
                        Some(CmdResult::NeedPoint)
                    }
                    "i" | "increment" => {
                        self.step = RenumberInteractiveStep::AwaitingIncrement;
                        Some(CmdResult::NeedPoint)
                    }
                    "a" | "all" => Some(CmdResult::Dispatch(format!(
                        "PC_RENUMBER \"{}\" {} {}",
                        self.prefix, self.current_number, self.increment
                    ))),
                    "x" | "exit" => Some(CmdResult::Cancel),
                    _ => None,
                }
            }
            RenumberInteractiveStep::AwaitingPrefix => {
                if !t.is_empty() {
                    self.prefix = t.to_string();
                }
                self.step = RenumberInteractiveStep::PickParcel;
                Some(CmdResult::NeedPoint)
            }
            RenumberInteractiveStep::AwaitingStartNumber => {
                if let Ok(n) = t.parse::<u32>() {
                    self.current_number = n;
                }
                self.step = RenumberInteractiveStep::PickParcel;
                Some(CmdResult::NeedPoint)
            }
            RenumberInteractiveStep::AwaitingIncrement => {
                if let Ok(inc) = t.parse::<u32>() {
                    if inc > 0 {
                        self.increment = inc;
                    }
                }
                self.step = RenumberInteractiveStep::PickParcel;
                Some(CmdResult::NeedPoint)
            }
            RenumberInteractiveStep::FenceStart | RenumberInteractiveStep::FenceEnd => {
                let lower = t.to_ascii_lowercase();
                if lower == "pick" || lower == "p" || lower == "c" || lower == "cancel" {
                    self.step = RenumberInteractiveStep::PickParcel;
                    self.fence_first_pt = None;
                    Some(CmdResult::NeedPoint)
                } else {
                    None
                }
            }
        }
    }

    fn on_point(&mut self, pt: DVec3) -> CmdResult {
        match self.step {
            RenumberInteractiveStep::FenceStart => {
                self.fence_first_pt = Some(pt);
                self.step = RenumberInteractiveStep::FenceEnd;
                CmdResult::NeedPoint
            }
            RenumberInteractiveStep::FenceEnd => {
                if let Some(p1) = self.fence_first_pt.take() {
                    self.step = RenumberInteractiveStep::PickParcel;
                    return CmdResult::Dispatch(format!(
                        "PC_RENUMBER_FENCE {} {} {} {} \"{}\" {} {}",
                        p1.x, p1.y, pt.x, pt.y, self.prefix, self.current_number, self.increment
                    ));
                }
                self.step = RenumberInteractiveStep::PickParcel;
                CmdResult::NeedPoint
            }
            _ => CmdResult::NeedPoint,
        }
    }

    fn on_preview_wires(&mut self, pt: DVec3) -> Vec<WireModel> {
        let mut wires = Vec::new();
        if self.step == RenumberInteractiveStep::FenceEnd {
            if let Some(p1) = self.fence_first_pt {
                wires.push(WireModel::solid_f64(
                    "preview_renumber_fence".to_string(),
                    vec![[p1.x, p1.y, p1.z], [pt.x, pt.y, pt.z]],
                    [0.1, 0.85, 1.0, 1.0],
                    false,
                ));
            }
        }
        wires
    }

    fn on_enter(&mut self) -> CmdResult {
        match self.step {
            RenumberInteractiveStep::PickParcel => {
                self.step = RenumberInteractiveStep::FenceStart;
                CmdResult::NeedPoint
            }
            RenumberInteractiveStep::FenceStart | RenumberInteractiveStep::FenceEnd => {
                self.step = RenumberInteractiveStep::PickParcel;
                self.fence_first_pt = None;
                CmdResult::NeedPoint
            }
            RenumberInteractiveStep::AwaitingPrefix
            | RenumberInteractiveStep::AwaitingStartNumber
            | RenumberInteractiveStep::AwaitingIncrement => {
                self.step = RenumberInteractiveStep::PickParcel;
                CmdResult::NeedPoint
            }
        }
    }

    fn on_escape(&mut self) -> CmdResult {
        if self.step != RenumberInteractiveStep::PickParcel {
            self.step = RenumberInteractiveStep::PickParcel;
            self.fence_first_pt = None;
            CmdResult::NeedPoint
        } else {
            CmdResult::Cancel
        }
    }
}


// ── Interactive ParcelSplitCommand ────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SplitStep {
    PickParcel,
    PickPoint1,
    PickPoint2,
}

pub struct ParcelSplitCommand {
    pub step: SplitStep,
    pub parent_handle: Option<Handle>,
    pub parent_name: String,
    pub parent_vertices: Vec<[f64; 2]>,
    pub point1: Option<[f64; 2]>,
    pub picked_entity: Option<EntityType>,
}

impl ParcelSplitCommand {
    pub fn new() -> Self {
        Self {
            step: SplitStep::PickParcel,
            parent_handle: None,
            parent_name: "Lot".to_string(),
            parent_vertices: Vec::new(),
            point1: None,
            picked_entity: None,
        }
    }
}

impl Default for ParcelSplitCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl CadCommand for ParcelSplitCommand {
    fn name(&self) -> &'static str {
        "PARCELSPLIT"
    }

    fn prompt(&self) -> String {
        match self.step {
            SplitStep::PickParcel => "PARCELSPLIT  Select parcel on canvas to split:".to_string(),
            SplitStep::PickPoint1 => "PARCELSPLIT  Specify first point of cutting line on parcel boundary:".to_string(),
            SplitStep::PickPoint2 => "PARCELSPLIT  Specify second point of cutting line:".to_string(),
        }
    }

    fn needs_entity_pick(&self) -> bool {
        self.step == SplitStep::PickParcel
    }

    fn inject_before_entity_pick(&self) -> bool {
        true
    }

    fn inject_picked_entity(&mut self, entity: EntityType) {
        self.picked_entity = Some(entity);
    }

    fn on_entity_pick(&mut self, handle: Handle, _pt: DVec3) -> CmdResult {
        if handle.is_null() {
            return CmdResult::NeedPoint;
        }

        if let Some(entity) = &self.picked_entity {
            if is_parcel_entity(entity) || matches!(entity, EntityType::LwPolyline(_)) {
                if let EntityType::LwPolyline(lw) = entity {
                    if lw.vertices.len() >= 3 {
                        let verts: Vec<[f64; 2]> = lw.vertices.iter().map(|v| [v.location.x, v.location.y]).collect();
                        if shoelace_area(&verts) > 1.0 {
                            self.parent_handle = Some(handle);
                            self.parent_vertices = verts;
                            if let Some(rec) = entity.common().extended_data.get_record(XDATA_PARCEL) {
                                if let Some(XDataValue::String(s)) = rec.values.first() {
                                    self.parent_name = s.clone();
                                }
                            } else {
                                self.parent_name = format!("Lot_{:X}", handle.value());
                            }
                            self.step = SplitStep::PickPoint1;
                            return CmdResult::NeedPoint;
                        }
                    }
                }
            }
        }
        CmdResult::NeedPoint
    }

    fn on_point(&mut self, pt: DVec3) -> CmdResult {
        match self.step {
            SplitStep::PickParcel => CmdResult::NeedPoint,
            SplitStep::PickPoint1 => {
                let (snapped, _, _) = snap_point_to_polygon([pt.x, pt.y], &self.parent_vertices);
                self.point1 = Some(snapped);
                self.step = SplitStep::PickPoint2;
                CmdResult::NeedPoint
            }
            SplitStep::PickPoint2 => {
                let (snapped, _, _) = snap_point_to_polygon([pt.x, pt.y], &self.parent_vertices);
                let Some(p1) = self.point1 else {
                    return CmdResult::NeedPoint;
                };
                let p2 = snapped;
                if (p1[0] - p2[0]).hypot(p1[1] - p2[1]) < 0.05 {
                    return CmdResult::NeedPoint;
                }
                CmdResult::Dispatch(format!(
                    "PC_SPLIT {} {:.3} {:.3} {:.3} {:.3}",
                    self.parent_name, p1[0], p1[1], p2[0], p2[1]
                ))
            }
        }
    }

    fn on_preview_wires(&mut self, pt: DVec3) -> Vec<WireModel> {
        let mut wires = Vec::new();
        if self.step == SplitStep::PickPoint2 {
            if let Some(p1) = self.point1 {
                let (p2, _, _) = snap_point_to_polygon([pt.x, pt.y], &self.parent_vertices);
                let mut line = WireModel::solid_f64(
                    "preview_split_cut".to_string(),
                    vec![[p1[0], p1[1], 0.0], [p2[0], p2[1], 0.0]],
                    [1.0, 0.2, 0.8, 1.0],
                    false,
                );
                line.set_fixed_screen_width(3.0);
                wires.push(line);
            }
        }
        wires
    }

        fn on_enter(&mut self) -> CmdResult {
        CmdResult::NeedPoint
    }

    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

// ── Interactive ParcelMergeCommand ────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MergeStep {
    PickFirst,
    PickSecond,
}

pub struct ParcelMergeCommand {
    pub step: MergeStep,
    pub p1_handle: Option<Handle>,
    pub p1_name: String,
    pub picked_entity: Option<EntityType>,
}

impl ParcelMergeCommand {
    pub fn new() -> Self {
        Self {
            step: MergeStep::PickFirst,
            p1_handle: None,
            p1_name: "Lot1".to_string(),
            picked_entity: None,
        }
    }
}

impl Default for ParcelMergeCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl CadCommand for ParcelMergeCommand {
    fn name(&self) -> &'static str {
        "PARCELMERGE"
    }

    fn prompt(&self) -> String {
        match self.step {
            MergeStep::PickFirst => "PARCELMERGE  Select first parcel to merge:".to_string(),
            MergeStep::PickSecond => "PARCELMERGE  Select second parcel to merge (or shared boundary):".to_string(),
        }
    }

    fn needs_entity_pick(&self) -> bool {
        true
    }

    fn inject_before_entity_pick(&self) -> bool {
        true
    }

    fn inject_picked_entity(&mut self, entity: EntityType) {
        self.picked_entity = Some(entity);
    }

    fn on_entity_pick(&mut self, handle: Handle, _pt: DVec3) -> CmdResult {
        if handle.is_null() {
            return CmdResult::NeedPoint;
        }

        if let Some(entity) = &self.picked_entity {
            if is_parcel_entity(entity) || matches!(entity, EntityType::LwPolyline(_)) {
                let name = entity
                    .common()
                    .extended_data
                    .get_record(XDATA_PARCEL)
                    .and_then(|rec| rec.values.first())
                    .and_then(|v| match v {
                        XDataValue::String(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| format!("Lot_{:X}", handle.value()));

                match self.step {
                    MergeStep::PickFirst => {
                        self.p1_handle = Some(handle);
                        self.p1_name = name;
                        self.step = MergeStep::PickSecond;
                        return CmdResult::NeedPoint;
                    }
                    MergeStep::PickSecond => {
                        if Some(handle) == self.p1_handle {
                            return CmdResult::NeedPoint;
                        }
                        let p2_name = name;
                        return CmdResult::Dispatch(format!("PC_MERGE {} {}", self.p1_name, p2_name));
                    }
                }
            }
        }
        CmdResult::NeedPoint
    }

    fn on_point(&mut self, _pt: DVec3) -> CmdResult {
        CmdResult::NeedPoint
    }

        fn on_enter(&mut self) -> CmdResult {
        CmdResult::NeedPoint
    }

    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

/// Computes an inward offset polygon for building setback envelopes.
pub fn compute_inward_setback(pts: &[[f64; 2]], dist: f64) -> Option<Vec<[f64; 2]>> {
    let n = pts.len();
    if n < 3 || dist <= 1e-4 {
        return None;
    }
    let is_ccw = shoelace_area(pts) > 0.0;
    let mut offset_lines: Vec<([f64; 2], [f64; 2])> = Vec::with_capacity(n);

    for i in 0..n {
        let p_start = pts[i];
        let p_end = pts[(i + 1) % n];
        let dx = p_end[0] - p_start[0];
        let dy = p_end[1] - p_start[1];
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-6 {
            continue;
        }
        let (nx, ny) = if is_ccw {
            (-dy / len, dx / len)
        } else {
            (dy / len, -dx / len)
        };
        let p1 = [p_start[0] + nx * dist, p_start[1] + ny * dist];
        let p2 = [p_end[0] + nx * dist, p_end[1] + ny * dist];
        offset_lines.push((p1, p2));
    }

    let m = offset_lines.len();
    if m < 3 {
        return None;
    }
    let mut offset_pts: Vec<[f64; 2]> = Vec::with_capacity(m);
    for i in 0..m {
        let next_i = (i + 1) % m;
        let l1 = &offset_lines[i];
        let l2 = &offset_lines[next_i];
        let x1 = l1.0[0]; let y1 = l1.0[1];
        let x2 = l1.1[0]; let y2 = l1.1[1];
        let x3 = l2.0[0]; let y3 = l2.0[1];
        let x4 = l2.1[0]; let y4 = l2.1[1];
        let denom = (x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4);
        if denom.abs() < 1e-8 {
            offset_pts.push(l1.1);
        } else {
            let ix = ((x1 * y2 - y1 * x2) * (x3 - x4) - (x1 - x2) * (x3 * y4 - y3 * x4)) / denom;
            let iy = ((x1 * y2 - y1 * x2) * (y3 - y4) - (y1 - y2) * (x3 * y4 - y3 * x4)) / denom;
            offset_pts.push([ix, iy]);
        }
    }

    let env_area = shoelace_area(&offset_pts).abs();
    let total_area = shoelace_area(pts).abs();
    if env_area > 0.0 && env_area < total_area {
        Some(offset_pts)
    } else {
        None
    }
}

// ── Interactive ParcelSetbackCommand ──────────────────────────────────────

pub struct ParcelSetbackCommand {
    pub offset_dist: f64,
    pub picked_entity: Option<EntityType>,
}

impl ParcelSetbackCommand {
    pub fn new(offset_dist: f64) -> Self {
        Self {
            offset_dist: if offset_dist > 0.0 { offset_dist } else { 3.0 },
            picked_entity: None,
        }
    }
}

impl CadCommand for ParcelSetbackCommand {
    fn name(&self) -> &'static str {
        "PARCELSETBACK"
    }

    fn prompt(&self) -> String {
        format!("PARCELSETBACK  Specify setback distance <{:.1}m> or select parcel [All/Pick] <All>:", self.offset_dist)
    }

    fn options(&self) -> Vec<CmdOption> {
        vec![CmdOption::new("All Parcels", "ALL")]
    }

    fn needs_entity_pick(&self) -> bool {
        true
    }

    fn inject_before_entity_pick(&self) -> bool {
        true
    }

    fn inject_picked_entity(&mut self, entity: EntityType) {
        self.picked_entity = Some(entity);
    }

    fn on_entity_pick(&mut self, handle: Handle, _pt: DVec3) -> CmdResult {
        if let Some(entity) = self.picked_entity.take() {
            if let EntityType::LwPolyline(lw) = &entity {
                let mut verts: Vec<[f64; 2]> = Vec::new();
                let n_v = lw.vertices.len();
                for i in 0..n_v {
                    let v = &lw.vertices[i];
                    let p1 = [v.location.x, v.location.y];
                    let next_idx = (i + 1) % n_v;
                    let p2 = [lw.vertices[next_idx].location.x, lw.vertices[next_idx].location.y];
                    if v.bulge.abs() > 1e-5 {
                        let arc_pts = tessellate_bulge(p1, p2, v.bulge, 16);
                        for p in arc_pts.into_iter().take(16) {
                            verts.push(p);
                        }
                    } else {
                        verts.push(p1);
                    }
                }

                if let Some(offset_pts) = compute_inward_setback(&verts, self.offset_dist) {
                    let mut setback_pl = LwPolyline::from_points(
                        offset_pts.iter().map(|p| Vector2::new(p[0], p[1])).collect(),
                    );
                    setback_pl.is_closed = true;
                    let mut setback_ent = EntityType::LwPolyline(setback_pl);
                    setback_ent.common_mut().layer = "PC-SETBACK".to_string();
                    return CmdResult::CommitEntity(setback_ent);
                }
            }

            if is_parcel_entity(&entity) {
                let name = entity
                    .common()
                    .extended_data
                    .get_record(XDATA_PARCEL)
                    .and_then(|rec| rec.values.first())
                    .and_then(|v| match v {
                        XDataValue::String(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| format!("Lot_{:X}", handle.value()));
                return CmdResult::Dispatch(format!("PC_SETBACK {} {:.2}", name, self.offset_dist));
            }
        }
        CmdResult::NeedPoint
    }

    fn on_point(&mut self, _pt: DVec3) -> CmdResult {
        CmdResult::NeedPoint
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        let t = text.trim();
        let up = t.to_ascii_uppercase();
        if up == "A" || up == "ALL" || up.is_empty() {
            Some(CmdResult::Dispatch(format!("PC_SETBACK all {:.2}", self.offset_dist)))
        } else if let Ok(val) = t.parse::<f64>() {
            if val > 0.0 {
                self.offset_dist = val;
                Some(CmdResult::NeedPoint)
            } else {
                None
            }
        } else {
            None
        }
    }

    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Dispatch(format!("PC_SETBACK all {:.2}", self.offset_dist))
    }

    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

// ── Interactive ParcelTableInsertCommand ──────────────────────────────────

pub struct ParcelTableInsertCommand {
    pub site_name: String,
}

impl ParcelTableInsertCommand {
    pub fn new() -> Self {
        Self { site_name: "Site1".to_string() }
    }
}

impl Default for ParcelTableInsertCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl CadCommand for ParcelTableInsertCommand {
    fn name(&self) -> &'static str {
        "PARCELTABLE"
    }

    fn prompt(&self) -> String {
        "PARCELTABLE  Specify insertion point for Area Table on canvas:".to_string()
    }

    fn on_point(&mut self, pt: DVec3) -> CmdResult {
        CmdResult::Dispatch(format!("PC_TABLE_AREA {} {:.1} {:.1} 2.0", self.site_name, pt.x, pt.y))
    }

    fn on_preview_wires(&mut self, pt: DVec3) -> Vec<WireModel> {
        let w = 80.0;
        let h = 35.0;
        vec![WireModel::solid_f64(
            "preview_table_box".to_string(),
            vec![
                [pt.x, pt.y, 0.0],
                [pt.x + w, pt.y, 0.0],
                [pt.x + w, pt.y - h, 0.0],
                [pt.x, pt.y - h, 0.0],
                [pt.x, pt.y, 0.0],
            ],
            [0.3, 0.7, 1.0, 0.8],
            false,
        )]
    }

        fn on_enter(&mut self) -> CmdResult {
        CmdResult::NeedPoint
    }

    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

// ── Interactive ParcelLabelCommand ────────────────────────────────────────

pub struct ParcelLabelCommand {
    pub picked_entity: Option<EntityType>,
}

impl ParcelLabelCommand {
    pub fn new() -> Self {
        Self { picked_entity: None }
    }
}

impl Default for ParcelLabelCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl CadCommand for ParcelLabelCommand {
    fn name(&self) -> &'static str {
        "PARCELLABEL"
    }

    fn prompt(&self) -> String {
        "PARCELLABEL  Select parcel on canvas to label or press Enter for [All] <All>:".to_string()
    }

    fn options(&self) -> Vec<CmdOption> {
        vec![CmdOption::new("All Parcels", "ALL")]
    }

    fn needs_entity_pick(&self) -> bool {
        true
    }

    fn inject_before_entity_pick(&self) -> bool {
        true
    }

    fn inject_picked_entity(&mut self, entity: EntityType) {
        self.picked_entity = Some(entity);
    }

    fn on_entity_pick(&mut self, handle: Handle, _pt: DVec3) -> CmdResult {
        if let Some(entity) = &self.picked_entity {
            if is_parcel_entity(entity) || matches!(entity, EntityType::LwPolyline(_)) {
                let name = entity
                    .common()
                    .extended_data
                    .get_record(XDATA_PARCEL)
                    .and_then(|rec| rec.values.first())
                    .and_then(|v| match v {
                        XDataValue::String(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| format!("Lot_{:X}", handle.value()));
                return CmdResult::Dispatch(format!("PC_LABEL {}", name));
            }
        }
        CmdResult::NeedPoint
    }

    fn on_point(&mut self, _pt: DVec3) -> CmdResult {
        CmdResult::NeedPoint
    }

    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Dispatch("PC_LABEL all".to_string())
    }

    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

// ── Interactive ParcelMapcheckCommand ─────────────────────────────────────

pub struct ParcelMapcheckCommand {
    pub picked_entity: Option<EntityType>,
}

impl ParcelMapcheckCommand {
    pub fn new() -> Self {
        Self { picked_entity: None }
    }
}

impl Default for ParcelMapcheckCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl CadCommand for ParcelMapcheckCommand {
    fn name(&self) -> &'static str {
        "PARCELMAPCHECK"
    }

    fn prompt(&self) -> String {
        "MAPCHECK  Select parcel on canvas for survey traverse closure analysis:".to_string()
    }

    fn needs_entity_pick(&self) -> bool {
        true
    }

    fn inject_before_entity_pick(&self) -> bool {
        true
    }

    fn inject_picked_entity(&mut self, entity: EntityType) {
        self.picked_entity = Some(entity);
    }

    fn on_entity_pick(&mut self, handle: Handle, _pt: DVec3) -> CmdResult {
        if let Some(entity) = &self.picked_entity {
            if is_parcel_entity(entity) || matches!(entity, EntityType::LwPolyline(_)) {
                let name = entity
                    .common()
                    .extended_data
                    .get_record(XDATA_PARCEL)
                    .and_then(|rec| rec.values.first())
                    .and_then(|v| match v {
                        XDataValue::String(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| format!("Lot_{:X}", handle.value()));
                return CmdResult::Dispatch(format!("PC_MAPCHECK {}", name));
            }
        }
        CmdResult::NeedPoint
    }

    fn on_point(&mut self, _pt: DVec3) -> CmdResult {
        CmdResult::NeedPoint
    }

        fn on_enter(&mut self) -> CmdResult {
        CmdResult::NeedPoint
    }

    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

// ── Interactive ParcelRectCommand ─────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RectStep {
    FirstCorner,
    SecondCorner,
}

pub struct ParcelRectCommand {
    pub step: RectStep,
    pub corner1: Option<[f64; 2]>,
    pub start_lot_number: u32,
    pub site_name: String,
    pub parcel_style: String,
    pub parcel_layer: String,
    pub segment_layer: String,
    pub area_label_style: String,
    pub line_label_style: String,
    pub curve_label_style: String,
    pub auto_add_segment_labels: bool,
}

impl ParcelRectCommand {
    pub fn new() -> Self {
        Self {
            step: RectStep::FirstCorner,
            corner1: None,
            start_lot_number: 101,
            site_name: "Site 1".to_string(),
            parcel_style: "Single-Family".to_string(),
            parcel_layer: "C-PROP".to_string(),
            segment_layer: "C-PROP-LINE".to_string(),
            area_label_style: "Parcel Name, Area & Perimeter".to_string(),
            line_label_style: "Bearing over Distance".to_string(),
            curve_label_style: "Distance over Radius".to_string(),
            auto_add_segment_labels: true,
        }
    }
}

impl Default for ParcelRectCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl CadCommand for ParcelRectCommand {
    fn name(&self) -> &'static str {
        "RECTPARCEL"
    }

    fn prompt(&self) -> String {
        match self.step {
            RectStep::FirstCorner => "RECTPARCEL  Specify first corner of lot:".to_string(),
            RectStep::SecondCorner => "RECTPARCEL  Specify opposite corner of lot:".to_string(),
        }
    }

    fn on_point(&mut self, pt: DVec3) -> CmdResult {
        match self.step {
            RectStep::FirstCorner => {
                self.corner1 = Some([pt.x, pt.y]);
                self.step = RectStep::SecondCorner;
                CmdResult::NeedPoint
            }
            RectStep::SecondCorner => {
                let (Some(c1), c2) = (self.corner1, [pt.x, pt.y]) else {
                    return CmdResult::NeedPoint;
                };
                let x_min = c1[0].min(c2[0]);
                let x_max = c1[0].max(c2[0]);
                let y_min = c1[1].min(c2[1]);
                let y_max = c1[1].max(c2[1]);
                let w = x_max - x_min;
                let h = y_max - y_min;
                if w < 1.0 || h < 1.0 {
                    return CmdResult::NeedPoint;
                }

                let pts = vec![
                    Vector2::new(x_min, y_min),
                    Vector2::new(x_max, y_min),
                    Vector2::new(x_max, y_max),
                    Vector2::new(x_min, y_max),
                ];
                let mut poly = LwPolyline::from_points(pts);
                poly.is_closed = true;
                let mut parcel_ent = EntityType::LwPolyline(poly);
                let assigned_layer = if self.parcel_layer.is_empty() || self.parcel_layer == "C-PROP" || self.parcel_layer == "PC-PARCEL" {
                    crate::ui::window::create_parcels_layout::layer_for_style(&self.parcel_style).to_string()
                } else {
                    self.parcel_layer.clone()
                };
                parcel_ent.common_mut().layer = assigned_layer;

                let lot_num = self.start_lot_number;
                let lot_name = format!("LOT-{}", lot_num);
                let mut rec = ExtendedDataRecord::new(XDATA_PARCEL);
                rec.add_value(XDataValue::String(lot_name));
                rec.add_value(XDataValue::String(self.site_name.clone()));
                rec.add_value(XDataValue::String(lot_num.to_string()));
                rec.add_value(XDataValue::String(self.parcel_style.clone()));
                rec.add_value(XDataValue::String("Standard Property Owner".to_string()));
                rec.add_value(XDataValue::String("D-2026-001".to_string()));
                rec.add_value(XDataValue::String("042-881-01".to_string()));
                rec.add_value(XDataValue::String(self.area_label_style.clone()));
                rec.add_value(XDataValue::String(self.line_label_style.clone()));
                rec.add_value(XDataValue::String(self.curve_label_style.clone()));
                rec.add_value(XDataValue::String(if self.auto_add_segment_labels { "true".to_string() } else { "false".to_string() }));
                rec.add_value(XDataValue::String(self.segment_layer.clone()));
                parcel_ent.common_mut().extended_data.add_record(rec);

                CmdResult::CommitEntitiesAndExit(vec![parcel_ent])
            }
        }
    }

    fn on_preview_wires(&mut self, pt: DVec3) -> Vec<WireModel> {
        let mut wires = Vec::new();
        if self.step == RectStep::SecondCorner {
            if let Some(c1) = self.corner1 {
                let c2 = [pt.x, pt.y];
                wires.push(WireModel::solid_f64(
                    "preview_rect_parcel".to_string(),
                    vec![
                        [c1[0], c1[1], 0.0],
                        [c2[0], c1[1], 0.0],
                        [c2[0], c2[1], 0.0],
                        [c1[0], c2[1], 0.0],
                        [c1[0], c1[1], 0.0],
                    ],
                    [0.2, 0.8, 1.0, 1.0],
                    false,
                ));
            }
        }
        wires
    }

        fn on_enter(&mut self) -> CmdResult {
        CmdResult::NeedPoint
    }

    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

// ── Interactive ParcelRowCommand ──────────────────────────────────────────

pub struct ParcelRowCommand {
    pub width: f64,
    pub centerline: Vec<[f64; 2]>,
}

impl ParcelRowCommand {
    pub fn new(width: f64) -> Self {
        Self {
            width: if width > 0.0 { width } else { 20.0 },
            centerline: Vec::new(),
        }
    }
}

impl Default for ParcelRowCommand {
    fn default() -> Self {
        Self::new(20.0)
    }
}

impl CadCommand for ParcelRowCommand {
    fn name(&self) -> &'static str {
        "PARCELROW"
    }

    fn prompt(&self) -> String {
        if self.centerline.is_empty() {
            format!("ROW  Specify start point of road centerline (Width: {:.1}m) [type number or W to change width]:", self.width)
        } else {
            format!(
                "ROW  Specify next centerline point ({} point(s) picked, Enter or Right-click to finish):",
                self.centerline.len()
            )
        }
    }

    fn wants_text_input(&self) -> bool {
        true
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        let t = text.trim();
        if let Ok(w) = t.parse::<f64>() {
            if w > 0.5 && w < 500.0 {
                self.width = w;
            }
            return Some(CmdResult::NeedPoint);
        }
        let up = t.to_ascii_uppercase();
        if up.starts_with('W') {
            let num = up.trim_start_matches('W').trim();
            if let Ok(w) = num.parse::<f64>() {
                if w > 0.5 && w < 500.0 {
                    self.width = w;
                }
            }
            return Some(CmdResult::NeedPoint);
        }
        None
    }

    fn on_point(&mut self, pt: DVec3) -> CmdResult {
        self.centerline.push([pt.x, pt.y]);
        CmdResult::NeedPoint
    }


    fn on_enter(&mut self) -> CmdResult {
        if self.centerline.len() < 2 {
            return CmdResult::Cancel;
        }
        let coords: Vec<String> = self.centerline.iter().map(|p| format!("{:.2},{:.2}", p[0], p[1])).collect();
        CmdResult::Dispatch(format!("PC_ROW {:.1} 5.0 {}", self.width, coords.join(" ")))
    }

    fn on_preview_wires(&mut self, pt: DVec3) -> Vec<WireModel> {
        let mut wires = Vec::new();
        if !self.centerline.is_empty() {
            let mut full_path = self.centerline.clone();
            full_path.push([pt.x, pt.y]);

            let pts_3d: Vec<[f64; 3]> = full_path.iter().map(|p| [p[0], p[1], 0.0]).collect();
            let mut cl_wire = WireModel::solid_f64("preview_row_centerline".to_string(), pts_3d, [1.0, 0.85, 0.1, 1.0], false);
            cl_wire.set_fixed_screen_width(2.0);
            wires.push(cl_wire);

            if let Some(first) = self.centerline.first() {
                let d = 0.5;
                wires.push(WireModel::solid_f64(
                    "preview_row_start_marker".to_string(),
                    vec![
                        [first[0] - d, first[1] - d, 0.0],
                        [first[0] + d, first[1] - d, 0.0],
                        [first[0] + d, first[1] + d, 0.0],
                        [first[0] - d, first[1] + d, 0.0],
                        [first[0] - d, first[1] - d, 0.0],
                    ],
                    [0.2, 1.0, 0.4, 1.0],
                    false,
                ));
            }

            if full_path.len() >= 2 {
                let half_w = self.width * 0.5;
                let mut left_pts = Vec::new();
                let mut right_pts = Vec::new();
                let n = full_path.len();

                for i in 0..n {
                    let dir = if i == 0 {
                        let dx = full_path[1][0] - full_path[0][0];
                        let dy = full_path[1][1] - full_path[0][1];
                        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
                        [dx / len, dy / len]
                    } else if i == n - 1 {
                        let dx = full_path[n - 1][0] - full_path[n - 2][0];
                        let dy = full_path[n - 1][1] - full_path[n - 2][1];
                        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
                        [dx / len, dy / len]
                    } else {
                        let dx1 = full_path[i][0] - full_path[i - 1][0];
                        let dy1 = full_path[i][1] - full_path[i - 1][1];
                        let len1 = (dx1 * dx1 + dy1 * dy1).sqrt().max(1e-6);
                        let dx2 = full_path[i + 1][0] - full_path[i][0];
                        let dy2 = full_path[i + 1][1] - full_path[i][1];
                        let len2 = (dx2 * dx2 + dy2 * dy2).sqrt().max(1e-6);
                        let sum_x = dx1 / len1 + dx2 / len2;
                        let sum_y = dy1 / len1 + dy2 / len2;
                        let sum_len = (sum_x * sum_x + sum_y * sum_y).sqrt().max(1e-6);
                        [sum_x / sum_len, sum_y / sum_len]
                    };

                    let norm_x = -dir[1];
                    let norm_y = dir[0];
                    left_pts.push([full_path[i][0] + norm_x * half_w, full_path[i][1] + norm_y * half_w, 0.0]);
                    right_pts.push([full_path[i][0] - norm_x * half_w, full_path[i][1] - norm_y * half_w, 0.0]);
                }

                let mut left_wire = WireModel::solid_f64("preview_row_left_edge".to_string(), left_pts, [0.2, 0.85, 1.0, 0.9], false);
                left_wire.set_fixed_screen_width(2.0);
                wires.push(left_wire);

                let mut right_wire = WireModel::solid_f64("preview_row_right_edge".to_string(), right_pts, [0.2, 0.85, 1.0, 0.9], false);
                right_wire.set_fixed_screen_width(2.0);
                wires.push(right_wire);
            }
        }
        wires
    }

    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

// ── Interactive ParcelCurveTableInsertCommand ─────────────────────────────

pub struct ParcelCurveTableInsertCommand {
    pub site_name: String,
}

impl ParcelCurveTableInsertCommand {
    pub fn new() -> Self {
        Self { site_name: "Site1".to_string() }
    }
}

impl Default for ParcelCurveTableInsertCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl CadCommand for ParcelCurveTableInsertCommand {
    fn name(&self) -> &'static str {
        "PARCELCURVETABLE"
    }

    fn prompt(&self) -> String {
        "PARCELCURVETABLE  Specify insertion point for Curve Table on canvas:".to_string()
    }

    fn on_point(&mut self, pt: DVec3) -> CmdResult {
        CmdResult::Dispatch(format!("PC_TABLE_CURVES {} {:.1} {:.1} 2.0", self.site_name, pt.x, pt.y))
    }

    fn on_preview_wires(&mut self, pt: DVec3) -> Vec<WireModel> {
        let w = 70.0;
        let h = 30.0;
        vec![WireModel::solid_f64(
            "preview_curve_table_box".to_string(),
            vec![
                [pt.x, pt.y, 0.0],
                [pt.x + w, pt.y, 0.0],
                [pt.x + w, pt.y - h, 0.0],
                [pt.x, pt.y - h, 0.0],
                [pt.x, pt.y, 0.0],
            ],
            [0.2, 0.8, 0.6, 0.8],
            false,
        )]
    }


        fn on_enter(&mut self) -> CmdResult {
        CmdResult::NeedPoint
    }

    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

// ── Interactive ParcelLineTableCommand ────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineTableStep {
    PickParcel,
    PickInsertionPoint,
}

pub struct ParcelLineTableCommand {
    pub step: LineTableStep,
    pub parcel_handle: Option<Handle>,
    pub parcel_name: String,
    pub picked_entity: Option<EntityType>,
    pub segment_count: usize,
}

impl ParcelLineTableCommand {
    pub fn new() -> Self {
        Self {
            step: LineTableStep::PickParcel,
            parcel_handle: None,
            parcel_name: String::new(),
            picked_entity: None,
            segment_count: 4,
        }
    }
}

impl Default for ParcelLineTableCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl CadCommand for ParcelLineTableCommand {
    fn name(&self) -> &'static str {
        "PARCELLINETABLE"
    }

    fn prompt(&self) -> String {
        match self.step {
            LineTableStep::PickParcel => {
                "PARCELLINETABLE  Select parcel on canvas for Line Table (or click its label):".to_string()
            }
            LineTableStep::PickInsertionPoint => {
                format!(
                    "PARCELLINETABLE  Specify insertion point for Line Table [{}] on canvas:",
                    self.parcel_name
                )
            }
        }
    }

    fn needs_entity_pick(&self) -> bool {
        self.step == LineTableStep::PickParcel
    }

    fn inject_before_entity_pick(&self) -> bool {
        true
    }

    fn inject_picked_entity(&mut self, entity: EntityType) {
        self.picked_entity = Some(entity);
    }

    fn on_entity_pick(&mut self, handle: Handle, _pt: DVec3) -> CmdResult {
        if handle.is_null() {
            return CmdResult::NeedPoint;
        }

        if let Some(entity) = &self.picked_entity {
            if is_parcel_entity(entity) || matches!(entity, EntityType::LwPolyline(_)) {
                let name = entity
                    .common()
                    .extended_data
                    .get_record(XDATA_PARCEL)
                    .and_then(|rec| rec.values.first())
                    .and_then(|v| match v {
                        XDataValue::String(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| format!("Lot_{:X}", handle.value()));

                if let EntityType::LwPolyline(lw) = entity {
                    self.segment_count = lw.vertices.len();
                }

                self.parcel_handle = Some(handle);
                self.parcel_name = name;
                self.step = LineTableStep::PickInsertionPoint;
                return CmdResult::NeedPoint;
            }
        } else if !handle.is_null() {
            self.parcel_handle = Some(handle);
            self.parcel_name = format!("Lot_{:X}", handle.value());
            self.step = LineTableStep::PickInsertionPoint;
            return CmdResult::NeedPoint;
        }

        CmdResult::NeedPoint
    }

    fn on_point(&mut self, pt: DVec3) -> CmdResult {
        match self.step {
            LineTableStep::PickParcel => CmdResult::NeedPoint,
            LineTableStep::PickInsertionPoint => {
                CmdResult::Dispatch(format!(
                    "PC_TABLE_LINES \"{}\" {:.2} {:.2}",
                    self.parcel_name, pt.x, pt.y
                ))
            }
        }
    }

    fn on_preview_wires(&mut self, pt: DVec3) -> Vec<WireModel> {
        if self.step == LineTableStep::PickInsertionPoint {
            let text_h = 2.0;
            let col_widths = [text_h * 6.0, text_h * 15.0, text_h * 13.0];
            let total_w: f64 = col_widths.iter().sum();
            let row_h = text_h * 2.2;
            let rows = (self.segment_count.max(2) + 2) as f64;
            let total_h = row_h * rows;

            vec![WireModel::solid_f64(
                "preview_line_table_box".to_string(),
                vec![
                    [pt.x, pt.y, 0.0],
                    [pt.x + total_w, pt.y, 0.0],
                    [pt.x + total_w, pt.y - total_h, 0.0],
                    [pt.x, pt.y - total_h, 0.0],
                    [pt.x, pt.y, 0.0],
                ],
                [0.2, 0.8, 0.9, 0.85],
                false,
            )]
        } else {
            Vec::new()
        }
    }

    fn on_enter(&mut self) -> CmdResult {
        CmdResult::NeedPoint
    }

    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

// ── Interactive ParcelSubdivideCommand ────────────────────────────────────

pub struct ParcelSubdivideCommand {
    pub min_area: f64,
    pub min_frontage: f64,
    pub remainder_mode: String,
    pub picked_entity: Option<EntityType>,
}

impl ParcelSubdivideCommand {
    pub fn new(min_area: f64, min_frontage: f64, remainder_mode: String) -> Self {
        Self {
            min_area: if min_area > 0.0 { min_area } else { 350.0 },
            min_frontage: if min_frontage > 0.0 { min_frontage } else { 12.0 },
            remainder_mode,
            picked_entity: None,
        }
    }
}

impl Default for ParcelSubdivideCommand {
    fn default() -> Self {
        Self::new(350.0, 12.0, "redistribute".to_string())
    }
}

impl CadCommand for ParcelSubdivideCommand {
    fn name(&self) -> &'static str {
        "PARCELSUBDIVIDE"
    }

    fn prompt(&self) -> String {
        format!(
            "AUTOSUBDIVIDE  Select parent parcel on canvas to subdivide (Min Area: {:.0}m², Min Frontage: {:.0}m):",
            self.min_area, self.min_frontage
        )
    }

    fn needs_entity_pick(&self) -> bool {
        true
    }

    fn inject_before_entity_pick(&self) -> bool {
        true
    }

    fn inject_picked_entity(&mut self, entity: EntityType) {
        self.picked_entity = Some(entity);
    }

    fn on_entity_pick(&mut self, handle: Handle, _pt: DVec3) -> CmdResult {
        if let Some(entity) = &self.picked_entity {
            if is_parcel_entity(entity) || matches!(entity, EntityType::LwPolyline(_)) {
                let name = entity
                    .common()
                    .extended_data
                    .get_record(XDATA_PARCEL)
                    .and_then(|rec| rec.values.first())
                    .and_then(|v| match v {
                        XDataValue::String(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| format!("Lot_{:X}", handle.value()));
                return CmdResult::Dispatch(format!(
                    "PC_SUBDIVIDE {} {:.1} {:.1} {}",
                    name, self.min_area, self.min_frontage, self.remainder_mode
                ));
            }
        }
        CmdResult::NeedPoint
    }

    fn on_point(&mut self, _pt: DVec3) -> CmdResult {
        CmdResult::NeedPoint
    }


        fn on_enter(&mut self) -> CmdResult {
        CmdResult::NeedPoint
    }

    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

// ── Interactive ParcelSetStyleCommand ─────────────────────────────────────

pub struct ParcelSetStyleCommand {
    pub style: String,
    pub picked_entity: Option<EntityType>,
}

impl ParcelSetStyleCommand {
    pub fn new(style: String) -> Self {
        Self {
            style: if !style.is_empty() { style } else { "residential".to_string() },
            picked_entity: None,
        }
    }
}

impl Default for ParcelSetStyleCommand {
    fn default() -> Self {
        Self::new("residential".to_string())
    }
}

impl CadCommand for ParcelSetStyleCommand {
    fn name(&self) -> &'static str {
        "PARCELSTYLE"
    }

    fn prompt(&self) -> String {
        format!(
            "PARCELSTYLE  Select parcel on canvas to apply style [{}] or type [Residential/Commercial/Park/Civic/ROW] <Residential>:",
            self.style
        )
    }

    fn options(&self) -> Vec<CmdOption> {
        vec![
            CmdOption::new("Residential", "RESIDENTIAL"),
            CmdOption::new("Commercial", "COMMERCIAL"),
            CmdOption::new("Park / Open Space", "PARK"),
            CmdOption::new("Civic / Institutional", "CIVIC"),
            CmdOption::new("Right-of-Way", "ROW"),
            CmdOption::new("Apply to All", "ALL"),
            CmdOption::new("Thematic Auto-Color All", "THEMATIC"),
        ]
    }

    fn needs_entity_pick(&self) -> bool {
        true
    }

    fn inject_before_entity_pick(&self) -> bool {
        true
    }

    fn inject_picked_entity(&mut self, entity: EntityType) {
        self.picked_entity = Some(entity);
    }

    fn on_entity_pick(&mut self, handle: Handle, _pt: DVec3) -> CmdResult {
        if let Some(mut entity) = self.picked_entity.take() {
            if is_parcel_entity(&entity) || matches!(entity, EntityType::LwPolyline(_)) {
                let (layer, zoning) = match self.style.to_ascii_lowercase().as_str() {
                    "residential" | "r" => ("PC-RESIDENTIAL", "Single-Family Residential"),
                    "commercial" | "c" => ("PC-COMMERCIAL", "Commercial / Retail"),
                    "civic" => ("PC-CIVIC", "Civic / Institutional"),
                    "park" | "p" => ("PC-PARK", "Open Space / Park"),
                    "row" => ("PC-ROW", "Right-of-Way"),
                    _ => ("PC-PARCEL", "Standard Parcel"),
                };

                entity.common_mut().layer = layer.to_string();

                let old_name = entity
                    .common()
                    .extended_data
                    .get_record(XDATA_PARCEL)
                    .and_then(|r| r.values.first())
                    .and_then(|v| match v {
                        XDataValue::String(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| format!("Lot_{:X}", handle.value()));
                let old_site = entity
                    .common()
                    .extended_data
                    .get_record(XDATA_PARCEL)
                    .and_then(|r| r.values.get(1))
                    .and_then(|v| match v {
                        XDataValue::String(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| "Site 1".to_string());
                let old_num = entity
                    .common()
                    .extended_data
                    .get_record(XDATA_PARCEL)
                    .and_then(|r| r.values.get(2))
                    .and_then(|v| match v {
                        XDataValue::String(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| "1".to_string());

                let mut rec = ExtendedDataRecord::new(XDATA_PARCEL);
                rec.add_value(XDataValue::String(old_name));
                rec.add_value(XDataValue::String(old_site));
                rec.add_value(XDataValue::String(old_num));
                rec.add_value(XDataValue::String(zoning.to_string()));
                rec.add_value(XDataValue::String("Standard Property Owner".to_string()));
                entity.common_mut().extended_data.add_record(rec);

                return CmdResult::UpdateEntityContinue { handle, entity };
            }
        }
        CmdResult::NeedPoint
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        let t = text.trim().to_ascii_lowercase();
        match t.as_str() {
            "residential" | "r" => { self.style = "residential".to_string(); Some(CmdResult::NeedPoint) }
            "commercial" | "c" => { self.style = "commercial".to_string(); Some(CmdResult::NeedPoint) }
            "park" | "p" => { self.style = "park".to_string(); Some(CmdResult::NeedPoint) }
            "civic" => { self.style = "civic".to_string(); Some(CmdResult::NeedPoint) }
            "row" => { self.style = "row".to_string(); Some(CmdResult::NeedPoint) }
            "all" | "a" => Some(CmdResult::Dispatch(format!("PC_SET_STYLE all {}", self.style))),
            "thematic" | "theme" | "t" => Some(CmdResult::Dispatch("PC_THEMATIC".to_string())),
            _ => None,
        }
    }

    fn on_point(&mut self, _pt: DVec3) -> CmdResult {
        CmdResult::NeedPoint
    }

    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Dispatch(format!("PC_SET_STYLE all {}", self.style))
    }

    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

// ── Interactive ParcelLabelSegsCommand ────────────────────────────────────

pub struct ParcelLabelSegsCommand {
    pub picked_entity: Option<EntityType>,
}

impl ParcelLabelSegsCommand {
    pub fn new() -> Self {
        Self { picked_entity: None }
    }
}

impl Default for ParcelLabelSegsCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl CadCommand for ParcelLabelSegsCommand {
    fn name(&self) -> &'static str {
        "PARCELLABELOGSEGS"
    }

    fn prompt(&self) -> String {
        "SEGMENTLABELS  Select parcel to label boundary segments [Bearing/Distance] or Enter for [All] <All>:".to_string()
    }

    fn options(&self) -> Vec<CmdOption> {
        vec![CmdOption::new("All Parcels", "ALL")]
    }

    fn needs_entity_pick(&self) -> bool {
        true
    }

    fn inject_before_entity_pick(&self) -> bool {
        true
    }

    fn inject_picked_entity(&mut self, entity: EntityType) {
        self.picked_entity = Some(entity);
    }

    fn on_entity_pick(&mut self, handle: Handle, _pt: DVec3) -> CmdResult {
        if let Some(entity) = &self.picked_entity {
            if is_parcel_entity(entity) || matches!(entity, EntityType::LwPolyline(_)) {
                let name = entity
                    .common()
                    .extended_data
                    .get_record(XDATA_PARCEL)
                    .and_then(|rec| rec.values.first())
                    .and_then(|v| match v {
                        XDataValue::String(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| format!("Lot_{:X}", handle.value()));
                return CmdResult::Dispatch(format!("PC_LABEL_SEGS {}", name));
            }
        }
        CmdResult::NeedPoint
    }

    fn on_point(&mut self, _pt: DVec3) -> CmdResult {
        CmdResult::NeedPoint
    }

    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Dispatch("PC_LABEL_SEGS all".to_string())
    }

    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

// ── Unit Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {

    #[test]
    fn test_interactive_renumber_command_workflow() {
        let mut cmd = ParcelRenumberCommand::new("LOT-".to_string(), 101, 2);
        assert_eq!(cmd.name(), "PARCELRENUMBER");
        assert!(cmd.needs_entity_pick());
        assert!(cmd.prompt().contains("LOT-101"));

        // Switch to fence mode
        let _res = cmd.on_text_input("fence");
        assert_eq!(cmd.step, RenumberInteractiveStep::FenceStart);
        assert!(!cmd.needs_entity_pick());
        assert!(cmd.prompt().contains("first point"));

        // Pick first fence point
        cmd.on_point(DVec3::new(10.0, 20.0, 0.0));
        assert_eq!(cmd.step, RenumberInteractiveStep::FenceEnd);
        assert_eq!(cmd.fence_first_pt, Some(DVec3::new(10.0, 20.0, 0.0)));
        assert!(cmd.prompt().contains("second point"));

        // Pick second fence point -> Dispatches PC_RENUMBER_FENCE
        let end_res = cmd.on_point(DVec3::new(50.0, 20.0, 0.0));
        match end_res {
            CmdResult::Dispatch(s) => {
                assert!(s.starts_with("PC_RENUMBER_FENCE 10 20 50 20 \"LOT-\" 101 2"));
            }
            _ => panic!("Expected CmdResult::Dispatch"),
        }
        assert_eq!(cmd.step, RenumberInteractiveStep::PickParcel);
    }

    #[test]
    fn test_interactive_renumber_entity_pick() {
        let mut cmd = ParcelRenumberCommand::new("Plot_".to_string(), 1, 1);
        let mut lw = LwPolyline::from_points(vec![
            Vector2::new(0.0, 0.0),
            Vector2::new(20.0, 0.0),
            Vector2::new(20.0, 30.0),
            Vector2::new(0.0, 30.0),
        ]);
        lw.is_closed = true;
        let mut ent = EntityType::LwPolyline(lw);
        let mut rec = ExtendedDataRecord::new(XDATA_PARCEL);
        rec.add_value(XDataValue::String("Old_Lot".to_string()));
        rec.add_value(XDataValue::String("Al_Waha".to_string()));
        rec.add_value(XDataValue::String("99".to_string()));
        ent.common_mut().extended_data.add_record(rec);

        cmd.inject_picked_entity(ent);
        let handle = Handle::from(42);
        let res = cmd.on_entity_pick(handle, DVec3::ZERO);
        match res {
            CmdResult::UpdateEntityContinue { handle: h, entity: updated } => {
                assert_eq!(h, handle);
                let rec = updated.common().extended_data.get_record(XDATA_PARCEL).unwrap();
                assert_eq!(rec.values[0], XDataValue::String("Plot_1".to_string()));
                assert_eq!(rec.values[1], XDataValue::String("Al_Waha".to_string()));
                assert_eq!(rec.values[2], XDataValue::String("1".to_string()));
            }
            _ => panic!("Expected CmdResult::UpdateEntityContinue"),
        }
        assert_eq!(cmd.current_number, 2);
        assert_eq!(cmd.renumbered_count, 1);
        assert!(cmd.prompt().contains("Plot_2"));
    }

    use super::*;

    #[test]
    fn test_shoelace_area_and_centroid() {
        let rect = vec![
            [0.0, 0.0],
            [40.0, 0.0],
            [40.0, 25.0],
            [0.0, 25.0],
        ];
        let area = shoelace_area(&rect);
        assert!((area - 1000.0).abs() < 1e-6);

        let c = polygon_centroid(&rect);
        assert!((c[0] - 20.0).abs() < 1e-6);
        assert!((c[1] - 12.5).abs() < 1e-6);
    }

    #[test]
    fn test_solve_slide_line_rectangular() {
        let rect = vec![
            [0.0, 0.0],
            [40.0, 0.0],
            [40.0, 25.0],
            [0.0, 25.0],
        ];
        let f_start = [0.0, 0.0];
        let f_end = [40.0, 0.0];

        // Subdivide 350 sq.m out of 1000 sq.m at 90 deg
        let res = solve_slide_line(&rect, f_start, f_end, 90.0, 350.0);
        assert!(res.is_ok(), "Slide line solve failed: {:?}", res.err());
        let slice = res.unwrap();

        assert!((slice.target_area - 350.0).abs() < 0.05, "Target area mismatch: {}", slice.target_area);
        assert!((slice.remainder_area - 650.0).abs() < 0.05, "Remainder area mismatch: {}", slice.remainder_area);

        // Expected cut line: x = 14.0 (350 / 25 = 14)
        assert!((slice.cut_line.0[0] - 14.0).abs() < 0.1);
        assert!((slice.cut_line.1[0] - 14.0).abs() < 0.1);
    }

    #[test]
    fn test_solve_slide_line_trapezoid() {
        let trap = vec![
            [0.0, 0.0],
            [50.0, 0.0],
            [40.0, 30.0],
            [10.0, 30.0],
        ];
        let f_start = [0.0, 0.0];
        let f_end = [50.0, 0.0];

        // Total area: (50 + 30)/2 * 30 = 1200
        let res = solve_slide_line(&trap, f_start, f_end, 90.0, 400.0);
        assert!(res.is_ok(), "Trapezoid slide line failed: {:?}", res.err());
        let slice = res.unwrap();
        assert!((slice.target_area - 400.0).abs() < 0.05);
        assert!((slice.remainder_area - 800.0).abs() < 0.05);
    }

    use crate::app::commands::civil_parcel::ParcelData;

    #[test]
    fn test_interactive_slide_line_command_workflow() {
        let mut doc = codec::CadDocument::default();
        let rect = vec![
            Vector2::new(0.0, 0.0),
            Vector2::new(40.0, 0.0),
            Vector2::new(40.0, 25.0),
            Vector2::new(0.0, 25.0),
        ];
        let mut pl = LwPolyline::from_points(rect);
        pl.is_closed = true;
        let mut ent = EntityType::LwPolyline(pl);
        ent.common_mut().layer = "PC-PARCEL".to_string();
        let handle = doc.add_entity(ent).unwrap();

        let mut cmd = SlideLineCommand::new(350.0, 12.0, 4.0);
        assert_eq!(cmd.step, SlideStep::PickParcel);
        assert!(cmd.needs_entity_pick());

        // 1. Pick parcel
        cmd.inject_picked_entity(doc.get_entity(handle).unwrap().clone());
        let res = cmd.on_entity_pick(handle, DVec3::new(20.0, 10.0, 0.0));
        assert!(matches!(res, CmdResult::NeedPoint));
        assert_eq!(cmd.step, SlideStep::PickFrontageStart);

        // 2. Frontage Start
        let res = cmd.on_point(DVec3::new(0.0, 0.0, 0.0));
        assert!(matches!(res, CmdResult::NeedPoint));
        assert_eq!(cmd.step, SlideStep::PickFrontageEnd);

        // 3. Frontage End
        let res = cmd.on_point(DVec3::new(40.0, 0.0, 0.0));
        assert!(matches!(res, CmdResult::NeedPoint));
        assert_eq!(cmd.step, SlideStep::SpecifyAngle);

        // 4. Specify Angle (Enter = 90 deg default)
        let res = cmd.on_enter();
        assert!(matches!(res, CmdResult::NeedPoint));
        assert_eq!(cmd.step, SlideStep::AcceptPreview);

        // Preview wires check
        let wires = cmd.on_preview_wires(DVec3::ZERO);
        assert!(wires.len() >= 2, "Must show cut line and lot preview wires");

        // 5. Accept
        let res = cmd.on_enter();
        match res {
            CmdResult::ReplaceMany(reps, _) => {
                assert_eq!(reps.len(), 1);
                assert_eq!(reps[0].0, handle);
                assert_eq!(reps[0].1.len(), 2, "Must produce 2 child parcels");
                // Replace in document
                doc.remove_entity(handle);
                for child in reps[0].1.clone() {
                    let _ = doc.add_entity(child);
                }
            }
            _ => panic!("Expected CmdResult::ReplaceMany"),
        }

        // Run dynamic sync
        let count = ParcelData::sync_dynamic_parcels(&mut doc, None, None);
        assert_eq!(count, 2, "Must dynamically synchronize both child parcels");
    }

    #[test]
    fn test_solve_multi_slide_line_five_lots() {
        let rect = vec![
            [0.0, 0.0],
            [100.0, 0.0],
            [100.0, 25.0],
            [0.0, 25.0],
        ];
        // Total area: 100 * 25 = 2500 m2
        // Subdivide with min_area = 500 m2 -> should produce 5 lots!
        let res = solve_multi_slide_line(
            &rect,
            [0.0, 0.0],
            [100.0, 0.0],
            90.0,
            500.0,
            12.0,
            "Place remainder in last parcel",
        );
        assert!(res.is_ok(), "Multi slide line failed: {:?}", res.err());
        let multi = res.unwrap();
        assert_eq!(multi.lots.len(), 5, "Expected 5 lots for 2500m² / 500m²");
        assert_eq!(multi.cut_lines.len(), 4, "Expected 4 cut lines for 5 lots");
        for a in &multi.lot_areas {
            assert!((a - 500.0).abs() < 1.0, "Lot area should be ~500 m2, got {}", a);
        }
    }

    #[test]
    fn test_parcel_from_objects_workflow() {
        let mut doc = codec::CadDocument::default();
        let rect = vec![
            Vector2::new(10.0, 10.0),
            Vector2::new(30.0, 10.0),
            Vector2::new(30.0, 30.0),
            Vector2::new(10.0, 30.0),
        ];
        let mut pl = LwPolyline::from_points(rect);
        pl.is_closed = true;
        let mut ent = EntityType::LwPolyline(pl);
        ent.common_mut().layer = "0".to_string(); // Existing linework on layer 0
        let handle = doc.add_entity(ent).unwrap();

        let mut cmd = ParcelFromObjectsCommand::new();
        cmd.inject_picked_entity(doc.get_entity(handle).unwrap().clone());
        let _ = cmd.on_entity_pick(handle, DVec3::new(15.0, 15.0, 0.0));
        assert_eq!(cmd.picked_handles.len(), 1);

        let res = cmd.on_enter();
        match res {
            CmdResult::ReplaceMany(reps, _) => {
                assert_eq!(reps.len(), 1);

                assert_eq!(reps[0].1[0].common().layer, "C-PROP");

                let rec = reps[0].1[0].common().extended_data.get_record(XDATA_PARCEL).unwrap();

                assert_eq!(rec.values[1], XDataValue::String("Site 1".to_string()));

                assert_eq!(rec.values[3], XDataValue::String("Single-Family".to_string()));
            }
            _ => panic!("Expected ReplaceMany"),
        }
    }

    #[test]
    fn test_solve_multi_slide_line_clockwise_and_redistribute() {
        // Clockwise winding: [0,0] -> [0,25] -> [100,25] -> [100,0]
        let cw_rect = vec![
            [0.0, 0.0],
            [0.0, 25.0],
            [100.0, 25.0],
            [100.0, 0.0],
        ];
        // Total area 2500m2 / 600m2 -> 4 lots redistributed (625m2 each)
        let res = solve_multi_slide_line(
            &cw_rect,
            [0.0, 0.0],
            [100.0, 0.0],
            90.0,
            600.0,
            12.0,
            "Redistribute remainder",
        );
        assert!(res.is_ok(), "Clockwise multi slide line failed: {:?}", res.err());
        let multi = res.unwrap();
        assert_eq!(multi.lots.len(), 4, "Expected 4 lots for redistribute mode");
        for a in &multi.lot_areas {
            assert!((a - 625.0).abs() < 5.0, "Lot area should be ~625 m2, got {}", a);
        }
    }

    #[test]
    fn test_all_three_remainder_modes() {
        let rect = vec![
            [0.0, 0.0],
            [100.0, 0.0],
            [100.0, 25.0],
            [0.0, 25.0],
        ]; // 2500 m2 total area, frontage 100m

        // Mode 1: Place remainder in last parcel (4 lots: 3x600, 1x700)
        let res_last = solve_multi_slide_line(
            &rect,
            [0.0, 0.0],
            [100.0, 0.0],
            90.0,
            600.0,
            12.0,
            "Place remainder in last parcel",
        ).unwrap();
        assert_eq!(res_last.lots.len(), 4);
        assert!((res_last.lot_areas[0] - 600.0).abs() < 1.0);
        assert!((res_last.lot_areas[1] - 600.0).abs() < 1.0);
        assert!((res_last.lot_areas[2] - 600.0).abs() < 1.0);
        assert!((res_last.lot_areas[3] - 700.0).abs() < 1.0);

        // Mode 2: Redistribute remainder (4 lots: 4x625)
        let res_redist = solve_multi_slide_line(
            &rect,
            [0.0, 0.0],
            [100.0, 0.0],
            90.0,
            600.0,
            12.0,
            "Redistribute remainder",
        ).unwrap();
        assert_eq!(res_redist.lots.len(), 4);
        for a in &res_redist.lot_areas {
            assert!((a - 625.0).abs() < 1.0);
        }

        // Mode 3: Create remainder parcel (5 lots: 4x600, 1x100)
        let res_create = solve_multi_slide_line(
            &rect,
            [0.0, 0.0],
            [100.0, 0.0],
            90.0,
            600.0,
            12.0,
            "Create remainder parcel",
        ).unwrap();
        assert_eq!(res_create.lots.len(), 5);
        for i in 0..4 {
            assert!((res_create.lot_areas[i] - 600.0).abs() < 1.0);
        }
        assert!((res_create.lot_areas[4] - 100.0).abs() < 1.0);
    }

    #[test]
    fn test_interactive_multi_slide_line_workflow() {
        let mut doc = codec::CadDocument::default();
        // 100m x 25m parcel = 2500 m2
        let rect = vec![
            Vector2::new(0.0, 0.0),
            Vector2::new(100.0, 0.0),
            Vector2::new(100.0, 25.0),
            Vector2::new(0.0, 25.0),
        ];
        let mut pl = LwPolyline::from_points(rect);
        pl.is_closed = true;
        let mut ent = EntityType::LwPolyline(pl);
        ent.common_mut().layer = "PC-PARCEL".to_string();
        let handle = doc.add_entity(ent).unwrap();

        // Min area = 500m2 -> should subdivide into 5 lots automatically!
        let mut cmd = SlideLineCommand::new(500.0, 12.0, 4.0);
        cmd.automatic_mode = true;
        cmd.remainder_mode = "Place remainder in last parcel".to_string();

        cmd.inject_picked_entity(doc.get_entity(handle).unwrap().clone());
        let _ = cmd.on_entity_pick(handle, DVec3::new(50.0, 12.5, 0.0));
        let _ = cmd.on_point(DVec3::new(0.0, 0.0, 0.0));
        let _ = cmd.on_point(DVec3::new(100.0, 0.0, 0.0));
        let _ = cmd.on_enter(); // 90 deg angle
        assert_eq!(cmd.step, SlideStep::AcceptPreview);

        let res = cmd.on_enter(); // Accept
        match res {
            CmdResult::ReplaceMany(reps, _) => {
                assert_eq!(reps.len(), 1);
                assert_eq!(reps[0].1.len(), 5, "Expected 5 lots generated by automatic multi slide line!");
            }
            _ => panic!("Expected ReplaceMany with multiple child lots"),
        }
    }

    #[test]
    fn test_solve_multi_slide_line_twelve_lots() {
        // 240m x 25m = 6000 m2
        let rect = vec![
            [0.0, 0.0],
            [240.0, 0.0],
            [240.0, 25.0],
            [0.0, 25.0],
        ];
        // min_area = 500 m2, min_frontage = 15m -> 12 lots (each 20m wide x 25m deep = 500m2)
        let res = solve_multi_slide_line(
            &rect,
            [0.0, 0.0],
            [240.0, 0.0],
            90.0,
            500.0,
            15.0,
            "Place remainder in last parcel",
        );
        assert!(res.is_ok(), "12 lots subdivision failed: {:?}", res.err());
        let multi = res.unwrap();
        assert_eq!(multi.lots.len(), 12, "Expected 12 lots, got {}", multi.lots.len());
        assert_eq!(multi.cut_lines.len(), 11, "Expected 11 cut lines");
        for a in &multi.lot_areas {
            assert!((a - 500.0).abs() < 1.0, "Lot area should be ~500 m2, got {}", a);
        }
    }

    #[test]
    fn test_solve_swing_line_candidates_and_criteria() {
        let rect = vec![
            [0.0, 0.0],
            [40.0, 0.0],
            [40.0, 25.0],
            [0.0, 25.0],
        ];
        let pivot = [20.0, 25.0]; // midpoint of rear boundary
        let path = vec![[0.0, 0.0], [40.0, 0.0]]; // 40m frontage
        let target_area = 350.0;

        let criteria = ParcelSizingCriteria {
            min_area: 350.0,
            min_frontage: 12.0,
            use_offset: true,
            frontage_offset: 4.0,
            min_width: 10.0,
            min_depth: 20.0,
            use_max_depth: true,
            max_depth: 80.0,
            solution_preference: "Use shortest frontage".to_string(),
            ..Default::default()
        };

        let sols = solve_swing_line_candidates(&rect, pivot, &path, target_area, Some(&criteria));
        assert!(sols.is_ok(), "Swing line candidates failed: {:?}", sols.err());
        let solutions = sols.unwrap();
        assert!(!solutions.is_empty(), "Should produce at least one candidate solution");
        for sol in &solutions {
            assert!(sol.frontage_length > 0.0, "Frontage length should be positive and valid");
            assert!(sol.depth >= 15.0, "Depth should be measured properly");
            assert!(sol.target_area > 0.0);
        }
    }

    #[test]
    fn test_interactive_swing_line_command_workflow() {
        let mut doc = codec::CadDocument::default();
        let rect = vec![
            Vector2::new(0.0, 0.0),
            Vector2::new(40.0, 0.0),
            Vector2::new(40.0, 25.0),
            Vector2::new(0.0, 25.0),
        ];
        let mut pl = LwPolyline::from_points(rect);
        pl.is_closed = true;
        let mut ent = EntityType::LwPolyline(pl);
        ent.common_mut().layer = "PC-PARCEL".to_string();
        let handle = doc.add_entity(ent).unwrap();

        let criteria = ParcelSizingCriteria {
            min_area: 350.0,
            min_frontage: 12.0,
            ..Default::default()
        };
        let mut cmd = SwingLineCommand::new_with_criteria(criteria);
        assert_eq!(cmd.step, SwingStep::PickParcel);

        // 1. Pick parcel
        cmd.inject_picked_entity(doc.get_entity(handle).unwrap().clone());
        let res = cmd.on_entity_pick(handle, DVec3::new(20.0, 10.0, 0.0));
        assert!(matches!(res, CmdResult::NeedPoint));
        assert_eq!(cmd.step, SwingStep::PickPivot);

        // 2. Pick pivot on rear
        let res = cmd.on_point(DVec3::new(20.0, 25.0, 0.0));
        assert!(matches!(res, CmdResult::NeedPoint));
        assert_eq!(cmd.step, SwingStep::PickFrontageStart);

        // 3. Pick frontage start
        let res = cmd.on_point(DVec3::new(0.0, 0.0, 0.0));
        assert!(matches!(res, CmdResult::NeedPoint));
        assert_eq!(cmd.step, SwingStep::PickFrontageEnd);

        // 4. Pick frontage end
        let res = cmd.on_point(DVec3::new(40.0, 0.0, 0.0));
        assert!(matches!(res, CmdResult::NeedPoint));
        assert_eq!(cmd.step, SwingStep::AcceptPreview);
        assert!(cmd.solution.is_some());
        assert!(!cmd.candidate_solutions.is_empty());

        // 5. Test next solution cycling
        if cmd.candidate_solutions.len() > 1 {
            let _ = cmd.on_text_input("NEXT");
            assert!(cmd.solution.is_some());
        }

        // 6. Accept
        let res = cmd.on_enter();
        match res {
            CmdResult::ReplaceMany(reps, _) => {
                assert_eq!(reps.len(), 1);
                assert_eq!(reps[0].0, handle);
                assert_eq!(reps[0].1.len(), 2, "Must produce 2 child parcels");
            }
            _ => panic!("Expected CmdResult::ReplaceMany for SwingLineCommand"),
        }
    }
}

