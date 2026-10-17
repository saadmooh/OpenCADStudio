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

// ── Polygon Slicing and Slide Line Solver ──────────────────────────────────

#[derive(Clone, Debug)]
pub struct SlideSlice {
    pub target_lot: Vec<[f64; 2]>,
    pub remainder_lot: Vec<[f64; 2]>,
    pub cut_line: ([f64; 2], [f64; 2]),
    pub target_area: f64,
    pub remainder_area: f64,
    pub frontage_length: f64,
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

    // Find all intersections of ray with edges other than frontage_edge
    let mut best_hit = None;
    let mut min_s = f64::INFINITY;

    for i in 0..n {
        if i == frontage_edge {
            continue;
        }
        let j = (i + 1) % n;
        if let Some((hit_pt, s, _u)) = ray_segment_intersection(origin, dir, poly[i], poly[j]) {
            if s > 1e-4 && s < min_s {
                min_s = s;
                best_hit = Some((i, hit_pt));
            }
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

    // Identify which loop contains f_start (the target lot)
    let loop1_has_fstart = loop1.iter().any(|v| {
        let dx = v[0] - f_start[0];
        let dy = v[1] - f_start[1];
        (dx * dx + dy * dy).sqrt() < 0.1
    }) || point_in_polygon(f_start, &loop1);

    let (target_lot, remainder_lot, target_area, remainder_area) = if loop1_has_fstart {
        (loop1, loop2, area1, area2)
    } else {
        (loop2, loop1, area2, area1)
    };

    let dx_f = origin[0] - f_start[0];
    let dy_f = origin[1] - f_start[1];
    let frontage_length = (dx_f * dx_f + dy_f * dy_f).sqrt();

    Some(SlideSlice {
        target_lot,
        remainder_lot,
        cut_line: (origin, exit_pt),
        target_area,
        remainder_area,
        frontage_length,
    })
}

/// Solves for the slide line position that achieves `target_area`
pub fn solve_slide_line(
    poly: &[[f64; 2]],
    f_start: [f64; 2],
    f_end: [f64; 2],
    angle_deg: f64,
    target_area: f64,
) -> Result<SlideSlice, String> {
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

    let (_, f_edge, _) = snap_point_to_polygon(f_start, poly);

    let f_vec = [f_end[0] - f_start[0], f_end[1] - f_start[1]];
    let f_len = (f_vec[0] * f_vec[0] + f_vec[1] * f_vec[1]).sqrt();
    if f_len < 1e-4 {
        return Err("Frontage length is too short".to_string());
    }

    let rad = angle_deg.to_radians();
    let cos_a = rad.cos();
    let sin_a = rad.sin();

    let u_x = f_vec[0] / f_len;
    let u_y = f_vec[1] / f_len;
    let n_x = -u_y;
    let n_y = u_x;

    let mut cut_dir = [
        u_x * cos_a + n_x * sin_a,
        u_y * cos_a + n_y * sin_a,
    ];

    // Ensure cut_dir points into the interior of the polygon
    let test_pt = [
        f_start[0] + 0.5 * f_vec[0] + cut_dir[0] * 0.1,
        f_start[1] + 0.5 * f_vec[1] + cut_dir[1] * 0.1,
    ];
    if !point_in_polygon(test_pt, poly) {
        cut_dir[0] = -cut_dir[0];
        cut_dir[1] = -cut_dir[1];
    }

    // Numerical Bisection Search
    let mut t_low = 0.001;
    let mut t_high = 0.999;
    let mut best_slice: Option<SlideSlice> = None;

    for _ in 0..50 {
        let t_mid = 0.5 * (t_low + t_high);
        let pt_on_frontage = [
            f_start[0] + t_mid * f_vec[0],
            f_start[1] + t_mid * f_vec[1],
        ];

        if let Some(slice) = slice_polygon_by_ray(poly, f_edge, pt_on_frontage, cut_dir, f_start) {
            let diff = slice.target_area - target_area;
            best_slice = Some(slice);
            if diff.abs() < 0.01 {
                break;
            }
            if diff < 0.0 {
                t_low = t_mid;
            } else {
                t_high = t_mid;
            }
        } else {
            // Adjust search bound if no slice found at t_mid
            t_low = (t_low + t_mid) * 0.5;
        }
    }

    best_slice.ok_or_else(|| "Could not find valid slide line cut for parcel".to_string())
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
    pub min_area: f64,
    pub min_frontage: f64,
    pub frontage_offset: f64,
    pub angle_deg: f64,
    pub step: SlideStep,

    pub parent_handle: Option<Handle>,
    pub parent_name: String,
    pub parent_site: String,
    pub parent_layer: String,
    pub parent_vertices: Vec<[f64; 2]>,
    pub parent_area: f64,

    pub frontage_start: Option<[f64; 2]>,
    pub frontage_end: Option<[f64; 2]>,
    pub solution: Option<SlideSlice>,
    pub picked_entity: Option<EntityType>,
}

impl SlideLineCommand {
    pub fn new(min_area: f64, min_frontage: f64, frontage_offset: f64) -> Self {
        Self {
            min_area: if min_area > 0.0 { min_area } else { 350.0 },
            min_frontage: if min_frontage > 0.0 { min_frontage } else { 12.0 },
            frontage_offset,
            angle_deg: 90.0,
            step: SlideStep::PickParcel,
            parent_handle: None,
            parent_name: "LOT".to_string(),
            parent_site: "Site 1".to_string(),
            parent_layer: "PC-PARCEL".to_string(),
            parent_vertices: Vec::new(),
            parent_area: 0.0,
            frontage_start: None,
            frontage_end: None,
            solution: None,
            picked_entity: None,
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
                crate::t!("SLIDELINE  Select end point on frontage:").into_owned()
            }
            SlideStep::SpecifyAngle => {
                format!(
                    "SLIDELINE  Specify angle relative to frontage <{:.2}> (or target area: {:.1}m²):",
                    self.angle_deg, self.min_area
                )
            }
            SlideStep::AcceptPreview => {
                if let Some(sol) = &self.solution {
                    format!(
                        "SLIDELINE  Target: {:.1}m² (Frontage: {:.1}m), Remainder: {:.1}m². Accept result? [Yes/No] <Yes>:",
                        sol.target_area, sol.frontage_length, sol.remainder_area
                    )
                } else {
                    crate::t!("SLIDELINE  Accept result? [Yes/No] <Yes>:").into_owned()
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
                vec![
                    CmdOption::new("Yes (Accept)", "Y"),
                    CmdOption::new("No (Reject)", "N"),
                ]
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
                    if lw.vertices.len() >= 3 {
                        let verts: Vec<[f64; 2]> = lw
                            .vertices
                            .iter()
                            .map(|v| [v.location.x, v.location.y])
                            .collect();
                        let area = shoelace_area(&verts);
                        if area > 1.0 {
                            self.parent_handle = Some(handle);
                            self.parent_layer = entity.common().layer.clone();
                            self.parent_vertices = verts;
                            self.parent_area = area;

                            // Extract parcel name from XData if present
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
                self.step = SlideStep::PickFrontageEnd;
                CmdResult::NeedPoint
            }
            SlideStep::PickFrontageEnd => {
                let (snapped, _, _) = snap_point_to_polygon([pt.x, pt.y], &self.parent_vertices);
                if let Some(f_start) = self.frontage_start {
                    let dx = snapped[0] - f_start[0];
                    let dy = snapped[1] - f_start[1];
                    if (dx * dx + dy * dy).sqrt() > 0.1 {
                        self.frontage_end = Some(snapped);
                        self.step = SlideStep::SpecifyAngle;
                        return CmdResult::NeedPoint;
                    }
                }
                CmdResult::NeedPoint
            }
            SlideStep::SpecifyAngle => {
                // If user clicks a point for angle, compute angle from frontage
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

                    match solve_slide_line(
                        &self.parent_vertices,
                        f_start,
                        f_end,
                        self.angle_deg,
                        self.min_area,
                    ) {
                        Ok(sol) => {
                            self.solution = Some(sol);
                            self.step = SlideStep::AcceptPreview;
                            return CmdResult::NeedPoint;
                        }
                        Err(e) => {
                            return CmdResult::ReportError(format!("Slide Line failed: {}", e));
                        }
                    }
                }
                CmdResult::NeedPoint
            }
            SlideStep::AcceptPreview => {
                // Click accepts preview!
                self.commit_subdivision()
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
                    // Allows entering target area
                    return Some(CmdResult::NeedPoint);
                }
                if let Ok(ang) = t.parse::<f64>() {
                    if ang > 1.0 && ang < 179.0 {
                        self.angle_deg = ang;
                    }
                }
                if let (Some(f_start), Some(f_end)) = (self.frontage_start, self.frontage_end) {
                    match solve_slide_line(
                        &self.parent_vertices,
                        f_start,
                        f_end,
                        self.angle_deg,
                        self.min_area,
                    ) {
                        Ok(sol) => {
                            self.solution = Some(sol);
                            self.step = SlideStep::AcceptPreview;
                            Some(CmdResult::NeedPoint)
                        }
                        Err(e) => Some(CmdResult::ReportError(format!("Slide Line failed: {}", e))),
                    }
                } else {
                    Some(CmdResult::NeedPoint)
                }
            }
            SlideStep::AcceptPreview => {
                let up = t.to_ascii_uppercase();
                if up == "Y" || up == "YES" || up.is_empty() {
                    Some(self.commit_subdivision())
                } else if up == "N" || up == "NO" {
                    Some(CmdResult::Cancel)
                } else {
                    Some(CmdResult::NeedPoint)
                }
            }
            _ => None,
        }
    }

    fn on_enter(&mut self) -> CmdResult {
        match self.step {
            SlideStep::SpecifyAngle => {
                // Default 90 degrees
                if let (Some(f_start), Some(f_end)) = (self.frontage_start, self.frontage_end) {
                    match solve_slide_line(
                        &self.parent_vertices,
                        f_start,
                        f_end,
                        self.angle_deg,
                        self.min_area,
                    ) {
                        Ok(sol) => {
                            self.solution = Some(sol);
                            self.step = SlideStep::AcceptPreview;
                            CmdResult::NeedPoint
                        }
                        Err(e) => CmdResult::ReportError(format!("Slide Line failed: {}", e)),
                    }
                } else {
                    CmdResult::NeedPoint
                }
            }
            SlideStep::AcceptPreview => self.commit_subdivision(),
            _ => CmdResult::Cancel,
        }
    }

    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }

    fn on_preview_wires(&mut self, _pt: DVec3) -> Vec<WireModel> {
        let mut wires = Vec::new();
        if let Some(sol) = &self.solution {
            // 1. Cut line in bright cyan
            let p1 = [sol.cut_line.0[0], sol.cut_line.0[1], 0.0];
            let p2 = [sol.cut_line.1[0], sol.cut_line.1[1], 0.0];
            wires.push(WireModel::solid_f64(
                "preview_cut_line".to_string(),
                vec![p1, p2],
                [0.1, 0.9, 1.0, 1.0],
                false,
            ));

            // 2. Target lot boundary in bright golden-orange
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
        } else if let Some(f_start) = self.frontage_start {
            // Preview frontage starting point
            let p = [f_start[0], f_start[1], 0.0];
            wires.push(WireModel::solid_f64(
                "preview_fstart".to_string(),
                vec![p, [p[0] + 0.5, p[1] + 0.5, 0.0]],
                [0.2, 1.0, 0.3, 1.0],
                false,
            ));
        }
        wires
    }
}

impl SlideLineCommand {
    fn commit_subdivision(&self) -> CmdResult {
        let Some(parent_handle) = self.parent_handle else {
            return CmdResult::Cancel;
        };
        let Some(sol) = &self.solution else {
            return CmdResult::Cancel;
        };

        let target_name = format!("{}_A", self.parent_name);
        let rem_name = format!("{}_B", self.parent_name);

        // 1. Build Target Lot Polyline Entity
        let mut pl_a = LwPolyline::from_points(
            sol.target_lot
                .iter()
                .map(|v| Vector2::new(v[0], v[1]))
                .collect(),
        );
        pl_a.is_closed = true;
        let mut ent_a = EntityType::LwPolyline(pl_a);
        ent_a.common_mut().layer = "PC-PARCEL".to_string();

        let mut rec_a = ExtendedDataRecord::new(XDATA_PARCEL);
        rec_a.add_value(XDataValue::String(target_name.clone()));
        rec_a.add_value(XDataValue::String(self.parent_site.clone()));
        rec_a.add_value(XDataValue::String("101".to_string()));
        rec_a.add_value(XDataValue::String("Single-Family Residential (R-1)".to_string()));
        ent_a.common_mut().extended_data.add_record(rec_a);

        // 2. Build Remainder Lot Polyline Entity
        let mut pl_b = LwPolyline::from_points(
            sol.remainder_lot
                .iter()
                .map(|v| Vector2::new(v[0], v[1]))
                .collect(),
        );
        pl_b.is_closed = true;
        let mut ent_b = EntityType::LwPolyline(pl_b);
        ent_b.common_mut().layer = "PC-PARCEL".to_string();

        let mut rec_b = ExtendedDataRecord::new(XDATA_PARCEL);
        rec_b.add_value(XDataValue::String(rem_name.clone()));
        rec_b.add_value(XDataValue::String(self.parent_site.clone()));
        rec_b.add_value(XDataValue::String("102".to_string()));
        rec_b.add_value(XDataValue::String("Single-Family Residential (R-1)".to_string()));
        ent_b.common_mut().extended_data.add_record(rec_b);

        // Replace the parent parcel with the two child parcels!
        CmdResult::ReplaceMany(
            vec![(parent_handle, vec![ent_a, ent_b])],
            Vec::new(),
        )
    }
}

// ── Interactive Create Parcel from Objects Command ────────────────────────

pub struct ParcelFromObjectsCommand {
    pub picked_handles: Vec<Handle>,
    pub picked_entities: Vec<EntityType>,
    pub picked_entity: Option<EntityType>,
}

impl ParcelFromObjectsCommand {
    pub fn new() -> Self {
        Self {
            picked_handles: Vec::new(),
            picked_entities: Vec::new(),
            picked_entity: None,
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
            parcel_ent.common_mut().layer = "PC-PARCEL".to_string();

            let lot_name = format!("LOT-{}", 101 + i);
            let mut rec = ExtendedDataRecord::new(XDATA_PARCEL);
            rec.add_value(XDataValue::String(lot_name));
            rec.add_value(XDataValue::String("Site 1".to_string()));
            rec.add_value(XDataValue::String((101 + i).to_string()));
            rec.add_value(XDataValue::String("Single-Family Residential (R-1)".to_string()));
            parcel_ent.common_mut().extended_data.add_record(rec);

            replacements.push((*handle, vec![parcel_ent]));
        }

        CmdResult::ReplaceMany(replacements, Vec::new())
    }

    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

// ── Unit Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
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
        assert_eq!(wires.len(), 2, "Must show cut line and target lot preview wires");

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
                assert_eq!(reps[0].1[0].common().layer, "PC-PARCEL");
            }
            _ => panic!("Expected ReplaceMany"),
        }
    }
}
