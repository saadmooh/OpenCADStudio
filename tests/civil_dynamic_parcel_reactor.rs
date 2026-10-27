use codec::entities::{LwPolyline, Text};
use codec::types::{Vector2, Vector3};
use codec::{CadDocument, EntityType};
use OpenCADStudio::app::commands::civil_parcel::{ParcelData, is_parcel_layer, is_parcel_entity, XDATA_PARCEL};

#[test]
fn test_dynamic_parcel_reactor_vertex_modification_and_isolation() {
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

    // Add a setback polyline inside the parcel on PC-SETBACK (Dimensions 14x44m, Area = 616 m², Perimeter = 116 m)
    // with accidental XDATA to test cleanup and isolation
    let mut setback_poly = LwPolyline::from_points(vec![
        Vector2::new(3.0, 3.0),
        Vector2::new(17.0, 3.0),
        Vector2::new(17.0, 47.0),
        Vector2::new(3.0, 47.0),
    ]);
    setback_poly.is_closed = true;
    let mut ent_setback = EntityType::LwPolyline(setback_poly);
    ent_setback.common_mut().layer = "PC-SETBACK".to_string();
    let mut rec_bad = codec::xdata::ExtendedDataRecord::new(XDATA_PARCEL);
    rec_bad.add_value(codec::xdata::XDataValue::String("LOT-TEST-1".to_string()));
    ent_setback.common_mut().extended_data.add_record(rec_bad);
    let setback_h = doc.add_entity(ent_setback).unwrap();

    // Verify is_parcel_layer and is_parcel_entity
    assert!(is_parcel_layer("PC-PARCEL"));
    assert!(is_parcel_layer("PC-RESIDENTIAL"));
    assert!(!is_parcel_layer("PC-SETBACK"));
    assert!(!is_parcel_layer("PC-PARCEL-LABEL"));
    assert!(!is_parcel_layer("PC-TABLE"));
    assert!(is_parcel_entity(doc.get_entity(poly_handle).unwrap()));
    assert!(!is_parcel_entity(doc.get_entity(setback_h).unwrap()));

    // 2. Add centroid labels (Name, Area)
    let t_name = Text::with_value("LOT-TEST-1", Vector3::new(10.0, 28.0, 0.0));
    let mut ent_name = EntityType::Text(t_name);
    ent_name.common_mut().layer = "PC-PARCEL-LABEL".to_string();
    doc.add_entity(ent_name).unwrap();

    let t_area = Text::with_value("1000.0 m² (0.247 ac)", Vector3::new(10.0, 25.0, 0.0));
    let mut ent_area = EntityType::Text(t_area);
    ent_area.common_mut().layer = "PC-PARCEL-LABEL".to_string();
    doc.add_entity(ent_area).unwrap();

    // 3. Add table row on PC-TABLE
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

    // 4. Modify vertex: stretch width from 20m to 30m!
    // New area = 30 * 50 = 1500 m², New perimeter = 2 * (30 + 50) = 160 m
    if let Some(EntityType::LwPolyline(lw)) = doc.get_entity_mut(poly_handle) {
        lw.vertices[1].location.x = 30.0;
        lw.vertices[2].location.x = 30.0;
    }

    // 5. Trigger dynamic synchronization
    let updated = ParcelData::sync_dynamic_parcels(&mut doc, None, None);
    assert!(updated > 0, "Expected parcels to be updated");

    // 6. Verify PC-SETBACK was cleaned and ignored
    let setback_ent = doc.get_entity(setback_h).unwrap();
    assert!(
        setback_ent.common().extended_data.get_record(XDATA_PARCEL).is_none(),
        "Accidental XDATA on PC-SETBACK should be purged"
    );

    // 7. Verify Table Row was updated to parcel's true geometry (1500.00 and 160.00)
    if let Some(EntityType::Text(t)) = doc.get_entity(col1_h) {
        assert_eq!(t.value, "1500.00", "Table area should update dynamically to 1500.00, not setback area");
    } else {
        panic!("Col 1 entity not found");
    }

    if let Some(EntityType::Text(t)) = doc.get_entity(col3_h) {
        assert_eq!(t.value, "160.00", "Table perimeter should update dynamically to 160.00, not setback perimeter");
    } else {
        panic!("Col 3 entity not found");
    }

    // 8. Verify Labels were updated dynamically & perimeter was synthesized!
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
fn test_al_waha_masterplan_real_drawing_dynamic_reactor() {
    let dwg_path = "projects/smart_village_masterplan/Al_Waha_Masterplan_Clean.dwg";
    let Ok(bytes) = std::fs::read(dwg_path) else {
        eprintln!("Skipping test: {} not found", dwg_path);
        return;
    };
    let mut doc = OpenCADStudio::io::load_bytes("Al_Waha_Masterplan_Clean.dwg", bytes)
        .expect("Failed to load Al_Waha_Masterplan_Clean.dwg");

    // 1. Sync dynamic parcels
    let count = ParcelData::sync_dynamic_parcels(&mut doc, None, None);
    assert!(count >= 14, "Expected at least 14 parcels synced, got {}", count);

    // 2. Discover parcels and verify LOT-101 and LOT-104
    let parcels = ParcelData::all_parcels(&doc);
    let lot_101 = parcels.iter().find(|p| p.name == "LOT-101").expect("LOT-101 not found");
    assert!((lot_101.area_2d - 618.75).abs() < 1.0, "LOT-101 area should be 618.75 m², got {}", lot_101.area_2d);
    assert!((lot_101.perimeter - 100.0).abs() < 1.0, "LOT-101 perimeter should be 100.0 m, got {}", lot_101.perimeter);

    let lot_104 = parcels.iter().find(|p| p.name == "LOT-104").expect("LOT-104 not found");
    assert!((lot_104.area_2d - 1000.0).abs() < 1.0, "LOT-104 area should be 1000.0 m², got {}", lot_104.area_2d);
    assert!((lot_104.perimeter - 130.0).abs() < 1.0, "LOT-104 perimeter should be 130.0 m, got {}", lot_104.perimeter);

    // 3. Verify Table Row for LOT-101 has 618.75 and 100.00
    let mut table_cells_101 = Vec::new();
    for ent in doc.entities() {
        if ent.common().layer.eq_ignore_ascii_case("PC-TABLE") {
            if let EntityType::Text(t) = ent {
                if (t.insertion_point.y - 174.308).abs() < 1.0 {
                    table_cells_101.push((t.insertion_point.x, t.value.clone()));
                }
            }
        }
    }
    table_cells_101.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    if table_cells_101.len() >= 4 {
        assert_eq!(table_cells_101[0].1, "LOT-101");
        assert_eq!(table_cells_101[1].1, "618.75", "Table area should be 618.75");
        assert_eq!(table_cells_101[3].1, "100.00", "Table perimeter should be 100.00");
    }

    // 4. Modify LOT-101 vertex: stretch x from 65.0 to 75.0 (add 10m to width: 37.5m x 22.5m = 843.75 m², perimeter = 120m)
    if let Some(EntityType::LwPolyline(lw)) = doc.get_entity_mut(lot_101.handle) {
        lw.vertices[1].location.x = 75.0;
        lw.vertices[2].location.x = 75.0;
    }

    let updated = ParcelData::sync_dynamic_parcels(&mut doc, Some(lot_101.handle), None);
    assert!(updated > 0);

    // Verify dynamic update in table
    for ent in doc.entities() {
        if ent.common().layer.eq_ignore_ascii_case("PC-TABLE") {
            if let EntityType::Text(t) = ent {
                if (t.insertion_point.y - 174.308).abs() < 1.0 && (t.insertion_point.x - 294.94).abs() < 1.0 {
                    assert_eq!(t.value, "843.75", "LOT-101 table area should update dynamically to 843.75");
                }
                if (t.insertion_point.y - 174.308).abs() < 1.0 && (t.insertion_point.x - 339.94).abs() < 1.0 {
                    assert_eq!(t.value, "120.00", "LOT-101 table perimeter should update dynamically to 120.00");
                }
            }
        }
    }
}


#[test]
fn test_swing_line_edit_between_adjacent_parcels() {
    use OpenCADStudio::app::commands::parcel_interactive::{union_two_parcels, SwingLineEditCommand};

    // 1. Two adjacent rectangular parcels:
    // Lot 1: [0,0] to [20,0], [20,50], [0,50] (Area = 1000 m²)
    // Lot 2: [20,0] to [40,0], [40,50], [20,50] (Area = 1000 m²)
    // Shared lot line: [20,0] to [20,50]
    let lot1_pts = vec![[0.0, 0.0], [20.0, 0.0], [20.0, 50.0], [0.0, 50.0]];
    let lot2_pts = vec![[20.0, 0.0], [40.0, 0.0], [40.0, 50.0], [20.0, 50.0]];

    // Verify union of adjacent parcels
    let union_pts = union_two_parcels(&lot1_pts, &lot2_pts, 0.1).expect("Parcels should union");
    assert_eq!(union_pts.len(), 6);
    let total_area = OpenCADStudio::app::commands::parcel_interactive::shoelace_area(&union_pts);
    assert!((total_area - 2000.0).abs() < 1e-4);

    // 2. Setup SwingLineEditCommand with target area = 1250 m² for Lot 1
    let mut cmd = SwingLineEditCommand::new(1250.0, 10.0);
    cmd.target_name = "LOT-1".to_string();
    cmd.target_vertices = lot1_pts;
    cmd.neighbor_name = "LOT-2".to_string();
    cmd.neighbor_vertices = lot2_pts;

    // Pivot at rear boundary [20.0, 50.0]
    cmd.pivot_point = Some([20.0, 50.0]);
    // Frontage along y=0 from [0.0, 0.0] to [40.0, 0.0]
    cmd.frontage_start = Some([0.0, 0.0]);
    cmd.frontage_end = Some([40.0, 0.0]);

    // Solve swing edit
    let res = cmd.solve_swing_edit();
    assert!(res.is_ok(), "Swing line edit should solve successfully");

    let sol = cmd.solution.expect("Solution should exist");
    // Target area should be 1250 m²
    assert!((sol.target_area - 1250.0).abs() < 1.0, "Target area should be ~1250 m², got {}", sol.target_area);
    // Remainder neighbor area should be 750 m² (2000 - 1250)
    assert!((sol.remainder_area - 750.0).abs() < 1.0, "Remainder area should be ~750 m², got {}", sol.remainder_area);
}

#[test]
fn test_subdivide_parcel_deletes_original_and_texts_and_creates_new_labels() {
    let mut doc = CadDocument::new();

    // 1. Create a parent rectangular parcel: 20x50m (Area = 1000 m², Perimeter = 140 m)
    let mut poly = LwPolyline::from_points(vec![
        Vector2::new(0.0, 0.0),
        Vector2::new(20.0, 0.0),
        Vector2::new(20.0, 50.0),
        Vector2::new(0.0, 50.0),
    ]);
    poly.is_closed = true;
    let mut ent_poly = EntityType::LwPolyline(poly);
    ent_poly.common_mut().layer = "PC-PARCEL".to_string();
    let mut rec = codec::xdata::ExtendedDataRecord::new(XDATA_PARCEL);
    rec.add_value(codec::xdata::XDataValue::String("LOT-ORIGINAL".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("Site 1".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("100".to_string()));
    ent_poly.common_mut().extended_data.add_record(rec);
    let parent_h = doc.add_entity(ent_poly).unwrap();

    // 2. Add original centroid labels for LOT-ORIGINAL
    let t_name = Text::with_value("LOT-ORIGINAL", Vector3::new(10.0, 28.0, 0.0));
    let mut ent_name = EntityType::Text(t_name);
    ent_name.common_mut().layer = "PC-PARCEL-LABEL".to_string();
    let mut rec_lbl1 = codec::xdata::ExtendedDataRecord::new("OPENCAD_PARCEL_LABEL");
    rec_lbl1.add_value(codec::xdata::XDataValue::String("LOT-ORIGINAL".to_string()));
    ent_name.common_mut().extended_data.add_record(rec_lbl1);
    let name_h = doc.add_entity(ent_name).unwrap();

    let t_area = Text::with_value("1000.0 m² (0.247 ac)", Vector3::new(10.0, 25.0, 0.0));
    let mut ent_area = EntityType::Text(t_area);
    ent_area.common_mut().layer = "PC-PARCEL-LABEL".to_string();
    let mut rec_lbl2 = codec::xdata::ExtendedDataRecord::new("OPENCAD_PARCEL_LABEL");
    rec_lbl2.add_value(codec::xdata::XDataValue::String("LOT-ORIGINAL".to_string()));
    ent_area.common_mut().extended_data.add_record(rec_lbl2);
    let area_h = doc.add_entity(ent_area).unwrap();

    let t_perim = Text::with_value("P: 140.00 m", Vector3::new(10.0, 22.0, 0.0));
    let mut ent_perim = EntityType::Text(t_perim);
    ent_perim.common_mut().layer = "PC-PARCEL-LABEL".to_string();
    let mut rec_lbl3 = codec::xdata::ExtendedDataRecord::new("OPENCAD_PARCEL_LABEL");
    rec_lbl3.add_value(codec::xdata::XDataValue::String("LOT-ORIGINAL".to_string()));
    ent_perim.common_mut().extended_data.add_record(rec_lbl3);
    let perim_h = doc.add_entity(ent_perim).unwrap();

    // 3. Add table row for LOT-ORIGINAL on PC-TABLE
    let mut ent_tab_name = EntityType::Text(Text::with_value("LOT-ORIGINAL", Vector3::new(100.0, 50.0, 0.0)));
    ent_tab_name.common_mut().layer = "PC-TABLE".to_string();
    let tab_name_h = doc.add_entity(ent_tab_name).unwrap();

    let mut ent_tab_area = EntityType::Text(Text::with_value("1000.00", Vector3::new(115.0, 50.0, 0.0)));
    ent_tab_area.common_mut().layer = "PC-TABLE".to_string();
    let tab_area_h = doc.add_entity(ent_tab_area).unwrap();

    let mut ent_tab_ac = EntityType::Text(Text::with_value("0.247", Vector3::new(130.0, 50.0, 0.0)));
    ent_tab_ac.common_mut().layer = "PC-TABLE".to_string();
    let tab_ac_h = doc.add_entity(ent_tab_ac).unwrap();

    let mut ent_tab_perim = EntityType::Text(Text::with_value("140.00", Vector3::new(145.0, 50.0, 0.0)));
    ent_tab_perim.common_mut().layer = "PC-TABLE".to_string();
    let tab_perim_h = doc.add_entity(ent_tab_perim).unwrap();

    // Verify initial state
    assert_eq!(ParcelData::all_parcels(&doc).len(), 1);

    // 4. Subdivide LOT-ORIGINAL into two child lots:
    // Child A: 20x25m from y=0 to y=25 (Area = 500 m², Perimeter = 90 m)
    // Child B: 20x25m from y=25 to y=50 (Area = 500 m², Perimeter = 90 m)
    // Erase parent parcel polyline
    doc.remove_entity(parent_h).unwrap();

    let mut child_a_poly = LwPolyline::from_points(vec![
        Vector2::new(0.0, 0.0),
        Vector2::new(20.0, 0.0),
        Vector2::new(20.0, 25.0),
        Vector2::new(0.0, 25.0),
    ]);
    child_a_poly.is_closed = true;
    let mut ent_child_a = EntityType::LwPolyline(child_a_poly);
    ent_child_a.common_mut().layer = "PC-PARCEL".to_string();
    let mut rec_a = codec::xdata::ExtendedDataRecord::new(XDATA_PARCEL);
    rec_a.add_value(codec::xdata::XDataValue::String("LOT-CHILD-1".to_string()));
    rec_a.add_value(codec::xdata::XDataValue::String("Site 1".to_string()));
    rec_a.add_value(codec::xdata::XDataValue::String("101".to_string()));
    ent_child_a.common_mut().extended_data.add_record(rec_a);
    let _child_a_h = doc.add_entity(ent_child_a).unwrap();

    let mut child_b_poly = LwPolyline::from_points(vec![
        Vector2::new(0.0, 25.0),
        Vector2::new(20.0, 25.0),
        Vector2::new(20.0, 50.0),
        Vector2::new(0.0, 50.0),
    ]);
    child_b_poly.is_closed = true;
    let mut ent_child_b = EntityType::LwPolyline(child_b_poly);
    ent_child_b.common_mut().layer = "PC-PARCEL".to_string();
    let mut rec_b = codec::xdata::ExtendedDataRecord::new(XDATA_PARCEL);
    rec_b.add_value(codec::xdata::XDataValue::String("LOT-CHILD-2".to_string()));
    rec_b.add_value(codec::xdata::XDataValue::String("Site 1".to_string()));
    rec_b.add_value(codec::xdata::XDataValue::String("102".to_string()));
    ent_child_b.common_mut().extended_data.add_record(rec_b);
    let _child_b_h = doc.add_entity(ent_child_b).unwrap();

    // 5. Run sync_dynamic_parcels (which runs automatically on any parcel subdivision/split commit)
    let count = ParcelData::sync_dynamic_parcels(&mut doc, None, None);
    assert!(count > 0, "Reactor should have synchronized child parcels and purged parent orphans");

    // 6. VERIFY:
    // A) Parent parcel polyline is deleted
    assert!(doc.get_entity(parent_h).is_none());

    // B) Original parent labels are strictly deleted!
    assert!(doc.get_entity(name_h).is_none(), "Original parent name label must be deleted");
    assert!(doc.get_entity(area_h).is_none(), "Original parent area label must be deleted");
    assert!(doc.get_entity(perim_h).is_none(), "Original parent perimeter label must be deleted");

    // C) Original parent table row is strictly deleted!
    assert!(doc.get_entity(tab_name_h).is_none(), "Original parent table row name must be deleted");
    assert!(doc.get_entity(tab_area_h).is_none(), "Original parent table row area must be deleted");
    assert!(doc.get_entity(tab_ac_h).is_none(), "Original parent table row acres must be deleted");
    assert!(doc.get_entity(tab_perim_h).is_none(), "Original parent table row perimeter must be deleted");

    // D) No leftover text entity in doc mentions "LOT-ORIGINAL"
    for ent in doc.entities() {
        match ent {
            EntityType::Text(t) => assert!(!t.value.contains("LOT-ORIGINAL"), "Found lingering text: {}", t.value),
            EntityType::MText(m) => assert!(!m.value.contains("LOT-ORIGINAL"), "Found lingering mtext: {}", m.value),
            _ => {}
        }
    }

    // E) Both child parcels exist with their own new centroid labels
    let parcels = ParcelData::all_parcels(&doc);
    assert_eq!(parcels.len(), 2, "Only the 2 new child parcels must exist");
    assert!(parcels.iter().any(|p| p.name == "LOT-CHILD-1"));
    assert!(parcels.iter().any(|p| p.name == "LOT-CHILD-2"));

    let mut found_child_1_label = false;
    let mut found_child_2_label = false;
    for ent in doc.entities() {
        if ent.common().layer == "PC-PARCEL-LABEL" {
            let val = match ent {
                EntityType::Text(t) => t.value.as_str(),
                EntityType::MText(m) => m.value.as_str(),
                _ => "",
            };
            if val.contains("LOT-CHILD-1") {
                found_child_1_label = true;
            }
            if val.contains("LOT-CHILD-2") {
                found_child_2_label = true;
            }
        }
    }
    assert!(found_child_1_label, "New child parcel LOT-CHILD-1 must have its own centroid label");
    assert!(found_child_2_label, "New child parcel LOT-CHILD-2 must have its own centroid label");
}


#[test]
fn test_parcel_area_and_segment_label_styles_automatic_generation() {
    let mut doc = CadDocument::new();

    // 1. Create a 30m x 40m lot with styles:
    // area_label_style = "Parcel Name & Area"
    // line_label_style = "Bearing over Distance"
    // auto_add_segment_labels = true
    // segment_layer = "C-PROP-LINE"
    let mut poly = LwPolyline::from_points(vec![
        Vector2::new(0.0, 0.0),
        Vector2::new(30.0, 0.0),
        Vector2::new(30.0, 40.0),
        Vector2::new(0.0, 40.0),
    ]);
    poly.is_closed = true;
    let mut ent = EntityType::LwPolyline(poly);
    ent.common_mut().layer = "C-PROP".to_string();

    let mut rec = codec::xdata::ExtendedDataRecord::new(XDATA_PARCEL);
    rec.add_value(codec::xdata::XDataValue::String("LOT-201".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("Site 1".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("201".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("Commercial".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("Owner A".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("D-101".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("TAX-01".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("Parcel Name & Area".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("Bearing over Distance".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("Distance over Radius".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("true".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("C-PROP-LINE".to_string()));
    ent.common_mut().extended_data.add_record(rec);

    let poly_h = doc.add_entity(ent).unwrap();

    // 2. Trigger dynamic synchronization
    let updated = ParcelData::sync_dynamic_parcels(&mut doc, Some(poly_h), None);
    assert!(updated > 0, "Parcel labels must be synchronized");

    // 3. Inspect Centroid Labels on PC-PARCEL-LABEL
    let centroid_labels: Vec<String> = doc.entities().filter_map(|e| {
        if e.common().layer == "PC-PARCEL-LABEL" {
            match e {
                EntityType::MText(m) => Some(m.value.clone()),
                EntityType::Text(t) => Some(t.value.clone()),
                _ => None,
            }
        } else {
            None
        }
    }).collect();

    // Under "Parcel Name & Area", there must be 2 lines: Name and Area (no perimeter line)
    assert_eq!(centroid_labels.len(), 2, "Parcel Name & Area style must generate exactly 2 centroid lines: {:?}", centroid_labels);
    assert!(centroid_labels.iter().any(|v| v.contains("LOT-201")), "Must have Name label");
    assert!(centroid_labels.iter().any(|v| v.contains("1200.0 m²")), "Must have Area label (1200.0 m²)");
    assert!(!centroid_labels.iter().any(|v| v.starts_with("P:")), "Must NOT have Perimeter label under Parcel Name & Area");

    // 4. Inspect Segment Labels on C-PROP-LINE
    let segment_labels: Vec<(String, String)> = doc.entities().filter_map(|e| {
        if e.common().layer == "C-PROP-LINE" {
            let val = match e {
                EntityType::MText(m) => m.value.clone(),
                EntityType::Text(t) => t.value.clone(),
                _ => String::new(),
            };
            let tag = e.common().extended_data.get_record("OPENCAD_PARCEL_SEGMENT_LABEL")
                .and_then(|r| r.values.first())
                .and_then(|v| match v {
                    codec::xdata::XDataValue::String(s) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            if !val.is_empty() {
                Some((val, tag))
            } else {
                None
            }
        } else {
            None
        }
    }).collect();

    assert_eq!(segment_labels.len(), 4, "A 4-sided rectangular lot must generate 4 segment labels: {:?}", segment_labels);
    for (val, tag) in &segment_labels {
        assert_eq!(tag, "LOT-201", "Segment label must be tagged with parcel name");
        assert!(val.contains('m'), "Segment label must contain distance: {}", val);
        assert!(val.contains('\u{b0}') || val.contains('N') || val.contains('S'), "Segment label must contain bearing: {}", val);
    }
}

#[test]
fn test_creating_multiple_parcels_preserves_all_centroid_and_segment_labels() {
    let mut doc = CadDocument::new();

    // 1. Verify next_available_lot_number on empty doc returns 101
    assert_eq!(ParcelData::next_available_lot_number(&doc), 101);

    // 2. Create Parcel 1 (LOT-101) from (0,0) to (30, 40)
    let mut poly1 = LwPolyline::from_points(vec![
        Vector2::new(0.0, 0.0),
        Vector2::new(30.0, 0.0),
        Vector2::new(30.0, 40.0),
        Vector2::new(0.0, 40.0),
    ]);
    poly1.is_closed = true;
    let mut ent1 = EntityType::LwPolyline(poly1);
    ent1.common_mut().layer = "C-PROP".to_string();

    let mut rec1 = codec::xdata::ExtendedDataRecord::new(XDATA_PARCEL);
    rec1.add_value(codec::xdata::XDataValue::String("LOT-101".to_string()));
    rec1.add_value(codec::xdata::XDataValue::String("Site 1".to_string()));
    rec1.add_value(codec::xdata::XDataValue::String("101".to_string()));
    rec1.add_value(codec::xdata::XDataValue::String("Single-Family".to_string()));
    rec1.add_value(codec::xdata::XDataValue::String("Owner 1".to_string()));
    rec1.add_value(codec::xdata::XDataValue::String("D-101".to_string()));
    rec1.add_value(codec::xdata::XDataValue::String("TAX-01".to_string()));
    rec1.add_value(codec::xdata::XDataValue::String("Parcel Name, Area & Perimeter".to_string()));
    rec1.add_value(codec::xdata::XDataValue::String("Bearing over Distance".to_string()));
    rec1.add_value(codec::xdata::XDataValue::String("Distance over Radius".to_string()));
    rec1.add_value(codec::xdata::XDataValue::String("true".to_string()));
    rec1.add_value(codec::xdata::XDataValue::String("C-PROP-LINE".to_string()));
    ent1.common_mut().extended_data.add_record(rec1);
    let _h1 = doc.add_entity(ent1).unwrap();

    // Sync Dynamic Parcels
    let _ = ParcelData::sync_dynamic_parcels(&mut doc, None, None);

    // Verify Parcel 1 has 3 centroid labels and 4 segment labels
    let labels_p1: Vec<String> = doc.entities().filter_map(|e| {
        if e.common().layer == "PC-PARCEL-LABEL" {
            match e {
                EntityType::MText(m) => Some(m.value.clone()),
                EntityType::Text(t) => Some(t.value.clone()),
                _ => None,
            }
        } else {
            None
        }
    }).collect();
    assert_eq!(labels_p1.len(), 3, "LOT-101 should have 3 centroid labels");
    assert!(labels_p1.iter().any(|v| v.contains("LOT-101")));

    let segs_p1: Vec<String> = doc.entities().filter_map(|e| {
        if e.common().layer == "C-PROP-LINE" {
            let seg_tag = e.common().extended_data.get_record("OPENCAD_PARCEL_SEGMENT_LABEL")
                .and_then(|r| r.values.first())
                .and_then(|v| match v {
                    codec::xdata::XDataValue::String(s) => Some(s.clone()),
                    _ => None,
                });
            if seg_tag.as_deref() == Some("LOT-101") {
                match e {
                    EntityType::MText(m) => Some(m.value.clone()),
                    EntityType::Text(t) => Some(t.value.clone()),
                    _ => None,
                }
            } else {
                None
            }
        } else {
            None
        }
    }).collect();
    assert_eq!(segs_p1.len(), 4, "LOT-101 should have 4 segment labels");

    // 3. Now verify next_available_lot_number returns 102
    assert_eq!(ParcelData::next_available_lot_number(&doc), 102);

    // 4. Create Parcel 2 (LOT-102) adjacent from (30,0) to (60, 40)
    let mut poly2 = LwPolyline::from_points(vec![
        Vector2::new(30.0, 0.0),
        Vector2::new(60.0, 0.0),
        Vector2::new(60.0, 40.0),
        Vector2::new(30.0, 40.0),
    ]);
    poly2.is_closed = true;
    let mut ent2 = EntityType::LwPolyline(poly2);
    ent2.common_mut().layer = "C-PROP".to_string();

    let mut rec2 = codec::xdata::ExtendedDataRecord::new(XDATA_PARCEL);
    rec2.add_value(codec::xdata::XDataValue::String("LOT-102".to_string()));
    rec2.add_value(codec::xdata::XDataValue::String("Site 1".to_string()));
    rec2.add_value(codec::xdata::XDataValue::String("102".to_string()));
    rec2.add_value(codec::xdata::XDataValue::String("Single-Family".to_string()));
    rec2.add_value(codec::xdata::XDataValue::String("Owner 2".to_string()));
    rec2.add_value(codec::xdata::XDataValue::String("D-102".to_string()));
    rec2.add_value(codec::xdata::XDataValue::String("TAX-02".to_string()));
    rec2.add_value(codec::xdata::XDataValue::String("Parcel Name, Area & Perimeter".to_string()));
    rec2.add_value(codec::xdata::XDataValue::String("Bearing over Distance".to_string()));
    rec2.add_value(codec::xdata::XDataValue::String("Distance over Radius".to_string()));
    rec2.add_value(codec::xdata::XDataValue::String("true".to_string()));
    rec2.add_value(codec::xdata::XDataValue::String("C-PROP-LINE".to_string()));
    ent2.common_mut().extended_data.add_record(rec2);
    let _h2 = doc.add_entity(ent2).unwrap();

    // Sync Dynamic Parcels for entire document (as happens on commit)
    let _ = ParcelData::sync_dynamic_parcels(&mut doc, None, None);

    // 5. CRITICAL VERIFICATION:
    // Verify that LOT-101 STILL HAS its 3 centroid labels!
    let labels_all: Vec<String> = doc.entities().filter_map(|e| {
        if e.common().layer == "PC-PARCEL-LABEL" {
            match e {
                EntityType::MText(m) => Some(m.value.clone()),
                EntityType::Text(t) => Some(t.value.clone()),
                _ => None,
            }
        } else {
            None
        }
    }).collect();

    assert!(labels_all.iter().any(|v| v.contains("LOT-101")), "LOT-101 centroid label must NOT disappear when LOT-102 is created!");
    assert!(labels_all.iter().any(|v| v.contains("LOT-102")), "LOT-102 centroid label must be present!");
    assert_eq!(labels_all.len(), 6, "Must have 3 centroid labels for LOT-101 and 3 for LOT-102 (total 6)");

    // Verify segment labels:
    // Shared edge (x=30 from y=0 to y=40) is deduplicated, outer edges labeled.
    let seg_tags: Vec<String> = doc.entities().filter_map(|e| {
        if e.common().layer == "C-PROP-LINE" {
            e.common().extended_data.get_record("OPENCAD_PARCEL_SEGMENT_LABEL")
                .and_then(|r| r.values.first())
                .and_then(|v| match v {
                    codec::xdata::XDataValue::String(s) => Some(s.clone()),
                    _ => None,
                })
        } else {
            None
        }
    }).collect();

    assert!(seg_tags.iter().any(|t| t == "LOT-101"), "LOT-101 must still have its segment labels!");
    assert!(seg_tags.iter().any(|t| t == "LOT-102"), "LOT-102 must have its segment labels!");
    assert_eq!(seg_tags.len(), 7, "Total segment labels across both adjacent parcels must be 7 (shared edge deduplicated)");

    // 6. Next available lot number should now be 103
    assert_eq!(ParcelData::next_available_lot_number(&doc), 103);
}

#[test]
fn test_resolve_parcel_from_centroid_and_segment_labels() {
    let mut doc = CadDocument::new();

    let mut poly = LwPolyline::from_points(vec![
        Vector2::new(0.0, 0.0),
        Vector2::new(50.0, 0.0),
        Vector2::new(50.0, 30.0),
        Vector2::new(0.0, 30.0),
    ]);
    poly.is_closed = true;
    let mut ent = EntityType::LwPolyline(poly);
    ent.common_mut().layer = "C-PROP".to_string();

    let mut rec = codec::xdata::ExtendedDataRecord::new(XDATA_PARCEL);
    rec.add_value(codec::xdata::XDataValue::String("LOT-500".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("Site 1".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("500".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("Single-Family".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("Owner 500".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("D-500".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("TAX-500".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("Parcel Name, Area & Perimeter".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("Bearing over Distance".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("Distance over Radius".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("true".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("C-PROP-LINE".to_string()));
    ent.common_mut().extended_data.add_record(rec);
    let poly_h = doc.add_entity(ent).unwrap();

    let _ = ParcelData::sync_dynamic_parcels(&mut doc, None, None);

    // 1. Centroid label resolution
    let centroid_label_h = doc.entities().find_map(|e| {
        if e.common().layer == "PC-PARCEL-LABEL" {
            Some(e.common().handle)
        } else {
            None
        }
    }).expect("Centroid label must exist");

    let resolved_from_centroid = ParcelData::resolve_parcel_from_label(&doc, centroid_label_h);
    assert_eq!(resolved_from_centroid, Some(poly_h), "Clicking on centroid label must resolve to the parcel polyline handle");

    // 2. Segment label resolution
    let segment_label_h = doc.entities().find_map(|e| {
        if e.common().layer == "C-PROP-LINE" {
            Some(e.common().handle)
        } else {
            None
        }
    }).expect("Segment label must exist");

    let resolved_from_segment = ParcelData::resolve_parcel_from_label(&doc, segment_label_h);
    assert_eq!(resolved_from_segment, Some(poly_h), "Clicking on boundary segment label must resolve to the parcel polyline handle");

    // 3. Unrelated entity returns None
    let mut line = codec::entities::Line::new();
    line.start = Vector3::ZERO;
    line.end = Vector3::new(1.0, 1.0, 0.0);
    let unrelated_line = EntityType::Line(line);
    let unrelated_h = doc.add_entity(unrelated_line).unwrap();
    assert_eq!(ParcelData::resolve_parcel_from_label(&doc, unrelated_h), None, "Unrelated entity must not resolve to a parcel");
}

#[test]
fn test_modify_styles_via_update_entity_xdata_full_and_dynamic_sync() {
    let mut doc = CadDocument::new();

    let mut poly = LwPolyline::from_points(vec![
        Vector2::new(0.0, 0.0),
        Vector2::new(40.0, 0.0),
        Vector2::new(40.0, 25.0),
        Vector2::new(0.0, 25.0),
    ]);
    poly.is_closed = true;
    let mut ent = EntityType::LwPolyline(poly);
    ent.common_mut().layer = "C-PROP".to_string();

    let mut rec = codec::xdata::ExtendedDataRecord::new(XDATA_PARCEL);
    rec.add_value(codec::xdata::XDataValue::String("LOT-STYLE-TEST".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("Site 1".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("600".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("Single-Family".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("Owner".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("Deed".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("Tax".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("Parcel Name, Area & Perimeter".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("Bearing over Distance".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("Distance over Radius".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("true".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("C-PROP-LINE".to_string()));
    ent.common_mut().extended_data.add_record(rec);
    let poly_h = doc.add_entity(ent).unwrap();

    let _ = ParcelData::sync_dynamic_parcels(&mut doc, None, None);

    // Initial check: 3 centroid labels, 4 segment labels
    let count_centroid = doc.entities().filter(|e| e.common().layer == "PC-PARCEL-LABEL").count();
    let count_segs = doc.entities().filter(|e| e.common().layer == "C-PROP-LINE").count();
    assert_eq!(count_centroid, 3, "Initial area style has 3 lines");
    assert_eq!(count_segs, 4, "Initial segment labels has 4 lines");

    // 1. Modify style via update_entity_xdata_full to "Name Only" and line_label_style to "Distance Only"
    ParcelData::update_entity_xdata_full(
        &mut doc,
        poly_h,
        "LOT-STYLE-TEST",
        600,
        "Site 1",
        "Commercial",
        "Owner Updated",
        "Deed",
        "Tax",
        "C-PROP",
        "Name Only",
        "Distance Only",
        "Distance over Radius",
        true,
        "C-PROP-LINE",
    );

    let _ = ParcelData::sync_dynamic_parcels(&mut doc, Some(poly_h), None);

    // Check centroid labels: exactly 1 line with parcel name
    let centroid_vals: Vec<String> = doc.entities().filter_map(|e| {
        if e.common().layer == "PC-PARCEL-LABEL" {
            match e {
                EntityType::MText(m) => Some(m.value.clone()),
                EntityType::Text(t) => Some(t.value.clone()),
                _ => None,
            }
        } else {
            None
        }
    }).collect();
    assert_eq!(centroid_vals.len(), 1, "Name Only style must produce exactly 1 centroid line");
    assert_eq!(centroid_vals[0], "LOT-STYLE-TEST");

    // Check segment labels: show distance only (e.g. "40.00m", "25.00m") and no bearing
    let seg_vals: Vec<String> = doc.entities().filter_map(|e| {
        if e.common().layer == "C-PROP-LINE" {
            match e {
                EntityType::MText(m) => Some(m.value.clone()),
                EntityType::Text(t) => Some(t.value.clone()),
                _ => None,
            }
        } else {
            None
        }
    }).collect();
    assert_eq!(seg_vals.len(), 4, "Still 4 segments");
    for v in &seg_vals {
        assert!(v.ends_with('m'), "Distance only must end with m: {}", v);
        assert!(!v.contains('N') && !v.contains('S'), "Distance only must not contain bearing quadrant: {}", v);
    }

    // 2. Turn off auto_add_segment_labels
    ParcelData::update_entity_xdata_full(
        &mut doc,
        poly_h,
        "LOT-STYLE-TEST",
        600,
        "Site 1",
        "Commercial",
        "Owner Updated",
        "Deed",
        "Tax",
        "C-PROP",
        "Name Only",
        "None",
        "None",
        false,
        "C-PROP-LINE",
    );
    let _ = ParcelData::sync_dynamic_parcels(&mut doc, Some(poly_h), None);
    let count_segs_after_disable = doc.entities().filter(|e| e.common().layer == "C-PROP-LINE").count();
    assert_eq!(count_segs_after_disable, 0, "Disabling segment labels must remove all segment labels for this parcel");
}

#[test]
fn test_parcel_line_table_interactive_command_and_drawing() {
    use OpenCADStudio::app::commands::parcel_interactive::ParcelLineTableCommand;
    use OpenCADStudio::command::{CadCommand, CmdResult};
    use OpenCADStudio::app::commands::civil_parcel::draw_parcel_line_table;
    use glam::DVec3;

    let mut doc = CadDocument::new();

    // 1. Create a 30x40m parcel
    let mut poly = LwPolyline::from_points(vec![
        Vector2::new(10.0, 10.0),
        Vector2::new(40.0, 10.0),
        Vector2::new(40.0, 50.0),
        Vector2::new(10.0, 50.0),
    ]);
    poly.is_closed = true;
    let mut ent_poly = EntityType::LwPolyline(poly);
    ent_poly.common_mut().layer = "PC-PARCEL".to_string();

    let mut rec = codec::xdata::ExtendedDataRecord::new(XDATA_PARCEL);
    rec.add_value(codec::xdata::XDataValue::String("PARCEL-LINE-TBL-TEST".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("Site 1".to_string()));
    rec.add_value(codec::xdata::XDataValue::String("101".to_string()));
    ent_poly.common_mut().extended_data.add_record(rec);

    let poly_h = doc.add_entity(ent_poly).unwrap();
    let parcel_ent = doc.get_entity(poly_h).unwrap().clone();
    let parcel_data = ParcelData::from_document_entity(&doc, poly_h).expect("ParcelData must be parsed");

    // 2. Test ParcelLineTableCommand Interactive Workflow
    let mut cmd = ParcelLineTableCommand::new();
    assert!(cmd.needs_entity_pick(), "Initial step must request entity pick");
    assert!(cmd.prompt().contains("Select parcel on canvas"));

    // Step 1: Pick the parcel entity
    cmd.inject_picked_entity(parcel_ent);
    let res1 = cmd.on_entity_pick(poly_h, DVec3::new(10.0, 10.0, 0.0));
    assert!(matches!(res1, CmdResult::NeedPoint));
    assert!(!cmd.needs_entity_pick(), "Step 2 should not need entity pick");
    assert!(cmd.prompt().contains("Specify insertion point"));
    assert!(cmd.prompt().contains("PARCEL-LINE-TBL-TEST"));

    // Check preview wires at mouse hover point
    let preview_wires = cmd.on_preview_wires(DVec3::new(150.0, 200.0, 0.0));
    assert!(!preview_wires.is_empty(), "Must generate table bounding box preview wires");

    // Step 2: Click insertion point
    let res2 = cmd.on_point(DVec3::new(150.0, 200.0, 0.0));
    if let CmdResult::Dispatch(cmd_str) = res2 {
        assert!(cmd_str.contains("PC_TABLE_LINES"), "Dispatched command must be PC_TABLE_LINES: {}", cmd_str);
        assert!(cmd_str.contains("PARCEL-LINE-TBL-TEST"), "Command must contain parcel name: {}", cmd_str);
        assert!(cmd_str.contains("150.00"), "Command must contain X coordinate: {}", cmd_str);
        assert!(cmd_str.contains("200.00"), "Command must contain Y coordinate: {}", cmd_str);
    } else {
        panic!("Expected CmdResult::Dispatch");
    }

    // 3. Test draw_parcel_line_table output in CadDocument
    let before_count = doc.entities().count();
    draw_parcel_line_table(&mut doc, &parcel_data, [150.0, 200.0], 2.0);
    let after_count = doc.entities().count();
    assert!(after_count > before_count, "draw_parcel_line_table must add entities to doc");

    // Verify entities on PC-TABLE layer
    let table_texts: Vec<String> = doc.entities().filter_map(|e| {
        if e.common().layer == "PC-TABLE" {
            match e {
                EntityType::Text(t) => Some(t.value.clone()),
                EntityType::MText(m) => Some(m.value.clone()),
                _ => None,
            }
        } else {
            None
        }
    }).collect();

    // Verify title and headers
    assert!(table_texts.iter().any(|t| t.contains("LINE TABLE - PARCEL-LINE-TBL-TEST")), "Table title must exist");
    assert!(table_texts.iter().any(|t| t == "Tag"), "Header 'Tag' must exist");
    assert!(table_texts.iter().any(|t| t == "Bearing"), "Header 'Bearing' must exist");
    assert!(table_texts.iter().any(|t| t == "Distance (m)"), "Header 'Distance (m)' must exist");

    // Verify line tags and segment distances
    assert!(table_texts.iter().any(|t| t == "L1"), "Segment L1 must exist");
    assert!(table_texts.iter().any(|t| t == "L2"), "Segment L2 must exist");
    assert!(table_texts.iter().any(|t| t == "L3"), "Segment L3 must exist");
    assert!(table_texts.iter().any(|t| t == "L4"), "Segment L4 must exist");
    assert!(table_texts.iter().any(|t| t == "30.000"), "Distance 30.000m must exist");
    assert!(table_texts.iter().any(|t| t == "40.000"), "Distance 40.000m must exist");

    // Verify grid lines
    let table_lines_count = doc.entities().filter(|e| {
        e.common().layer == "PC-TABLE" && matches!(e, EntityType::Line(_))
    }).count();
    assert!(table_lines_count >= 5, "Table grid must have boundary and divider lines, found {}", table_lines_count);
}

#[test]
fn test_site_topology_overlap_resolution() {
    let mut doc = CadDocument::new();

    // 1. Create LOT-101 (dimensions 30x20m: [0, 0] to [30, 20], Area = 600 m²) in Site 1
    let mut poly1 = LwPolyline::from_points(vec![
        Vector2::new(0.0, 0.0),
        Vector2::new(30.0, 0.0),
        Vector2::new(30.0, 20.0),
        Vector2::new(0.0, 20.0),
    ]);
    poly1.is_closed = true;
    let mut ent1 = EntityType::LwPolyline(poly1);
    ent1.common_mut().layer = "C-PROP".to_string();
    let mut rec1 = codec::xdata::ExtendedDataRecord::new(XDATA_PARCEL);
    rec1.add_value(codec::xdata::XDataValue::String("LOT-101".to_string()));
    rec1.add_value(codec::xdata::XDataValue::String("Site 1".to_string()));
    rec1.add_value(codec::xdata::XDataValue::String("101".to_string()));
    ent1.common_mut().extended_data.add_record(rec1);
    let h1 = doc.add_entity(ent1).unwrap();

    // 2. Create LOT-102 (dimensions 30x20m: [20, 0] to [50, 20], Area = 600 m²) in Site 1
    // Overlapping LOT-101 by 10x20m (200 m² overlap)!
    let mut poly2 = LwPolyline::from_points(vec![
        Vector2::new(20.0, 0.0),
        Vector2::new(50.0, 0.0),
        Vector2::new(50.0, 20.0),
        Vector2::new(20.0, 20.0),
    ]);
    poly2.is_closed = true;
    let mut ent2 = EntityType::LwPolyline(poly2);
    ent2.common_mut().layer = "C-PROP".to_string();
    let mut rec2 = codec::xdata::ExtendedDataRecord::new(XDATA_PARCEL);
    rec2.add_value(codec::xdata::XDataValue::String("LOT-102".to_string()));
    rec2.add_value(codec::xdata::XDataValue::String("Site 1".to_string()));
    rec2.add_value(codec::xdata::XDataValue::String("102".to_string()));
    ent2.common_mut().extended_data.add_record(rec2);
    let h2 = doc.add_entity(ent2).unwrap();

    // 3. Resolve overlaps with LOT-102 as cutter
    let resolved = ParcelData::resolve_site_topology_overlaps(&mut doc, Some(h2));
    assert_eq!(resolved, 1, "Must resolve 1 overlapping parcel");

    // 4. Verify LOT-101 was clipped:
    // Original LOT-101 was [0..30, 0..20] (600 m²).
    // LOT-102 took [20..50, 0..20].
    // LOT-101 remaining should be [0..20, 0..20] (400 m²)!
    let p1_after = ParcelData::from_document_entity(&doc, h1).expect("LOT-101 must exist");
    assert!((p1_after.area_2d - 400.0).abs() < 1.0, "LOT-101 area should be 400 m², got {}", p1_after.area_2d);

    let p2_after = ParcelData::from_document_entity(&doc, h2).expect("LOT-102 must exist");
    assert!((p2_after.area_2d - 600.0).abs() < 1.0, "LOT-102 area should be 600 m², got {}", p2_after.area_2d);

    // 5. Verify different sites are NOT clipped:
    // Create LOT-201 in Site 2 overlapping LOT-102
    let mut poly3 = LwPolyline::from_points(vec![
        Vector2::new(30.0, 0.0),
        Vector2::new(60.0, 0.0),
        Vector2::new(60.0, 20.0),
        Vector2::new(30.0, 20.0),
    ]);
    poly3.is_closed = true;
    let mut ent3 = EntityType::LwPolyline(poly3);
    ent3.common_mut().layer = "C-PROP".to_string();
    let mut rec3 = codec::xdata::ExtendedDataRecord::new(XDATA_PARCEL);
    rec3.add_value(codec::xdata::XDataValue::String("LOT-201".to_string()));
    rec3.add_value(codec::xdata::XDataValue::String("Site 2".to_string())); // Different Site!
    rec3.add_value(codec::xdata::XDataValue::String("201".to_string()));
    ent3.common_mut().extended_data.add_record(rec3);
    let h3 = doc.add_entity(ent3).unwrap();

    let resolved_diff_site = ParcelData::resolve_site_topology_overlaps(&mut doc, Some(h3));
    assert_eq!(resolved_diff_site, 0, "Different sites must NOT be clipped against each other");

    let p2_still = ParcelData::from_document_entity(&doc, h2).unwrap();
    assert!((p2_still.area_2d - 600.0).abs() < 1.0, "LOT-102 in Site 1 must retain full area when Site 2 overlaps");
}
