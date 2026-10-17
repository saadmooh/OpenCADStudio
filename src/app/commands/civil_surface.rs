use codec::entities::{Line, PolyfaceFace, PolyfaceMesh, PolyfaceVertex, Polyline3D};
use codec::tables::Layer;
use codec::types::{Color, Vector3};
use codec::{CadDocument, EntityType, Handle};
use crate::command::{CadCommand, CmdOption, CmdResult};
use crate::scene::model::wire_model::WireModel;
use glam::DVec3;

#[derive(Debug, Clone)]
pub struct TinTriangle {
    pub v0: [f64; 3],
    pub v1: [f64; 3],
    pub v2: [f64; 3],
}

impl TinTriangle {
    #[inline]
    pub fn area_2d(&self) -> f64 {
        0.5 * ((self.v1[0] - self.v0[0]) * (self.v2[1] - self.v0[1])
            - (self.v2[0] - self.v0[0]) * (self.v1[1] - self.v0[1]))
            .abs()
    }

    #[inline]
    pub fn area_3d(&self) -> f64 {
        let ax = self.v1[0] - self.v0[0];
        let ay = self.v1[1] - self.v0[1];
        let az = self.v1[2] - self.v0[2];
        let bx = self.v2[0] - self.v0[0];
        let by = self.v2[1] - self.v0[1];
        let bz = self.v2[2] - self.v0[2];

        let nx = ay * bz - az * by;
        let ny = az * bx - ax * bz;
        let nz = ax * by - ay * bx;

        0.5 * (nx * nx + ny * ny + nz * nz).sqrt()
    }

    #[inline]
    #[allow(dead_code)]
    pub fn contains_point_2d(&self, px: f64, py: f64) -> bool {
        let x0 = self.v0[0];
        let y0 = self.v0[1];
        let x1 = self.v1[0];
        let y1 = self.v1[1];
        let x2 = self.v2[0];
        let y2 = self.v2[1];

        let denom = (y1 - y2) * (x0 - x2) + (x2 - x1) * (y0 - y2);
        if denom.abs() < 1e-12 {
            return false;
        }

        let w0 = ((y1 - y2) * (px - x2) + (x2 - x1) * (py - y2)) / denom;
        let w1 = ((y2 - y0) * (px - x2) + (x0 - x2) * (py - y2)) / denom;
        let w2 = 1.0 - w0 - w1;

        w0 >= -1e-6 && w1 >= -1e-6 && w2 >= -1e-6
    }

