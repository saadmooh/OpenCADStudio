//! The Browser: a standing outline of what the drawing is built from.
//!
//! Three sections, each read straight from the document rather than from a
//! parallel model the panel would have to keep in sync:
//!
//! * **Origin planes** — the three world planes, as somewhere to start a
//!   sketch when the drawing is still empty and there is no face to pick.
//! * **Sketch** — the open sketch, if there is one.
//! * **Bodies** — every `Solid3D`, labelled with the operation that made it,
//!   recovered from the DWG solid history.
//!
//! What this panel deliberately is *not* is a parametric feature tree. The
//! history records how each solid was built, but nothing re-runs it, so
//! editing an earlier feature cannot rebuild the ones after it. Showing the
//! list as if it were editable history would promise exactly that. It is a
//! table of contents, and it is labelled as one.

use crate::app::commands::civil_surface::TinSurfaceData;
use crate::app::Message;
use crate::ui::dock::PanelId;
use codec::objects::SolidHistoryOperation;
use codec::{Document, EntityType, Handle};
use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Background, Border, Element, Fill, Theme};

/// The three world planes, with the argument `CREATESKETCH` takes for each.
const ORIGIN_PLANES: [(&str, &str); 3] = [("XY", "XY"), ("XZ", "XZ"), ("YZ", "YZ")];

/// Human name for the operation that produced a solid. `None` when the
/// drawing carries no history for it — a solid imported as plain geometry,
/// or one whose history was stripped on save.
fn operation_label(operation: &SolidHistoryOperation) -> &'static str {
    match operation {
        SolidHistoryOperation::Box(_) => "Box",
        SolidHistoryOperation::Wedge(_) => "Wedge",
        SolidHistoryOperation::Sphere(_) => "Sphere",
        SolidHistoryOperation::Cone(_) => "Cone",
        SolidHistoryOperation::Cylinder(_) => "Cylinder",
        SolidHistoryOperation::Torus(_) => "Torus",
        SolidHistoryOperation::Pyramid(_) => "Pyramid",
        SolidHistoryOperation::Sweep(_) => "Sweep",
        SolidHistoryOperation::Loft(_) => "Loft",
        SolidHistoryOperation::Extrusion(_) => "Extrude",
        SolidHistoryOperation::Revolve(_) => "Revolve",
        SolidHistoryOperation::Fillet(_) => "Fillet",
        SolidHistoryOperation::Chamfer(_) => "Chamfer",
        _ => "Solid",
    }
}

/// Every `Solid3D` in model space, with the label to show for it.
fn bodies(document: &Document) -> Vec<(Handle, String)> {
    document
        .entities()
        .filter(|entity| matches!(entity, EntityType::Solid3D(_)))
        .map(|entity| {
            let handle = entity.common().handle;
            let label =
                crate::scene::model::solid_history::primitive_property_operation(document, handle)
                    .as_ref()
                    .map_or("Solid", operation_label);
            (handle, format!("{label} {:X}", handle.value()))
        })
        .collect()
}

/// Every TIN Surface (`PolyfaceMesh` or `Mesh`) in model space.
fn surfaces(document: &Document) -> Vec<TinSurfaceData> {
    document
        .entities()
        .filter_map(TinSurfaceData::from_entity)
        .collect()
}

