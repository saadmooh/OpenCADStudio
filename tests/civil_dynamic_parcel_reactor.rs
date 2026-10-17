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