    #[inline]
    pub fn interpolate_z_2d(&self, px: f64, py: f64) -> Option<f64> {
        let x0 = self.v0[0];
        let y0 = self.v0[1];
        let x1 = self.v1[0];
        let y1 = self.v1[1];
        let x2 = self.v2[0];
        let y2 = self.v2[1];

        let denom = (y1 - y2) * (x0 - x2) + (x2 - x1) * (y0 - y2);
        if denom.abs() < 1e-12 {
            return None;
        }

        let w0 = ((y1 - y2) * (px - x2) + (x2 - x1) * (py - y2)) / denom;
        let w1 = ((y2 - y0) * (px - x2) + (x0 - x2) * (py - y2)) / denom;
        let w2 = 1.0 - w0 - w1;

        if w0 >= -1e-6 && w1 >= -1e-6 && w2 >= -1e-6 {
            Some(w0 * self.v0[2] + w1 * self.v1[2] + w2 * self.v2[2])
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct TinSurfaceData {
    pub handle: Handle,
    pub name: String,
    pub layer: String,
    pub triangles: Vec<TinTriangle>,
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
    pub min_z: f64,
    pub max_z: f64,
    pub mean_z: f64,
    pub area_2d: f64,
    pub area_3d: f64,
}

impl TinSurfaceData {
    pub fn from_document_entity(document: &CadDocument, target_handle: Handle) -> Option<Self> {
        let entity = document.entities().find(|e| e.common().handle == target_handle)?;
        Self::from_entity(entity)
    }

    pub fn from_entity(entity: &EntityType) -> Option<Self> {
        let handle = entity.common().handle;
        let layer = entity.common().layer.clone();
        let name = if let Some(stripped) = layer.strip_prefix("LS-TIN-") {
            stripped.to_string()
        } else if let Some(stripped) = layer.strip_prefix("LS-GRADE-") {
            format!("Grading: {}", stripped)
        } else {
            layer.clone()
        };

        let mut triangles = Vec::new();
        match entity {
            EntityType::PolyfaceMesh(mesh) => {
                let vertices: Vec<[f64; 3]> = mesh
                    .vertices
                    .iter()
                    .map(|v| [v.location.x, v.location.y, v.location.z])
                    .collect();

                for face in &mesh.faces {
                    let raw = [face.index1, face.index2, face.index3, face.index4];
                    let indices: Vec<usize> = raw
                        .iter()
                        .filter(|&&i| i != 0)
                        .filter_map(|&i| (i.unsigned_abs() as usize).checked_sub(1))
                        .filter(|&i| i < vertices.len())
                        .collect();

                    if indices.len() == 3 {
                        triangles.push(TinTriangle {
                            v0: vertices[indices[0]],
                            v1: vertices[indices[1]],
                            v2: vertices[indices[2]],
                        });
                    } else if indices.len() >= 4 {
                        triangles.push(TinTriangle {
                            v0: vertices[indices[0]],
                            v1: vertices[indices[1]],
                            v2: vertices[indices[2]],
                        });
                        triangles.push(TinTriangle {
                            v0: vertices[indices[0]],
                            v1: vertices[indices[2]],
                            v2: vertices[indices[3]],
                        });
                    }
                }
            }
            EntityType::Mesh(mesh) => {
                let vertices: Vec<[f64; 3]> = mesh
                    .vertices
                    .iter()
                    .map(|v| [v.x, v.y, v.z])
                    .collect();

                for face in &mesh.faces {
                    if face.vertices.len() == 3 {
                        if face.vertices[0] < vertices.len() && face.vertices[1] < vertices.len() && face.vertices[2] < vertices.len() {
                            triangles.push(TinTriangle {
                                v0: vertices[face.vertices[0]],
                                v1: vertices[face.vertices[1]],
                                v2: vertices[face.vertices[2]],
                            });
                        }
                    } else if face.vertices.len() >= 4 {
                        if face.vertices[0] < vertices.len() && face.vertices[1] < vertices.len() && face.vertices[2] < vertices.len() && face.vertices[3] < vertices.len() {
                            triangles.push(TinTriangle {
                                v0: vertices[face.vertices[0]],
                                v1: vertices[face.vertices[1]],
                                v2: vertices[face.vertices[2]],
                            });
                            triangles.push(TinTriangle {
                                v0: vertices[face.vertices[0]],
                                v1: vertices[face.vertices[2]],
                                v2: vertices[face.vertices[3]],
                            });
                        }
                    }
                }
            }
            _ => return None,
        }

        if triangles.is_empty() {
            return None;
        }

        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        let mut min_z = f64::INFINITY;
        let mut max_z = f64::NEG_INFINITY;
        let mut z_sum = 0.0;
        let mut pt_count = 0usize;
        let mut area_2d = 0.0;
        let mut area_3d = 0.0;

        for tri in &triangles {
            area_2d += tri.area_2d();
            area_3d += tri.area_3d();

            for pt in [&tri.v0, &tri.v1, &tri.v2] {
                min_x = min_x.min(pt[0]);
                max_x = max_x.max(pt[0]);
                min_y = min_y.min(pt[1]);
                max_y = max_y.max(pt[1]);
                min_z = min_z.min(pt[2]);
                max_z = max_z.max(pt[2]);
                z_sum += pt[2];
                pt_count += 1;
            }
        }

        let mean_z = if pt_count > 0 { z_sum / (pt_count as f64) } else { 0.0 };

        Some(TinSurfaceData {
            handle,
            name,
            layer,
            triangles,
            min_x,
            max_x,
            min_y,
            max_y,
            min_z,
            max_z,
            mean_z,
            area_2d,
            area_3d,
        })
    }

    /// Elevation at arbitrary (x, y) coordinates via point-in-triangle test.
    pub fn elevation_at(&self, x: f64, y: f64) -> Option<f64> {
        if x < self.min_x || x > self.max_x || y < self.min_y || y > self.max_y {
            return None;
        }
        for tri in &self.triangles {
            if let Some(z) = tri.interpolate_z_2d(x, y) {
                return Some(z);
            }
        }
        None
    }
}

/// Statistics and formatted report for a surface.
pub fn format_surface_statistics(surf: &TinSurfaceData) -> String {
    let hectares = surf.area_2d / 10_000.0;
    let acres = surf.area_2d * 0.000247105;
    format!(
        "═══════════════════════════════════════════════════\n\
         📊 Surface Properties: {} [{:X}]\n\
         ═══════════════════════════════════════════════════\n\
         • Layer: {}\n\
         • Triangles: {}\n\
         • Minimum Elevation: {:.3} m\n\
         • Maximum Elevation: {:.3} m\n\
         • Elevation Range: {:.3} m\n\
         • Mean Elevation: {:.3} m\n\
         • 2D Projected Area: {:.2} m² ({:.3} ha / {:.3} acres)\n\
         • 3D Surface Area: {:.2} m²\n\
         • Area Ratio (3D / 2D): {:.4}\n\
         ═══════════════════════════════════════════════════",
        surf.name,
        surf.handle.value(),
        surf.layer,
        surf.triangles.len(),
        surf.min_z,
        surf.max_z,
        surf.max_z - surf.min_z,
        surf.mean_z,
        surf.area_2d,
        hectares,
        acres,
        surf.area_3d,
        if surf.area_2d > 0.0 { surf.area_3d / surf.area_2d } else { 1.0 }
    )
}

/// Generate 3D contour lines intersecting triangles at given interval (e.g. 1.0m).
#[allow(dead_code)]
pub fn generate_contours(
    surf: &TinSurfaceData,
    interval: f64,
) -> (Vec<Line>, Vec<Line>) {
    let interval = if interval <= 0.0 { 1.0 } else { interval };
    let mut major_lines = Vec::new();
    let mut minor_lines = Vec::new();

    let start_z = (surf.min_z / interval).ceil() * interval;
    let end_z = (surf.max_z / interval).floor() * interval;

    let mut z_cur = start_z;
    while z_cur <= end_z + 1e-6 {
        let is_major = ((z_cur / interval).round() as i64).abs() % 5 == 0;
        let layer_name = if is_major { "LS-CONTOUR-MAJOR" } else { "LS-CONTOUR-MINOR" };

        for tri in &surf.triangles {
            let t_min = tri.v0[2].min(tri.v1[2]).min(tri.v2[2]);
            let t_max = tri.v0[2].max(tri.v1[2]).max(tri.v2[2]);

            if z_cur < t_min || z_cur > t_max || (t_max - t_min).abs() < 1e-12 {
                continue;
            }

            // Test 3 edges
            let edges = [
                (tri.v0, tri.v1),
                (tri.v1, tri.v2),
                (tri.v2, tri.v0),
            ];

            let mut intersections = Vec::with_capacity(2);
            for &(p0, p1) in &edges {
                let z0 = p0[2];
                let z1 = p1[2];

                if (z0 <= z_cur && z1 >= z_cur) || (z1 <= z_cur && z0 >= z_cur) {
                    let dz = z1 - z0;
                    if dz.abs() > 1e-12 {
                        let t = (z_cur - z0) / dz;
                        if (0.0..=1.0).contains(&t) {
                            let ix = p0[0] + t * (p1[0] - p0[0]);
                            let iy = p0[1] + t * (p1[1] - p0[1]);
                            intersections.push([ix, iy, z_cur]);
                        }
                    }
                }
            }

            // If we have at least 2 distinct intersection points
            if intersections.len() >= 2 {
                let p1 = intersections[0];
                let mut p2 = intersections[1];
                let d = (p1[0] - p2[0]).hypot(p1[1] - p2[1]);
                if d < 1e-6 && intersections.len() > 2 {
                    p2 = intersections[2];
                }
                let d = (p1[0] - p2[0]).hypot(p1[1] - p2[1]);
                if d >= 1e-6 {
                    let mut line = Line::new();
                    line.start = Vector3::new(p1[0], p1[1], p1[2]);
                    line.end = Vector3::new(p2[0], p2[1], p2[2]);
                    line.common.layer = layer_name.to_string();
                    line.common.color = Color::ByLayer;

                    if is_major {
                        major_lines.push(line);
                    } else {
                        minor_lines.push(line);
                    }
                }
            }
        }

        z_cur += interval;
    }

    (major_lines, minor_lines)
}

/// Earthwork Cut & Fill Volume Results
#[derive(Debug, Clone)]
pub struct VolumeResult {
    pub base_name: String,
    pub comp_name: String,
    pub cut_volume: f64,
    pub fill_volume: f64,
    pub net_volume: f64,
    pub overlap_area: f64,
    pub samples_count: usize,
}

/// Calculate cut and fill earthwork volume between two surfaces using regular grid prism integration.
pub fn calculate_volumes(
    base: &TinSurfaceData,
    comp: &TinSurfaceData,
) -> Option<VolumeResult> {
    let ov_min_x = base.min_x.max(comp.min_x);
    let ov_max_x = base.max_x.min(comp.max_x);
    let ov_min_y = base.min_y.max(comp.min_y);
    let ov_max_y = base.max_y.min(comp.max_y);

    if ov_min_x >= ov_max_x || ov_min_y >= ov_max_y {
        return None;
    }

    let dx = ov_max_x - ov_min_x;
    let dy = ov_max_y - ov_min_y;

    // Grid step target ~150 divisions per axis for fast (<50ms) and accurate calculation
    let max_dim = dx.max(dy);
    let step = (max_dim / 150.0).clamp(0.25, 2.0);
    let cell_area = step * step;

    let mut cut_vol = 0.0;
    let mut fill_vol = 0.0;
    let mut overlap_area = 0.0;
    let mut samples = 0usize;

    let mut y = ov_min_y + step * 0.5;
    while y <= ov_max_y {
        let mut x = ov_min_x + step * 0.5;
        while x <= ov_max_x {
            if let (Some(zb), Some(zc)) = (base.elevation_at(x, y), comp.elevation_at(x, y)) {
                let dz = zc - zb;
                let dv = cell_area * dz.abs();
                if dz > 1e-4 {
                    fill_vol += dv;
                } else if dz < -1e-4 {
                    cut_vol += dv;
                }
                overlap_area += cell_area;
                samples += 1;
            }
            x += step;
        }
        y += step;
    }

    if samples == 0 {
        return None;
    }

    Some(VolumeResult {
        base_name: base.name.clone(),
        comp_name: comp.name.clone(),
        cut_volume: cut_vol,
        fill_volume: fill_vol,
        net_volume: fill_vol - cut_vol,
        overlap_area,
        samples_count: samples,
    })
}

/// Format volume calculation report string.
pub fn format_volume_report(res: &VolumeResult) -> String {
    let net_desc = if res.net_volume >= 0.0 {
        format!("+{:.2} m³ (Fill Excess / زيادة ردم)", res.net_volume)
    } else {
        format!("{:.2} m³ (Cut Excess / زيادة حفر)", res.net_volume)
    };

    format!(
        "═══════════════════════════════════════════════════\n\
         ⚖ Earthwork Volume Calculation (حساب كميات الحفر والردم)\n\
         ═══════════════════════════════════════════════════\n\
         • Base Surface (الأرض الطبيعية): {}\n\
         • Comparison Surface (السطح التصميمي): {}\n\
         ───────────────────────────────────────────────────\n\
         • Cut Volume (حجم الحفر):     {:.2} m³\n\
         • Fill Volume (حجم الردم):    {:.2} m³\n\
         • Net Volume (صافي الكمية):   {}\n\
         • Overlap Area (مساحة التداخل): {:.2} m²\n\
         • Grid Sampling Resolution:    {} prisms\n\
         ═══════════════════════════════════════════════════",
        res.base_name,
        res.comp_name,
        res.cut_volume,
        res.fill_volume,
        net_desc,
        res.overlap_area,
        res.samples_count
    )
}

/// Export a TIN surface to LandXML 1.2 format.
pub fn export_to_landxml(surf: &TinSurfaceData) -> String {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<LandXML xmlns=\"http://www.landxml.org/schema/LandXML-1.2\" version=\"1.2\" date=\"2026-10-16\">\n");
    xml.push_str("  <Units>\n");
    xml.push_str("    <Metric areaUnit=\"squareMeter\" linearUnit=\"meter\" volumeUnit=\"cubicMeter\" temperatureUnit=\"celsius\" />\n");
    xml.push_str("  </Units>\n");
    xml.push_str("  <Surfaces>\n");
    xml.push_str(&format!("    <Surface name=\"{}\">\n", surf.name));
    xml.push_str("      <Definition surfType=\"TIN\">\n");
    xml.push_str("        <Pnts>\n");

    let mut points: Vec<[f64; 3]> = Vec::new();
    let mut pnt_map = std::collections::HashMap::new();

    let mut pt_id = 1usize;
    for tri in &surf.triangles {
        for pt in [&tri.v0, &tri.v1, &tri.v2] {
            let key = (
                (pt[0] * 1000.0).round() as i64,
                (pt[1] * 1000.0).round() as i64,
                (pt[2] * 1000.0).round() as i64,
            );
            if let std::collections::hash_map::Entry::Vacant(e) = pnt_map.entry(key) {
                e.insert(pt_id);
                points.push(*pt);
                xml.push_str(&format!("          <P id=\"{}\">{:.4} {:.4} {:.4}</P>\n", pt_id, pt[1], pt[0], pt[2])); // Northing Easting Elev
                pt_id += 1;
            }
        }
    }
    xml.push_str("        </Pnts>\n");
    xml.push_str("        <Faces>\n");

    for tri in &surf.triangles {
        let k0 = ((tri.v0[0] * 1000.0).round() as i64, (tri.v0[1] * 1000.0).round() as i64, (tri.v0[2] * 1000.0).round() as i64);
        let k1 = ((tri.v1[0] * 1000.0).round() as i64, (tri.v1[1] * 1000.0).round() as i64, (tri.v1[2] * 1000.0).round() as i64);
        let k2 = ((tri.v2[0] * 1000.0).round() as i64, (tri.v2[1] * 1000.0).round() as i64, (tri.v2[2] * 1000.0).round() as i64);

        if let (Some(&id0), Some(&id1), Some(&id2)) = (pnt_map.get(&k0), pnt_map.get(&k1), pnt_map.get(&k2)) {
            xml.push_str(&format!("          <F>{} {} {}</F>\n", id0, id1, id2));
        }
    }

    xml.push_str("        </Faces>\n");
    xml.push_str("      </Definition>\n");
    xml.push_str("    </Surface>\n");
    xml.push_str("  </Surfaces>\n");
    xml.push_str("</LandXML>\n");

    xml
}

/// Helper to ensure layer exists with designated color.
pub fn ensure_layer_with_color(document: &mut CadDocument, name: &str, color_index: i16) {
    if document.layers.get(name).is_none() {
        let mut layer = Layer::new(name);
        layer.handle = document.allocate_handle();
        layer.color = Color::from_index(color_index);
        let _ = document.layers.add(layer);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2D Delaunay Triangulation (Bowyer-Watson Algorithm)
// ─────────────────────────────────────────────────────────────────────────────

fn circumcircle_2d(p0: [f64; 2], p1: [f64; 2], p2: [f64; 2]) -> Option<([f64; 2], f64)> {
    let d = 2.0 * (p0[0] * (p1[1] - p2[1]) + p1[0] * (p2[1] - p0[1]) + p2[0] * (p0[1] - p1[1]));
    if d.abs() < 1e-12 {
        return None;
    }
    let p0_sq = p0[0] * p0[0] + p0[1] * p0[1];
    let p1_sq = p1[0] * p1[0] + p1[1] * p1[1];
    let p2_sq = p2[0] * p2[0] + p2[1] * p2[1];

    let cx = (p0_sq * (p1[1] - p2[1]) + p1_sq * (p2[1] - p0[1]) + p2_sq * (p0[1] - p1[1])) / d;
    let cy = (p0_sq * (p2[0] - p1[0]) + p1_sq * (p0[0] - p2[0]) + p2_sq * (p1[0] - p0[0])) / d;
    let r_sq = (p0[0] - cx).powi(2) + (p0[1] - cy).powi(2);
    Some(([cx, cy], r_sq))
}

#[derive(Debug, Clone, Copy)]
struct DTri {
    v: [usize; 3],
    center: [f64; 2],
    radius_sq: f64,
}

/// Compute 2D Delaunay Triangulation using Bowyer-Watson algorithm.
pub fn delaunay_triangulation_2d(points: &[[f64; 3]]) -> Vec<[usize; 3]> {
    if points.len() < 3 {
        return Vec::new();
    }

    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for pt in points {
        min_x = min_x.min(pt[0]);
        max_x = max_x.max(pt[0]);
        min_y = min_y.min(pt[1]);
        max_y = max_y.max(pt[1]);
    }

    let dx = max_x - min_x;
    let dy = max_y - min_y;
    let d_max = dx.max(dy).max(1.0);
    let mid_x = (min_x + max_x) * 0.5;
    let mid_y = (min_y + max_y) * 0.5;

    let s0 = [mid_x - 20.0 * d_max, mid_y - d_max];
    let s1 = [mid_x, mid_y + 20.0 * d_max];
    let s2 = [mid_x + 20.0 * d_max, mid_y - d_max];

    let mut all_pts_2d = vec![s0, s1, s2];
    for pt in points {
        all_pts_2d.push([pt[0], pt[1]]);
    }

    let (st_center, st_r_sq) = match circumcircle_2d(s0, s1, s2) {
        Some(res) => res,
        None => return Vec::new(),
    };

    let mut triangles = vec![DTri {
        v: [0, 1, 2],
        center: st_center,
        radius_sq: st_r_sq,
    }];

    for i in 3..all_pts_2d.len() {
        let pt = all_pts_2d[i];

        let mut bad_indices = Vec::new();
        for (idx, tri) in triangles.iter().enumerate() {
            let dist_sq = (pt[0] - tri.center[0]).powi(2) + (pt[1] - tri.center[1]).powi(2);
            if dist_sq <= tri.radius_sq + 1e-9 {
                bad_indices.push(idx);
            }
        }

        let mut edges: Vec<([usize; 2], bool)> = Vec::new();
        for &b in &bad_indices {
            let tri = &triangles[b];
            let tri_edges = [
                [tri.v[0], tri.v[1]],
                [tri.v[1], tri.v[2]],
                [tri.v[2], tri.v[0]],
            ];
            for e in tri_edges {
                if let Some(pos) = edges.iter().position(|&(other, _)| {
                    (other[0] == e[0] && other[1] == e[1]) || (other[0] == e[1] && other[1] == e[0])
                }) {
                    edges[pos].1 = false;
                } else {
                    edges.push((e, true));
                }
            }
        }

        let poly_edges: Vec<[usize; 2]> = edges
            .into_iter()
            .filter(|&(_, unique)| unique)
            .map(|(e, _)| e)
            .collect();

        for &b in bad_indices.iter().rev() {
            triangles.swap_remove(b);
        }

        for e in poly_edges {
            if let Some((c, r_sq)) = circumcircle_2d(all_pts_2d[e[0]], all_pts_2d[e[1]], pt) {
                triangles.push(DTri {
                    v: [e[0], e[1], i],
                    center: c,
                    radius_sq: r_sq,
                });
            }
        }
    }

    let mut result = Vec::new();
    for tri in triangles {
        if tri.v[0] >= 3 && tri.v[1] >= 3 && tri.v[2] >= 3 {
            let v0 = tri.v[0] - 3;
            let v1 = tri.v[1] - 3;
            let v2 = tri.v[2] - 3;

            let p0 = points[v0];
            let p1 = points[v1];
            let p2 = points[v2];
            let signed_area = (p1[0] - p0[0]) * (p2[1] - p0[1]) - (p2[0] - p0[0]) * (p1[1] - p0[1]);
            if signed_area < 0.0 {
                result.push([v0, v2, v1]);
            } else {
                result.push([v0, v1, v2]);
            }
        }
    }

    result
}

/// Create a new TIN surface (PolyfaceMesh) from survey points in the document.
pub fn create_surface_from_points(
    doc: &mut CadDocument,
    surface_name: &str,
    layer_filter: Option<&str>,
) -> Result<(Handle, usize, usize), String> {
    let mut raw_points: Vec<[f64; 3]> = Vec::new();

    for entity in doc.entities() {
        if let EntityType::Point(pt) = entity {
            if let Some(req_layer) = layer_filter {
                if !entity.common().layer.eq_ignore_ascii_case(req_layer) {
                    continue;
                }
            }
            raw_points.push([pt.location.x, pt.location.y, pt.location.z]);
        }
    }

    if raw_points.len() < 3 {
        return Err(format!(
            "Insufficient survey points ({}) to generate a TIN surface (minimum 3 required).",
            raw_points.len()
        ));
    }

    let mut dedup_points: Vec<[f64; 3]> = Vec::new();
    for pt in raw_points {
        let is_dup = dedup_points.iter().any(|dp| {
            (dp[0] - pt[0]).hypot(dp[1] - pt[1]) < 0.001
        });
        if !is_dup {
            dedup_points.push(pt);
        }
    }

    if dedup_points.len() < 3 {
        return Err("After deduplicating coincident points, fewer than 3 points remain.".to_string());
    }

    let triangles = delaunay_triangulation_2d(&dedup_points);
    if triangles.is_empty() {
        return Err("Delaunay triangulation failed (points might be collinear).".to_string());
    }

    let layer_name = format!("LS-TIN-{}", surface_name);
    ensure_layer_with_color(doc, &layer_name, 3);

    let mut mesh = PolyfaceMesh::new();
    mesh.common.layer = layer_name;
    mesh.common.color = Color::ByLayer;

    for pt in &dedup_points {
        mesh.add_vertex(PolyfaceVertex::from_xyz(pt[0], pt[1], pt[2]));
    }

    for tri in &triangles {
        mesh.add_face(PolyfaceFace::triangle(
            (tri[0] + 1) as i16,
            (tri[1] + 1) as i16,
            (tri[2] + 1) as i16,
        ));
    }

    let tri_count = triangles.len();
    let pt_count = dedup_points.len();
    let handle = doc.add_entity(EntityType::PolyfaceMesh(mesh)).map_err(|e| format!("{e:?}"))?;

    Ok((handle, pt_count, tri_count))
}

// ─────────────────────────────────────────────────────────────────────────────
// Smooth Contour Chaining (Civil 3D 5:1 Standard)
// ─────────────────────────────────────────────────────────────────────────────

fn smooth_chaikin_polyline(pts: &[[f64; 3]], is_closed: bool, iterations: usize) -> Vec<[f64; 3]> {
    if pts.len() < 3 || iterations == 0 {
        return pts.to_vec();
    }

    let mut current = pts.to_vec();
    for _ in 0..iterations {
        let n = current.len();
        let mut next = Vec::with_capacity(n * 2);

        if is_closed {
            for i in 0..n {
                let p0 = current[i];
                let p1 = current[(i + 1) % n];
                let q = [
                    0.75 * p0[0] + 0.25 * p1[0],
                    0.75 * p0[1] + 0.25 * p1[1],
                    p0[2],
                ];
                let r = [
                    0.25 * p0[0] + 0.75 * p1[0],
                    0.25 * p0[1] + 0.75 * p1[1],
                    p0[2],
                ];
                next.push(q);
                next.push(r);
            }
        } else {
            next.push(current[0]);
            for i in 0..n - 1 {
                let p0 = current[i];
                let p1 = current[i + 1];
                let q = [
                    0.75 * p0[0] + 0.25 * p1[0],
                    0.75 * p0[1] + 0.25 * p1[1],
                    p0[2],
                ];
                let r = [
                    0.25 * p0[0] + 0.75 * p1[0],
                    0.25 * p0[1] + 0.75 * p1[1],
                    p0[2],
                ];
                next.push(q);
                next.push(r);
            }
            next.push(current[n - 1]);
        }
        current = next;
    }
    current
}

fn chain_segments(segments: &[([f64; 3], [f64; 3])]) -> Vec<(Vec<[f64; 3]>, bool)> {
    if segments.is_empty() {
        return Vec::new();
    }

    let mut remaining: Vec<([f64; 3], [f64; 3])> = segments.to_vec();
    let mut chains: Vec<(Vec<[f64; 3]>, bool)> = Vec::new();

    let dist_sq = |a: [f64; 3], b: [f64; 3]| -> f64 {
        (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)
    };
    let tol_sq = 1e-6;

    while !remaining.is_empty() {
        let first = remaining.swap_remove(0);
        let mut chain = vec![first.0, first.1];

        // Extend forward
        let mut progress = true;
        while progress {
            progress = false;
            let end_pt = *chain.last().unwrap();
            for i in 0..remaining.len() {
                let seg = remaining[i];
                if dist_sq(end_pt, seg.0) < tol_sq {
                    chain.push(seg.1);
                    remaining.swap_remove(i);
                    progress = true;
                    break;
                } else if dist_sq(end_pt, seg.1) < tol_sq {
                    chain.push(seg.0);
                    remaining.swap_remove(i);
                    progress = true;
                    break;
                }
            }
        }

        // Extend backward
        progress = true;
        while progress {
            progress = false;
            let start_pt = *chain.first().unwrap();
            for i in 0..remaining.len() {
                let seg = remaining[i];
                if dist_sq(start_pt, seg.1) < tol_sq {
                    chain.insert(0, seg.0);
                    remaining.swap_remove(i);
                    progress = true;
                    break;
                } else if dist_sq(start_pt, seg.0) < tol_sq {
                    chain.insert(0, seg.1);
                    remaining.swap_remove(i);
                    progress = true;
                    break;
                }
            }
        }

        let start_pt = *chain.first().unwrap();
        let end_pt = *chain.last().unwrap();
        let is_closed = chain.len() >= 3 && dist_sq(start_pt, end_pt) < tol_sq;
        if is_closed {
            chain.pop();
        }

        chains.push((chain, is_closed));
    }

    chains
}

/// Generate smooth continuous 3D contour polylines with 5:1 Civil 3D interval ratio.
pub fn generate_contours_chained_smoothed(
    surf: &TinSurfaceData,
    minor_interval: f64,
    smooth: bool,
) -> (Vec<Polyline3D>, Vec<Polyline3D>) {
    let minor_interval = if minor_interval <= 0.0 { 1.0 } else { minor_interval };
    let major_interval = minor_interval * 5.0;

    let mut major_polylines = Vec::new();
    let mut minor_polylines = Vec::new();

    let start_z = (surf.min_z / minor_interval).ceil() * minor_interval;
    let end_z = (surf.max_z / minor_interval).floor() * minor_interval;

    let mut z_cur = start_z;
    while z_cur <= end_z + 1e-6 {
        let is_major = ((z_cur / major_interval).round() * major_interval - z_cur).abs() < (minor_interval * 0.1);
        let layer_name = if is_major { "LS-CONTOUR-MAJOR" } else { "LS-CONTOUR-MINOR" };

        let mut segments_at_z = Vec::new();

        for tri in &surf.triangles {
            let t_min = tri.v0[2].min(tri.v1[2]).min(tri.v2[2]);
            let t_max = tri.v0[2].max(tri.v1[2]).max(tri.v2[2]);

            if z_cur < t_min || z_cur > t_max || (t_max - t_min).abs() < 1e-12 {
                continue;
            }

            let edges = [
                (tri.v0, tri.v1),
                (tri.v1, tri.v2),
                (tri.v2, tri.v0),
            ];

            let mut intersections = Vec::with_capacity(2);
            for &(p0, p1) in &edges {
                let z0 = p0[2];
                let z1 = p1[2];

                if (z0 <= z_cur && z1 >= z_cur) || (z1 <= z_cur && z0 >= z_cur) {
                    let dz = z1 - z0;
                    if dz.abs() > 1e-12 {
                        let t = (z_cur - z0) / dz;
                        if (0.0..=1.0).contains(&t) {
                            let ix = p0[0] + t * (p1[0] - p0[0]);
                            let iy = p0[1] + t * (p1[1] - p0[1]);
                            intersections.push([ix, iy, z_cur]);
                        }
                    }
                }
            }

            if intersections.len() >= 2 {
                let p1 = intersections[0];
                let mut p2 = intersections[1];
                let d = (p1[0] - p2[0]).hypot(p1[1] - p2[1]);
                if d < 1e-6 && intersections.len() > 2 {
                    p2 = intersections[2];
                }
                let d = (p1[0] - p2[0]).hypot(p1[1] - p2[1]);
                if d >= 1e-6 {
                    segments_at_z.push((p1, p2));
                }
            }
        }

        let chains = chain_segments(&segments_at_z);
        for (pts, is_closed) in chains {
            let final_pts = if smooth {
                smooth_chaikin_polyline(&pts, is_closed, 2)
            } else {
                pts
            };

            let mut pl = Polyline3D::new();
            pl.common.layer = layer_name.to_string();
            pl.common.color = Color::ByLayer;
            pl.flags.closed = is_closed;
            for p in final_pts {
                pl.add_vertex(Vector3::new(p[0], p[1], p[2]));
            }

            if is_major {
                major_polylines.push(pl);
            } else {
                minor_polylines.push(pl);
            }
        }

        z_cur += minor_interval;
    }

    (major_polylines, minor_polylines)
}

// ─────────────────────────────────────────────────────────────────────────────
// TIN Editing Operations (Swap Edge, Raise/Lower, Delete Triangle)
// ─────────────────────────────────────────────────────────────────────────────

fn segments_intersect_2d(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    let cross = |v1: [f64; 2], v2: [f64; 2]| -> f64 { v1[0] * v2[1] - v1[1] * v2[0] };
    let sub = |p1: [f64; 2], p2: [f64; 2]| -> [f64; 2] { [p1[0] - p2[0], p1[1] - p2[1]] };

    let r = sub(b, a);
    let s = sub(d, c);
    let r_cross_s = cross(r, s);
    if r_cross_s.abs() < 1e-12 {
        return false;
    }

    let c_minus_a = sub(c, a);
    let t = cross(c_minus_a, s) / r_cross_s;
    let u = cross(c_minus_a, r) / r_cross_s;

    t > 1e-5 && t < (1.0 - 1e-5) && u > 1e-5 && u < (1.0 - 1e-5)
}

fn dist_to_segment_2d(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-12 {
        return (p[0] - a[0]).hypot(p[1] - a[1]);
    }
    let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len_sq).clamp(0.0, 1.0);
    let proj_x = a[0] + t * dx;
    let proj_y = a[1] + t * dy;
    (p[0] - proj_x).hypot(p[1] - proj_y)
}

pub fn swap_edge_in_mesh(mesh: &mut PolyfaceMesh, pt: DVec3) -> Result<String, String> {
    let n_faces = mesh.faces.len();
    if n_faces < 2 {
        return Err("Surface has fewer than 2 faces.".to_string());
    }

    let mut face_indices: Vec<[usize; 3]> = Vec::with_capacity(n_faces);
    for f in &mesh.faces {
        let idxs = [
            f.index1.unsigned_abs() as usize,
            f.index2.unsigned_abs() as usize,
            f.index3.unsigned_abs() as usize,
        ];
        if idxs[0] > 0 && idxs[1] > 0 && idxs[2] > 0 {
            face_indices.push([idxs[0] - 1, idxs[1] - 1, idxs[2] - 1]);
        } else {
            return Err("Non-triangular face encountered in surface mesh.".to_string());
        }
    }

    let get_v2d = |idx: usize| -> [f64; 2] {
        [mesh.vertices[idx].location.x, mesh.vertices[idx].location.y]
    };

    let mut cand_f_a = None;
    for (f_idx, &[i0, i1, i2]) in face_indices.iter().enumerate() {
        if i0 < mesh.vertices.len() && i1 < mesh.vertices.len() && i2 < mesh.vertices.len() {
            let tri = TinTriangle {
                v0: [mesh.vertices[i0].location.x, mesh.vertices[i0].location.y, mesh.vertices[i0].location.z],
                v1: [mesh.vertices[i1].location.x, mesh.vertices[i1].location.y, mesh.vertices[i1].location.z],
                v2: [mesh.vertices[i2].location.x, mesh.vertices[i2].location.y, mesh.vertices[i2].location.z],
            };
            if tri.contains_point_2d(pt.x, pt.y) {
                cand_f_a = Some(f_idx);
                break;
            }
        }
    }

    if cand_f_a.is_none() {
        let mut min_d2 = 9.0;
        for (f_idx, &[i0, i1, i2]) in face_indices.iter().enumerate() {
            if i0 < mesh.vertices.len() && i1 < mesh.vertices.len() && i2 < mesh.vertices.len() {
                let cx = (mesh.vertices[i0].location.x + mesh.vertices[i1].location.x + mesh.vertices[i2].location.x) / 3.0;
                let cy = (mesh.vertices[i0].location.y + mesh.vertices[i1].location.y + mesh.vertices[i2].location.y) / 3.0;
                let d2 = (pt.x - cx) * (pt.x - cx) + (pt.y - cy) * (pt.y - cy);
                if d2 < min_d2 {
                    min_d2 = d2;
                    cand_f_a = Some(f_idx);
                }
            }
        }
    }

    let f_a = cand_f_a.ok_or_else(|| "No triangle found near pick point.".to_string())?;
    let tri_a = face_indices[f_a];

    let p_click = [pt.x, pt.y];
    let mut edges = [
        (tri_a[0], tri_a[1]),
        (tri_a[1], tri_a[2]),
        (tri_a[2], tri_a[0]),
    ];
    edges.sort_by(|&(u0, v0), &(u1, v1)| {
        let d0 = dist_to_segment_2d(p_click, get_v2d(u0), get_v2d(v0));
        let d1 = dist_to_segment_2d(p_click, get_v2d(u1), get_v2d(v1));
        d0.partial_cmp(&d1).unwrap_or(std::cmp::Ordering::Equal)
    });

    for &(e_u, e_v) in &edges {
        for (f_b, tri_b) in face_indices.iter().enumerate() {
            if f_b == f_a { continue; }
            if tri_b.contains(&e_u) && tri_b.contains(&e_v) {
                let va_opp = tri_a.iter().copied().find(|&v| v != e_u && v != e_v).unwrap();
                let vb_opp = tri_b.iter().copied().find(|&v| v != e_u && v != e_v).unwrap();

                let s0 = e_u;
                let s1 = e_v;

                if segments_intersect_2d(get_v2d(s0), get_v2d(s1), get_v2d(va_opp), get_v2d(vb_opp)) {
                    let p_va = get_v2d(va_opp);
                    let p_vb = get_v2d(vb_opp);
                    let p_s0 = get_v2d(s0);
                    let p_s1 = get_v2d(s1);

                    let signed_area = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| {
                        (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])
                    };

                    let tri1 = if signed_area(p_va, p_vb, p_s0) > 0.0 {
                        (va_opp, vb_opp, s0)
                    } else {
                        (va_opp, s0, vb_opp)
                    };

                    let tri2 = if signed_area(p_va, p_s1, p_vb) > 0.0 {
                        (va_opp, s1, vb_opp)
                    } else {
                        (va_opp, vb_opp, s1)
                    };

                    mesh.faces[f_a] = PolyfaceFace::triangle(
                        (tri1.0 + 1) as i16,
                        (tri1.1 + 1) as i16,
                        (tri1.2 + 1) as i16,
                    );
                    mesh.faces[f_b] = PolyfaceFace::triangle(
                        (tri2.0 + 1) as i16,
                        (tri2.1 + 1) as i16,
                        (tri2.2 + 1) as i16,
                    );

                    return Ok(format!(
                        "✔ Swapped edge between triangles #{} and #{} (flipped diagonal {:?} <-> {:?}).",
                        f_a + 1, f_b + 1, (s0 + 1, s1 + 1), (va_opp + 1, vb_opp + 1)
                    ));
                }
            }
        }
    }

    Err("Selected edge cannot be swapped (outer boundary or concave quad).".to_string())
}

pub fn swap_edge_in_dxf_mesh(mesh: &mut codec::entities::Mesh, pt: DVec3) -> Result<String, String> {
    let n_faces = mesh.faces.len();
    if n_faces < 2 {
        return Err("Surface has fewer than 2 faces.".to_string());
    }

    let get_v2d = |idx: usize| -> [f64; 2] {
        [mesh.vertices[idx].x, mesh.vertices[idx].y]
    };

    let mut cand_f_a = None;
    for (f_idx, face) in mesh.faces.iter().enumerate() {
        if face.vertices.len() >= 3 {
            let i0 = face.vertices[0];
            let i1 = face.vertices[1];
            let i2 = face.vertices[2];
            if i0 < mesh.vertices.len() && i1 < mesh.vertices.len() && i2 < mesh.vertices.len() {
                let tri = TinTriangle {
                    v0: [mesh.vertices[i0].x, mesh.vertices[i0].y, mesh.vertices[i0].z],
                    v1: [mesh.vertices[i1].x, mesh.vertices[i1].y, mesh.vertices[i1].z],
                    v2: [mesh.vertices[i2].x, mesh.vertices[i2].y, mesh.vertices[i2].z],
                };
                if tri.contains_point_2d(pt.x, pt.y) {
                    cand_f_a = Some(f_idx);
                    break;
                }
            }
        }
    }

    let f_a = cand_f_a.ok_or_else(|| "No triangle found near pick point.".to_string())?;
    let tri_a = &mesh.faces[f_a].vertices;
    if tri_a.len() != 3 {
        return Err("Target face is not triangular.".to_string());
    }

    let p_click = [pt.x, pt.y];
    let mut edges = [
        (tri_a[0], tri_a[1]),
        (tri_a[1], tri_a[2]),
        (tri_a[2], tri_a[0]),
    ];
    edges.sort_by(|&(u0, v0), &(u1, v1)| {
        let d0 = dist_to_segment_2d(p_click, get_v2d(u0), get_v2d(v0));
        let d1 = dist_to_segment_2d(p_click, get_v2d(u1), get_v2d(v1));
        d0.partial_cmp(&d1).unwrap_or(std::cmp::Ordering::Equal)
    });

    for &(e_u, e_v) in &edges {
        for (f_b, face_b) in mesh.faces.iter().enumerate() {
            if f_b == f_a { continue; }
            let tri_b = &face_b.vertices;
            if tri_b.len() == 3 && tri_b.contains(&e_u) && tri_b.contains(&e_v) {
                let va_opp = tri_a.iter().copied().find(|&v| v != e_u && v != e_v).unwrap();
                let vb_opp = tri_b.iter().copied().find(|&v| v != e_u && v != e_v).unwrap();
                let s0 = e_u;
                let s1 = e_v;

                if segments_intersect_2d(get_v2d(s0), get_v2d(s1), get_v2d(va_opp), get_v2d(vb_opp)) {
                    mesh.faces[f_a] = codec::entities::mesh::MeshFace::triangle(va_opp, vb_opp, s0);
                    mesh.faces[f_b] = codec::entities::mesh::MeshFace::triangle(va_opp, s1, vb_opp);
                    return Ok(format!("✔ Swapped edge between mesh triangles #{} and #{}.", f_a + 1, f_b + 1));
                }
            }
        }
    }

    Err("Selected edge cannot be swapped (outer boundary or concave quad).".to_string())
}

/// Swap shared edge between adjacent triangles in a TIN surface.
pub fn swap_edge_in_surface(
    doc: &mut CadDocument,
    handle: Handle,
    candidate_tri_idx: Option<usize>,
) -> Result<String, String> {
    let entity = doc.get_entity_mut(handle)
        .ok_or_else(|| format!("Surface entity {:X} not found in drawing.", handle.value()))?;

    match entity {
        EntityType::PolyfaceMesh(mesh) => {
            if let Some(idx) = candidate_tri_idx {
                if idx < mesh.faces.len() {
                    let f = &mesh.faces[idx];
                    let i0 = (f.index1.unsigned_abs() as usize).saturating_sub(1);
                    let i1 = (f.index2.unsigned_abs() as usize).saturating_sub(1);
                    let i2 = (f.index3.unsigned_abs() as usize).saturating_sub(1);
                    if i0 < mesh.vertices.len() && i1 < mesh.vertices.len() && i2 < mesh.vertices.len() {
                        let cx = (mesh.vertices[i0].location.x + mesh.vertices[i1].location.x + mesh.vertices[i2].location.x) / 3.0;
                        let cy = (mesh.vertices[i0].location.y + mesh.vertices[i1].location.y + mesh.vertices[i2].location.y) / 3.0;
                        let cz = (mesh.vertices[i0].location.z + mesh.vertices[i1].location.z + mesh.vertices[i2].location.z) / 3.0;
                        if let Ok(res) = swap_edge_in_mesh(mesh, DVec3::new(cx, cy, cz)) {
                            return Ok(res);
                        }
                    }
                }
            }
            for idx in 0..mesh.faces.len() {
                let f = &mesh.faces[idx];
                let i0 = (f.index1.unsigned_abs() as usize).saturating_sub(1);
                let i1 = (f.index2.unsigned_abs() as usize).saturating_sub(1);
                let i2 = (f.index3.unsigned_abs() as usize).saturating_sub(1);
                if i0 < mesh.vertices.len() && i1 < mesh.vertices.len() && i2 < mesh.vertices.len() {
                    let cx = (mesh.vertices[i0].location.x + mesh.vertices[i1].location.x + mesh.vertices[i2].location.x) / 3.0;
                    let cy = (mesh.vertices[i0].location.y + mesh.vertices[i1].location.y + mesh.vertices[i2].location.y) / 3.0;
                    let cz = (mesh.vertices[i0].location.z + mesh.vertices[i1].location.z + mesh.vertices[i2].location.z) / 3.0;
                    if let Ok(res) = swap_edge_in_mesh(mesh, DVec3::new(cx, cy, cz)) {
                        return Ok(res);
                    }
                }
            }
            Err("No swappable convex triangle pair found in surface.".to_string())
        }
        EntityType::Mesh(mesh) => {
            for idx in 0..mesh.faces.len() {
                let face = &mesh.faces[idx];
                if face.vertices.len() >= 3 {
                    let i0 = face.vertices[0];
                    let i1 = face.vertices[1];
                    let i2 = face.vertices[2];
                    if i0 < mesh.vertices.len() && i1 < mesh.vertices.len() && i2 < mesh.vertices.len() {
                        let cx = (mesh.vertices[i0].x + mesh.vertices[i1].x + mesh.vertices[i2].x) / 3.0;
                        let cy = (mesh.vertices[i0].y + mesh.vertices[i1].y + mesh.vertices[i2].y) / 3.0;
                        let cz = (mesh.vertices[i0].z + mesh.vertices[i1].z + mesh.vertices[i2].z) / 3.0;
                        if let Ok(res) = swap_edge_in_dxf_mesh(mesh, DVec3::new(cx, cy, cz)) {
                            return Ok(res);
                        }
                    }
                }
            }
            Err("No swappable convex triangle pair found in mesh surface.".to_string())
        }
        _ => Err("Target entity is not a TIN surface (PolyfaceMesh or Mesh).".to_string()),
    }
}


/// Raise or lower surface elevation by delta_z.
pub fn raise_lower_surface(
    doc: &mut CadDocument,
    handle: Handle,
    delta_z: f64,
) -> Result<String, String> {
    let entity = doc.get_entity_mut(handle)
        .ok_or_else(|| format!("Surface entity {:X} not found.", handle.value()))?;

    match entity {
        EntityType::PolyfaceMesh(mesh) => {
            for v in &mut mesh.vertices {
                v.location.z += delta_z;
            }
            Ok(format!("✔ Adjusted {} vertices in surface [{:X}] by {:+.3} m.", mesh.vertices.len(), handle.value(), delta_z))
        }
        EntityType::Mesh(mesh) => {
            for v in &mut mesh.vertices {
                v.z += delta_z;
            }
            Ok(format!("✔ Adjusted {} vertices in mesh surface [{:X}] by {:+.3} m.", mesh.vertices.len(), handle.value(), delta_z))
        }
        _ => Err("Target entity is not a TIN surface.".to_string()),
    }
}

/// Delete a boundary or specified triangle from the TIN surface.
pub fn delete_triangle_in_surface(
    doc: &mut CadDocument,
    handle: Handle,
    tri_idx: Option<usize>,
) -> Result<String, String> {
    let entity = doc.get_entity_mut(handle)
        .ok_or_else(|| format!("Surface entity {:X} not found.", handle.value()))?;

    match entity {
        EntityType::PolyfaceMesh(mesh) => {
            if mesh.faces.is_empty() {
                return Err("Surface has no triangles left.".to_string());
            }

            let target_idx = if let Some(idx) = tri_idx {
                if idx >= mesh.faces.len() {
                    return Err(format!("Triangle index {} out of range (total {}).", idx, mesh.faces.len()));
                }
                idx
            } else {
                let mut edge_count = std::collections::HashMap::new();
                for f in &mesh.faces {
                    let idxs = [f.index1.unsigned_abs(), f.index2.unsigned_abs(), f.index3.unsigned_abs()];
                    let edges = [
                        (idxs[0].min(idxs[1]), idxs[0].max(idxs[1])),
                        (idxs[1].min(idxs[2]), idxs[1].max(idxs[2])),
                        (idxs[2].min(idxs[0]), idxs[2].max(idxs[0])),
                    ];
                    for e in edges {
                        *edge_count.entry(e).or_insert(0usize) += 1;
                    }
                }

                let mut best_face = 0;
                let mut max_outer_len = -1.0;

                for (f_idx, f) in mesh.faces.iter().enumerate() {
                    let idxs = [f.index1.unsigned_abs() as usize, f.index2.unsigned_abs() as usize, f.index3.unsigned_abs() as usize];
                    if idxs[0] == 0 || idxs[1] == 0 || idxs[2] == 0 { continue; }
                    let edges = [
                        ((idxs[0] as u16).min(idxs[1] as u16), (idxs[0] as u16).max(idxs[1] as u16), idxs[0] - 1, idxs[1] - 1),
                        ((idxs[1] as u16).min(idxs[2] as u16), (idxs[1] as u16).max(idxs[2] as u16), idxs[1] - 1, idxs[2] - 1),
                        ((idxs[2] as u16).min(idxs[0] as u16), (idxs[2] as u16).max(idxs[0] as u16), idxs[2] - 1, idxs[0] - 1),
                    ];
                    for &(k0, k1, v0, v1) in &edges {
                        if edge_count.get(&(k0, k1)) == Some(&1) {
                            if v0 < mesh.vertices.len() && v1 < mesh.vertices.len() {
                                let p0 = mesh.vertices[v0].location;
                                let p1 = mesh.vertices[v1].location;
                                let len = (p0.x - p1.x).hypot(p0.y - p1.y);
                                if len > max_outer_len {
                                    max_outer_len = len;
                                    best_face = f_idx;
                                }
                            }
                        }
                    }
                }
                best_face
            };

            mesh.faces.remove(target_idx);
            Ok(format!("✔ Deleted triangle #{} from surface [{:X}] (remaining triangles: {}).", target_idx + 1, handle.value(), mesh.faces.len()))
        }
        EntityType::Mesh(mesh) => {
            if mesh.faces.is_empty() {
                return Err("Mesh surface has no faces left.".to_string());
            }
            let target_idx = tri_idx.unwrap_or(mesh.faces.len() - 1).min(mesh.faces.len() - 1);
            mesh.faces.remove(target_idx);
            Ok(format!("✔ Deleted triangle #{} from mesh surface [{:X}].", target_idx + 1, handle.value()))
        }
        _ => Err("Target entity is not a TIN surface.".to_string()),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Interactive TIN Surface Editing Commands (Viewport click workflows)
// ─────────────────────────────────────────────────────────────────────────────

pub struct SurfaceDeleteTriangleCommand {
    pub handle: Handle,
    pub entity: EntityType,
    pub deleted_count: usize,
}

impl SurfaceDeleteTriangleCommand {
    pub fn new(handle: Handle, entity: EntityType) -> Self {
        Self {
            handle,
            entity,
            deleted_count: 0,
        }
    }
}

impl CadCommand for SurfaceDeleteTriangleCommand {
    fn name(&self) -> &'static str {
        "SURFACEDELETETRI"
    }

    fn prompt(&self) -> String {
        if self.deleted_count == 0 {
            "SURFACEDELETETRI  حدد المثلث المراد حذفه في مساحة العمل [أو اضغط Enter للإنهاء]:".to_string()
        } else {
            format!(
                "SURFACEDELETETRI  تم حذف {} مثلث. حدد مثلثاً آخر [أو اضغط Enter للإنهاء]:",
                self.deleted_count
            )
        }
    }

    fn options(&self) -> Vec<CmdOption> {
        vec![CmdOption::new("Finish", "")]
    }

    fn on_preview_wires(&mut self, pt: DVec3) -> Vec<WireModel> {
        let mut wires = Vec::new();
        match &self.entity {
            EntityType::PolyfaceMesh(mesh) => {
                for face in &mesh.faces {
                    let i1 = (face.index1.unsigned_abs() as usize).saturating_sub(1);
                    let i2 = (face.index2.unsigned_abs() as usize).saturating_sub(1);
                    let i3 = (face.index3.unsigned_abs() as usize).saturating_sub(1);

                    if i1 < mesh.vertices.len() && i2 < mesh.vertices.len() && i3 < mesh.vertices.len() {
                        let p0 = [mesh.vertices[i1].location.x, mesh.vertices[i1].location.y, mesh.vertices[i1].location.z];
                        let p1 = [mesh.vertices[i2].location.x, mesh.vertices[i2].location.y, mesh.vertices[i2].location.z];
                        let p2 = [mesh.vertices[i3].location.x, mesh.vertices[i3].location.y, mesh.vertices[i3].location.z];
                        let tri = TinTriangle { v0: p0, v1: p1, v2: p2 };
                        if tri.contains_point_2d(pt.x, pt.y) {
                            let mut wm = WireModel::solid_f64(
                                "del_tri_hover".to_string(),
                                vec![p0, p1, p2, p0],
                                [1.0, 0.2, 0.1, 0.9],
                                false,
                            );
                            wm.set_fixed_screen_width(3.0);
                            wires.push(wm);
                            break;
                        }
                    }
                }
            }
            EntityType::Mesh(mesh) => {
                for face in &mesh.faces {
                    if face.vertices.len() >= 3 {
                        let i1 = face.vertices[0];
                        let i2 = face.vertices[1];
                        let i3 = face.vertices[2];
                        if i1 < mesh.vertices.len() && i2 < mesh.vertices.len() && i3 < mesh.vertices.len() {
                            let p0 = [mesh.vertices[i1].x, mesh.vertices[i1].y, mesh.vertices[i1].z];
                            let p1 = [mesh.vertices[i2].x, mesh.vertices[i2].y, mesh.vertices[i2].z];
                            let p2 = [mesh.vertices[i3].x, mesh.vertices[i3].y, mesh.vertices[i3].z];
                            let tri = TinTriangle { v0: p0, v1: p1, v2: p2 };
                            if tri.contains_point_2d(pt.x, pt.y) {
                                let mut wm = WireModel::solid_f64(
                                    "del_tri_hover".to_string(),
                                    vec![p0, p1, p2, p0],
                                    [1.0, 0.2, 0.1, 0.9],
                                    false,
                                );
                                wm.set_fixed_screen_width(3.0);
                                wires.push(wm);
                                break;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        wires
    }

    fn on_point(&mut self, pt: DVec3) -> CmdResult {
        match &mut self.entity {
            EntityType::PolyfaceMesh(mesh) => {
                let n_faces = mesh.faces.len();
                if n_faces == 0 {
                    return CmdResult::CancelWithMessage("Surface has no triangles left.".to_string());
                }

                let mut hit_idx = None;
                for (f_idx, face) in mesh.faces.iter().enumerate() {
                    let i1 = (face.index1.unsigned_abs() as usize).saturating_sub(1);
                    let i2 = (face.index2.unsigned_abs() as usize).saturating_sub(1);
                    let i3 = (face.index3.unsigned_abs() as usize).saturating_sub(1);

                    if i1 < mesh.vertices.len() && i2 < mesh.vertices.len() && i3 < mesh.vertices.len() {
                        let tri = TinTriangle {
                            v0: [mesh.vertices[i1].location.x, mesh.vertices[i1].location.y, mesh.vertices[i1].location.z],
                            v1: [mesh.vertices[i2].location.x, mesh.vertices[i2].location.y, mesh.vertices[i2].location.z],
                            v2: [mesh.vertices[i3].location.x, mesh.vertices[i3].location.y, mesh.vertices[i3].location.z],
                        };
                        if tri.contains_point_2d(pt.x, pt.y) {
                            hit_idx = Some(f_idx);
                            break;
                        }
                    }
                }

                if hit_idx.is_none() {
                    let mut min_dist_sq = 9.0;
                    for (f_idx, face) in mesh.faces.iter().enumerate() {
                        let i1 = (face.index1.unsigned_abs() as usize).saturating_sub(1);
                        let i2 = (face.index2.unsigned_abs() as usize).saturating_sub(1);
                        let i3 = (face.index3.unsigned_abs() as usize).saturating_sub(1);

                        if i1 < mesh.vertices.len() && i2 < mesh.vertices.len() && i3 < mesh.vertices.len() {
                            let cx = (mesh.vertices[i1].location.x + mesh.vertices[i2].location.x + mesh.vertices[i3].location.x) / 3.0;
                            let cy = (mesh.vertices[i1].location.y + mesh.vertices[i2].location.y + mesh.vertices[i3].location.y) / 3.0;
                            let d2 = (pt.x - cx) * (pt.x - cx) + (pt.y - cy) * (pt.y - cy);
                            if d2 < min_dist_sq {
                                min_dist_sq = d2;
                                hit_idx = Some(f_idx);
                            }
                        }
                    }
                }

                if let Some(idx) = hit_idx {
                    mesh.faces.remove(idx);
                    self.deleted_count += 1;
                    CmdResult::UpdateEntityContinue {
                        handle: self.handle,
                        entity: self.entity.clone(),
                    }
                } else {
                    CmdResult::NeedPoint
                }
            }
            EntityType::Mesh(mesh) => {
                let n_faces = mesh.faces.len();
                if n_faces == 0 {
                    return CmdResult::CancelWithMessage("Mesh surface has no triangles left.".to_string());
                }

                let mut hit_idx = None;
                for (f_idx, face) in mesh.faces.iter().enumerate() {
                    if face.vertices.len() >= 3 {
                        let i1 = face.vertices[0];
                        let i2 = face.vertices[1];
                        let i3 = face.vertices[2];
                        if i1 < mesh.vertices.len() && i2 < mesh.vertices.len() && i3 < mesh.vertices.len() {
                            let tri = TinTriangle {
                                v0: [mesh.vertices[i1].x, mesh.vertices[i1].y, mesh.vertices[i1].z],
                                v1: [mesh.vertices[i2].x, mesh.vertices[i2].y, mesh.vertices[i2].z],
                                v2: [mesh.vertices[i3].x, mesh.vertices[i3].y, mesh.vertices[i3].z],
                            };
                            if tri.contains_point_2d(pt.x, pt.y) {
                                hit_idx = Some(f_idx);
                                break;
                            }
                        }
                    }
                }

                if let Some(idx) = hit_idx {
                    mesh.faces.remove(idx);
                    self.deleted_count += 1;
                    CmdResult::UpdateEntityContinue {
                        handle: self.handle,
                        entity: self.entity.clone(),
                    }
                } else {
                    CmdResult::NeedPoint
                }
            }
            _ => CmdResult::CancelWithMessage("Not a TIN surface entity.".to_string()),
        }
    }

    fn on_enter(&mut self) -> CmdResult {
        if self.deleted_count > 0 {
            CmdResult::CancelWithMessage(format!(
                "✔ Surface editing finished: {} triangle(s) deleted.",
                self.deleted_count
            ))
        } else {
            CmdResult::Cancel
        }
    }

    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

pub struct SurfaceSwapEdgeCommand {
    pub handle: Handle,
    pub entity: EntityType,
    pub swapped_count: usize,
}

impl SurfaceSwapEdgeCommand {
    pub fn new(handle: Handle, entity: EntityType) -> Self {
        Self {
            handle,
            entity,
            swapped_count: 0,
        }
    }
}

impl CadCommand for SurfaceSwapEdgeCommand {
    fn name(&self) -> &'static str {
        "SURFACESWAPEDGE"
    }

    fn prompt(&self) -> String {
        if self.swapped_count == 0 {
            "SURFACESWAPEDGE  حدد الضلع أو المثلث لتبديل الاتجاه [أو اضغط Enter للإنهاء]:".to_string()
        } else {
            format!(
                "SURFACESWAPEDGE  تم تبديل {} ضلع. حدد ضلعاً آخر [أو اضغط Enter للإنهاء]:",
                self.swapped_count
            )
        }
    }

    fn options(&self) -> Vec<CmdOption> {
        vec![CmdOption::new("Finish", "")]
    }

    fn on_preview_wires(&mut self, pt: DVec3) -> Vec<WireModel> {
        let mut wires = Vec::new();
        match &self.entity {
            EntityType::PolyfaceMesh(mesh) => {
                for face in &mesh.faces {
                    let i1 = (face.index1.unsigned_abs() as usize).saturating_sub(1);
                    let i2 = (face.index2.unsigned_abs() as usize).saturating_sub(1);
                    let i3 = (face.index3.unsigned_abs() as usize).saturating_sub(1);

                    if i1 < mesh.vertices.len() && i2 < mesh.vertices.len() && i3 < mesh.vertices.len() {
                        let p0 = [mesh.vertices[i1].location.x, mesh.vertices[i1].location.y, mesh.vertices[i1].location.z];
                        let p1 = [mesh.vertices[i2].location.x, mesh.vertices[i2].location.y, mesh.vertices[i2].location.z];
                        let p2 = [mesh.vertices[i3].location.x, mesh.vertices[i3].location.y, mesh.vertices[i3].location.z];
                        let tri = TinTriangle { v0: p0, v1: p1, v2: p2 };
                        if tri.contains_point_2d(pt.x, pt.y) {
                            let mut wm = WireModel::solid_f64(
                                "swap_edge_hover".to_string(),
                                vec![p0, p1, p2, p0],
                                [0.0, 0.9, 1.0, 0.9],
                                false,
                            );
                            wm.set_fixed_screen_width(3.0);
                            wires.push(wm);
                            break;
                        }
                    }
                }
            }
            EntityType::Mesh(mesh) => {
                for face in &mesh.faces {
                    if face.vertices.len() >= 3 {
                        let i1 = face.vertices[0];
                        let i2 = face.vertices[1];
                        let i3 = face.vertices[2];
                        if i1 < mesh.vertices.len() && i2 < mesh.vertices.len() && i3 < mesh.vertices.len() {
                            let p0 = [mesh.vertices[i1].x, mesh.vertices[i1].y, mesh.vertices[i1].z];
                            let p1 = [mesh.vertices[i2].x, mesh.vertices[i2].y, mesh.vertices[i2].z];
                            let p2 = [mesh.vertices[i3].x, mesh.vertices[i3].y, mesh.vertices[i3].z];
                            let tri = TinTriangle { v0: p0, v1: p1, v2: p2 };
                            if tri.contains_point_2d(pt.x, pt.y) {
                                let mut wm = WireModel::solid_f64(
                                    "swap_edge_hover".to_string(),
                                    vec![p0, p1, p2, p0],
                                    [0.0, 0.9, 1.0, 0.9],
                                    false,
                                );
                                wm.set_fixed_screen_width(3.0);
                                wires.push(wm);
                                break;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        wires
    }

    fn on_point(&mut self, pt: DVec3) -> CmdResult {
        match &mut self.entity {
            EntityType::PolyfaceMesh(mesh) => {
                match swap_edge_in_mesh(mesh, pt) {
                    Ok(_) => {
                        self.swapped_count += 1;
                        CmdResult::UpdateEntityContinue {
                            handle: self.handle,
                            entity: self.entity.clone(),
                        }
                    }
                    Err(_) => CmdResult::NeedPoint,
                }
            }
            EntityType::Mesh(mesh) => {
                match swap_edge_in_dxf_mesh(mesh, pt) {
                    Ok(_) => {
                        self.swapped_count += 1;
                        CmdResult::UpdateEntityContinue {
                            handle: self.handle,
                            entity: self.entity.clone(),
                        }
                    }
                    Err(_) => CmdResult::NeedPoint,
                }
            }
            _ => CmdResult::CancelWithMessage("Not a TIN surface entity.".to_string()),
        }
    }

    fn on_enter(&mut self) -> CmdResult {
        if self.swapped_count > 0 {
            CmdResult::CancelWithMessage(format!(
                "✔ Surface editing finished: {} edge(s) swapped.",
                self.swapped_count
            ))
        } else {
            CmdResult::Cancel
        }
    }

    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}


/// Apply display style / visibility to surface.
pub fn apply_surface_style(
    doc: &mut CadDocument,
    handle: Handle,
    style: &str,
) -> Result<String, String> {
    let surf = TinSurfaceData::from_document_entity(doc, handle)
        .ok_or_else(|| format!("Surface {:X} not found.", handle.value()))?;

    let style_norm = style.trim().to_ascii_uppercase();

    match style_norm.as_str() {
        "TRIANGLES" | "TRIANGLE" => {
            if let Some(l) = doc.layers.get_mut(&surf.layer) {
                l.flags.off = false;
            }
            if let Some(l) = doc.layers.get_mut("LS-CONTOUR-MAJOR") {
                l.flags.off = true;
            }
            if let Some(l) = doc.layers.get_mut("LS-CONTOUR-MINOR") {
                l.flags.off = true;
            }
            Ok(format!("✔ Applied style 'Triangles Only' to surface '{}'.", surf.name))
        }
        "CONTOURS" | "CONTOUR" => {
            if let Some(l) = doc.layers.get_mut(&surf.layer) {
                l.flags.off = true;
            }
            ensure_layer_with_color(doc, "LS-CONTOUR-MAJOR", 1);
            ensure_layer_with_color(doc, "LS-CONTOUR-MINOR", 8);
            if let Some(l) = doc.layers.get_mut("LS-CONTOUR-MAJOR") {
                l.flags.off = false;
            }
            if let Some(l) = doc.layers.get_mut("LS-CONTOUR-MINOR") {
                l.flags.off = false;
            }

            let contour_count = doc.entities().filter(|e| {
                e.common().layer == "LS-CONTOUR-MAJOR" || e.common().layer == "LS-CONTOUR-MINOR"
            }).count();
            if contour_count == 0 {
                let (maj, min) = generate_contours_chained_smoothed(&surf, 1.0, true);
                for pl in maj {
                    let _ = doc.add_entity(EntityType::Polyline3D(pl));
                }
                for pl in min {
                    let _ = doc.add_entity(EntityType::Polyline3D(pl));
                }
            }
            Ok(format!("✔ Applied style 'Contours 1m and 5m' to surface '{}'.", surf.name))
        }
        "BOTH" | "CONTOURS_AND_TRIANGLES" | "ALL" => {
            if let Some(l) = doc.layers.get_mut(&surf.layer) {
                l.flags.off = false;
            }
            ensure_layer_with_color(doc, "LS-CONTOUR-MAJOR", 1);
            ensure_layer_with_color(doc, "LS-CONTOUR-MINOR", 8);
            if let Some(l) = doc.layers.get_mut("LS-CONTOUR-MAJOR") {
                l.flags.off = false;
            }
            if let Some(l) = doc.layers.get_mut("LS-CONTOUR-MINOR") {
                l.flags.off = false;
            }
            let contour_count = doc.entities().filter(|e| {
                e.common().layer == "LS-CONTOUR-MAJOR" || e.common().layer == "LS-CONTOUR-MINOR"
            }).count();
            if contour_count == 0 {
                let (maj, min) = generate_contours_chained_smoothed(&surf, 1.0, true);
                for pl in maj {
                    let _ = doc.add_entity(EntityType::Polyline3D(pl));
                }
                for pl in min {
                    let _ = doc.add_entity(EntityType::Polyline3D(pl));
                }
            }
            Ok(format!("✔ Applied style 'Contours and Triangles' to surface '{}'.", surf.name))
        }
        "BORDER" => {
            if let Some(l) = doc.layers.get_mut(&surf.layer) {
                l.flags.off = true;
            }
            if let Some(l) = doc.layers.get_mut("LS-CONTOUR-MAJOR") {
                l.flags.off = true;
            }
            if let Some(l) = doc.layers.get_mut("LS-CONTOUR-MINOR") {
                l.flags.off = true;
            }

            let mut edge_count = std::collections::HashMap::new();
            for tri in &surf.triangles {
                let k0 = ((tri.v0[0]*1000.0).round() as i64, (tri.v0[1]*1000.0).round() as i64);
                let k1 = ((tri.v1[0]*1000.0).round() as i64, (tri.v1[1]*1000.0).round() as i64);
                let k2 = ((tri.v2[0]*1000.0).round() as i64, (tri.v2[1]*1000.0).round() as i64);
                let e0 = (k0.min(k1), k0.max(k1), tri.v0, tri.v1);
                let e1 = (k1.min(k2), k1.max(k2), tri.v1, tri.v2);
                let e2 = (k2.min(k0), k2.max(k0), tri.v2, tri.v0);
                for &(ka, kb, va, vb) in &[e0, e1, e2] {
                    edge_count.entry((ka, kb)).or_insert((0usize, va, vb)).0 += 1;
                }
            }
            let outer_segs: Vec<([f64; 3], [f64; 3])> = edge_count.into_iter()
                .filter(|(_, (cnt, _, _))| *cnt == 1)
                .map(|(_, (_, va, vb))| (va, vb))
                .collect();
            let chains = chain_segments(&outer_segs);
            ensure_layer_with_color(doc, "LS-TIN-BORDER", 6);
            if let Some(l) = doc.layers.get_mut("LS-TIN-BORDER") {
                l.flags.off = false;
            }
            for (pts, is_closed) in chains {
                let mut pl = Polyline3D::new();
                pl.common.layer = "LS-TIN-BORDER".to_string();
                pl.common.color = Color::ByLayer;
                pl.flags.closed = is_closed;
                for p in pts {
                    pl.add_vertex(Vector3::new(p[0], p[1], p[2]));
                }
                let _ = doc.add_entity(EntityType::Polyline3D(pl));
            }
            Ok(format!("✔ Applied style 'Border Only' to surface '{}'.", surf.name))
        }
        "OFF" | "NONE" => {
            if let Some(l) = doc.layers.get_mut(&surf.layer) {
                l.flags.off = true;
            }
            if let Some(l) = doc.layers.get_mut("LS-CONTOUR-MAJOR") {
                l.flags.off = true;
            }
            if let Some(l) = doc.layers.get_mut("LS-CONTOUR-MINOR") {
                l.flags.off = true;
            }
            Ok(format!("✔ Turned OFF display for surface '{}'.", surf.name))
        }
        _ => Err(format!("Unknown style '{}'. Options: TRIANGLES, CONTOURS, BOTH, BORDER, OFF.", style)),
    }
}