fn surface_card(surf: &TinSurfaceData) -> Element<'static, Message> {
    let handle_hex = format!("{:X}", surf.handle.value());
    let title = format!("{} [{}]", surf.name, handle_hex);

    let stats_text = format!(
        "Z: {:.1}..{:.1}m | Area: {:.0}m² | {} tris",
        surf.min_z,
        surf.max_z,
        surf.area_2d,
        surf.triangles.len()
    );

    let title_btn = button(
        row![
            text("📐").size(11),
            text(title).size(11.5),
        ]
        .spacing(4)
        .align_y(iced::Center)
    )
        .on_press(Message::OpenSurfaceProperties(surf.handle))
        .width(Fill)
        .padding([2, 4])
        .style(button::subtle);

    let btn_zoom = button(text("🔍 Zoom").size(10))
        .on_press(Message::Command(format!("SELECT {handle_hex}")))
        .width(Fill)
        .padding([3, 2])
        .style(button::secondary);

    let btn_props = button(text("⚙ Props").size(10))
        .on_press(Message::OpenSurfaceProperties(surf.handle))
        .width(Fill)
        .padding([3, 2])
        .style(button::primary);

    let btn_stats = button(text("📊 Stats").size(10))
        .on_press(Message::Command(format!("SURFACESTATS {handle_hex}")))
        .width(Fill)
        .padding([3, 2])
        .style(button::secondary);

    let btn_xml = button(text("📤 LandXML").size(10))
        .on_press(Message::Command(format!("SURFACELANDXML {handle_hex}")))
        .width(Fill)
        .padding([3, 2])
        .style(button::secondary);

    // Style toggles (Civil 3D Surface Styles)
    let btn_style_tri = button(text("🔺 Triangles").size(10))
        .on_press(Message::Command(format!("SURFACESTYLE {handle_hex} TRIANGLES")))
        .width(Fill)
        .padding([3, 2])
        .style(button::secondary);

    let btn_style_cont = button(text("〰 Contours").size(10))
        .on_press(Message::Command(format!("SURFACESTYLE {handle_hex} CONTOURS")))
        .width(Fill)
        .padding([3, 2])
        .style(button::secondary);

    let btn_style_both = button(text("🔲 Both").size(10))
        .on_press(Message::Command(format!("SURFACESTYLE {handle_hex} BOTH")))
        .width(Fill)
        .padding([3, 2])
        .style(button::secondary);

    // TIN Operations (Civil 3D Surface Edits)
    let btn_swap = button(text("🔄 Swap").size(10))
        .on_press(Message::Command(format!("SURFACESWAPEDGE {handle_hex}")))
        .width(Fill)
        .padding([3, 2])
        .style(button::secondary);

    let btn_raise = button(text("⬆ +1m").size(10))
        .on_press(Message::Command(format!("SURFACERAISELOWER {handle_hex} 1.0")))
        .width(Fill)
        .padding([3, 2])
        .style(button::secondary);

    let btn_lower = button(text("⬇ -1m").size(10))
        .on_press(Message::Command(format!("SURFACERAISELOWER {handle_hex} -1.0")))
        .width(Fill)
        .padding([3, 2])
        .style(button::secondary);

    let btn_del = button(text("✂ Del Tri").size(10))
        .on_press(Message::Command(format!("SURFACEDELETETRI {handle_hex}")))
        .width(Fill)
        .padding([3, 2])
        .style(button::secondary);

    let row1 = row![btn_zoom, btn_props, btn_stats, btn_xml].spacing(3).width(Fill);
    let row2 = row![btn_style_tri, btn_style_cont, btn_style_both].spacing(3).width(Fill);
    let row3 = row![btn_swap, btn_raise, btn_lower, btn_del].spacing(3).width(Fill);

    let card_content = column![
        title_btn,
        container(
            text(stats_text)
                .size(10)
                .style(crate::ui::style::common::muted_style)
        )
        .padding([0, 4]),
        row1,
        row2,
        row3,
    ]
    .spacing(3);

    container(card_content)
        .width(Fill)
        .padding([4, 6])
        .style(|theme: &Theme| container::Style {
            background: Some(Background::Color(theme.palette().background.weak.color)),
            border: Border {
                color: theme.palette().background.neutral.color,
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        })
        .into()
}

/// Feature Lines / Breaklines in model space.
fn featurelines(document: &Document) -> Vec<(Handle, String)> {
    document
        .entities()
        .filter(|entity| matches!(entity, EntityType::Polyline3D(_)))
        .map(|entity| {
            let handle = entity.common().handle;
            let layer = &entity.common().layer;
            let name = if let Some(stripped) = layer.strip_prefix("LS-FEATURELINE-") {
                stripped.to_string()
            } else if layer.starts_with("LS-DAYLIGHT") {
                "Daylight".to_string()
            } else {
                layer.clone()
            };
            (handle, format!("{name} [{:X}]", handle.value()))
        })
        .collect()
}

/// Survey Points in model space.
fn survey_points_count(document: &Document) -> usize {
    document
        .entities()
        .filter(|entity| matches!(entity, EntityType::Point(_)))
        .count()
}

fn section_header(label: String) -> Element<'static, Message> {
    container(text(label).size(11))
        .width(Fill)
        .padding([3, 6])
        .style(|theme: &Theme| container::Style {
            background: Some(Background::Color(theme.palette().background.weak.color)),
            ..Default::default()
        })
        .into()
}

/// A clickable row that runs `command`.
fn entry_row(label: String, command: String) -> Element<'static, Message> {
    button(text(label).size(12))
        .on_press(Message::Command(command))
        .width(Fill)
        .padding([3, 10])
        .style(button::subtle)
        .into()
}

/// A row that states something rather than doing something.
fn note_row(label: String) -> Element<'static, Message> {
    container(
        text(label)
            .size(11)
            .style(crate::ui::style::common::muted_style),
    )
    .width(Fill)
    .padding([3, 10])
    .into()
}

pub fn view<'a>(
    document: &'a Document,
    open_sketch: Option<&'a str>,
    width: f32,
    chrome: crate::ui::dock::Chrome,
) -> Element<'a, Message> {
    let title_bar = crate::ui::dock::title_bar(
        PanelId::Browser,
        "Prospector (Toolspace)".to_string(),
        chrome,
    );

    let mut tree = column![].spacing(1);

    // ── Prospector: Civil 3D Surfaces & Feature Lines ──────────────────
    let surfs = surfaces(document);
    tree = tree.push(section_header(format!("Prospector - Surfaces ({})", surfs.len())));

    let create_surf_btn = button(
        row![
            text("➕").size(11),
            text("Create Surface from Points").size(11),
        ]
        .spacing(5)
    )
    .on_press(Message::Command("SURFACECREATE Ground".to_string()))
    .width(Fill)
    .padding([4, 8])
    .style(button::secondary);

    tree = tree.push(container(create_surf_btn).padding([2, 4]));

    if surfs.is_empty() {
        tree = tree.push(note_row("No surfaces in drawing".to_string()));
    } else {
        if surfs.len() >= 2 {
            let volume_btn = button(
                row![
                    text("⚖").size(12),
                    text("Calculate Cut / Fill Volume").size(11),
                ]
                .spacing(5)
            )
            .on_press(Message::Command("SURFACEVOLUME".to_string()))
            .width(Fill)
            .padding([5, 8])
            .style(button::primary);

            tree = tree.push(container(volume_btn).padding([2, 4]));
        }

        for surf in &surfs {
            tree = tree.push(surface_card(surf));
        }
    }

    // ── Civil 3D Road Profile Views ───────────────────────────────────────
    tree = tree.push(section_header("Civil 3D Road Profiles & Views".to_string()));

    let pview_tools_btn = button(
        row![
            text("📐").size(11),
            text("Profile Layout Tools (Floating)").size(11),
        ]
        .spacing(5)
    )
    .on_press(Message::OpenProfileLayoutTools)
    .width(Fill)
    .padding([4, 8])
    .style(button::primary);

    let pview_props_btn = button(
        row![
            text("⚙").size(11),
            text("Profile View Properties (Full)").size(11),
        ]
        .spacing(5)
    )
    .on_press(Message::OpenProfileViewProperties(None))
    .width(Fill)
    .padding([4, 8])
    .style(button::secondary);

    let pview_create_btn = button(
        row![
            text("⊞").size(11),
            text("Create Profile View Grid").size(11),
        ]
        .spacing(5)
    )
    .on_press(Message::StartProfileViewCreation)
    .width(Fill)
    .padding([4, 8])
    .style(button::secondary);

    tree = tree.push(container(column![pview_tools_btn, pview_props_btn, pview_create_btn].spacing(3)).padding([2, 4]));

    let fls = featurelines(document);
    if !fls.is_empty() {
        tree = tree.push(section_header(format!("Feature Lines ({})", fls.len())));
        for (handle, label) in fls {
            tree = tree.push(entry_row(label, format!("SELECT {:X}", handle.value())));
        }
    }

    let pts = survey_points_count(document);
    if pts > 0 {
        tree = tree.push(section_header(format!("Survey Points ({pts})")));
        let pts_tri_btn = button(
            row![
                text("📐").size(11),
                text("Triangulate Survey Points -> TIN").size(11),
            ]
            .spacing(5)
        )
        .on_press(Message::Command("SURFACECREATE Survey_TIN".to_string()))
        .width(Fill)
        .padding([4, 8])
        .style(button::secondary);
        tree = tree.push(container(pts_tri_btn).padding([2, 4]));
    }

    // ── Modeling & Solid Bodies (Secondary) ───────────────────────────────
    let bodies = bodies(document);
    if !bodies.is_empty() {
        tree = tree.push(section_header(
            crate::tf!("Bodies ({})", bodies.len()).into_owned(),
        ));
        for (handle, label) in bodies {
            tree = tree.push(entry_row(label, format!("SELECT {:X}", handle.value())));
        }
    }

    // ── Origin Planes & Sketch ────────────────────────────────────────────
    tree = tree.push(section_header(crate::t!("Origin planes").into_owned()));
    for (label, argument) in ORIGIN_PLANES {
        tree = tree.push(entry_row(
            label.to_string(),
            format!("CREATESKETCH {argument}"),
        ));
    }

    if let Some(name) = open_sketch {
        tree = tree.push(section_header(crate::t!("Sketch").into_owned()));
        tree = tree.push(entry_row(name.to_string(), "FINISHSKETCH".to_string()));
    }

    let body = scrollable(container(tree).padding(iced::Padding {
        top: 0.0,
        right: 8.0,
        bottom: 6.0,
        left: 0.0,
    }))
    .width(Fill)
    .height(Fill);

    crate::ui::dock::frame(title_bar, column![body].spacing(6), width)
}
