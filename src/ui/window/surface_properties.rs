use crate::app::commands::civil_surface::TinSurfaceData;
use crate::app::Message;
use crate::ui::style::form::{dialog_button, dialog_button_styled};
use codec::Handle;
use iced::widget::{button, checkbox, column, container, pick_list, row, scrollable, text, text_input, Space};
use iced::{Background, Border, Color, Element, Fill, Theme};
use std::borrow::Cow;

/// Active tab in the Surface Properties dialog.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SurfacePropertiesTab {
    #[default]
    Information,
    Definition,
    Analysis,
    Statistics,
}

/// Working state of the Surface Properties dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct SurfacePropertiesState {
    pub handle: Handle,
    pub name: String,
    pub description: String,
    pub active_tab: SurfacePropertiesTab,
    pub selected_style: String,
    pub selected_material: String,
    pub object_locked: bool,
    pub show_tooltips: bool,

    // Definition Tab inputs
    pub exclude_min_elev_enabled: bool,
    pub exclude_min_elev: String,
    pub exclude_max_elev_enabled: bool,
    pub exclude_max_elev: String,
    pub max_tri_length_enabled: bool,
    pub max_tri_length: String,

    // Analysis Tab
    pub analysis_type: String,
    pub ranges_count: usize,

    // Statistics Tab
    pub point_count: usize,
    pub tri_count: usize,
    pub min_z: f64,
    pub max_z: f64,
    pub mean_z: f64,
    pub area_2d: f64,
    pub area_3d: f64,
    pub layer: String,
}

impl SurfacePropertiesState {
    pub fn from_surface(surf: &TinSurfaceData) -> Self {
        Self {
            handle: surf.handle,
            name: surf.name.clone(),
            description: format!("Civil 3D TIN Surface '{}' on layer {}", surf.name, surf.layer),
            active_tab: SurfacePropertiesTab::Information,
            selected_style: "Contours 1m and 5m (Design)".to_string(),
            selected_material: "ByLayer".to_string(),
            object_locked: false,
            show_tooltips: true,

            exclude_min_elev_enabled: false,
            exclude_min_elev: format!("{:.2}", (surf.min_z - 5.0).max(0.0)),
            exclude_max_elev_enabled: false,
            exclude_max_elev: format!("{:.2}", surf.max_z + 10.0),
            max_tri_length_enabled: false,
            max_tri_length: "50.00".to_string(),

            analysis_type: "Elevations".to_string(),
            ranges_count: 5,

            point_count: surf.triangles.len() / 2 + 2,
            tri_count: surf.triangles.len(),
            min_z: surf.min_z,
            max_z: surf.max_z,
            mean_z: surf.mean_z,
            area_2d: surf.area_2d,
            area_3d: surf.area_3d,
            layer: surf.layer.clone(),
        }
    }
}

/// Helper to render an outlined group box matching Civil 3D aesthetics.
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
        .spacing(6),
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

pub fn view_window<'a>(
    state: &'a SurfacePropertiesState,
    sizing: crate::ui::modal::ModalSizing,
) -> Element<'a, Message> {
    // ── Tab Bar Strip ────────────────────────────────────────────────────
    let tab_button = |tab: SurfacePropertiesTab, label: &'static str| {
        let is_active = state.active_tab == tab;
        button(text(label).size(11.5))
            .on_press(Message::SurfacePropertiesTabChanged(tab))
            .style(move |theme: &Theme, status| {
                let palette = theme.palette();
                let pair = match (is_active, status) {
                    (true, _) => palette.primary.strong,
                    (false, button::Status::Hovered | button::Status::Pressed) => {
                        palette.background.strong
                    }
                    _ => palette.background.weak,
                };
                button::Style {
                    background: Some(Background::Color(pair.color)),
                    text_color: pair.text,
                    border: Border {
                        color: palette.background.neutral.color,
                        width: 1.0,
                        radius: iced::border::Radius {
                            top_left: 4.0,
                            top_right: 4.0,
                            bottom_right: 0.0,
                            bottom_left: 0.0,
                        },
                    },
                    ..Default::default()
                }
            })
            .padding([5, 12])
    };

    let tab_row = row![
        tab_button(SurfacePropertiesTab::Information, "Information"),
        tab_button(SurfacePropertiesTab::Definition, "Definition"),
        tab_button(SurfacePropertiesTab::Analysis, "Analysis"),
        tab_button(SurfacePropertiesTab::Statistics, "Statistics"),
    ]
    .spacing(2)
    .width(Fill);

    let tab_divider = container(Space::new().width(Fill).height(1))
        .width(Fill)
        .height(1)
        .style(|theme: &Theme| container::Style {
            background: Some(Background::Color(theme.palette().background.neutral.color)),
            ..Default::default()
        });

    let tab_bar = column![tab_row, tab_divider].spacing(0).width(Fill);

    // ── Tab 1: Information (Civil 3D Layout) ──────────────────────────────
    let styles_list = vec![
        "Contours 1m and 5m (Design)".to_string(),
        "Contours 1m and 5m (Background)".to_string(),
        "Contours 0.5m and 2.5m".to_string(),
        "Contours and Triangles".to_string(),
        "Triangles Only".to_string(),
        "Border Only".to_string(),
        "_No Display".to_string(),
    ];

    let materials_list = vec![
        "ByLayer".to_string(),
        "Grass".to_string(),
        "Asphalt".to_string(),
        "Gravel".to_string(),
        "Dirt".to_string(),
        "Water".to_string(),
    ];

    let information_view = {
        // Name field
        let name_col = column![
            text("Name:").size(11.5),
            text_input("Surface Name", &state.name)
                .on_input(Message::SurfacePropertiesNameChanged)
                .size(11.5)
                .padding([5, 8])
                .width(Fill),
        ]
        .spacing(4);

        // Description field
        let desc_col = column![
            text("Description:").size(11.5),
            text_input("Description", &state.description)
                .on_input(Message::SurfacePropertiesDescriptionChanged)
                .size(11)
                .padding([5, 8])
                .width(Fill),
        ]
        .spacing(4);

        // Default styles group box
        let style_picker = pick_list(
            Some(state.selected_style.clone()),
            styles_list,
            |s| s.to_string(),
        )
        .on_select(Message::SurfacePropertiesStyleChanged)
        .text_size(11)
        .padding([4, 6])
        .width(Fill);

        let style_edit_btn = button(text("✏").size(12))
            .on_press(Message::SurfacePropertiesApply)
            .padding([4, 8])
            .style(button::secondary);

        let style_preview_btn = button(text("🔍").size(12))
            .on_press(Message::SurfacePropertiesApply)
            .padding([4, 8])
            .style(button::secondary);

        let style_row = row![
            row![text("📐").size(12), text("Surface style:").size(11.5)]
                .spacing(4)
                .width(110)
                .align_y(iced::Center),
            style_picker,
            style_edit_btn,
            style_preview_btn,
        ]
        .spacing(6)
        .align_y(iced::Center);

        let mat_picker = pick_list(
            Some(state.selected_material.clone()),
            materials_list,
            |s| s.to_string(),
        )
        .on_select(Message::SurfacePropertiesMaterialChanged)
        .text_size(11)
        .padding([4, 6])
        .width(Fill);

        // Material swatch preview
        let mat_swatch = container(Space::new().width(20).height(20))
            .style(|_theme: &Theme| container::Style {
                background: Some(Background::Color(Color::from_rgb(0.25, 0.65, 0.45))),
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: Color::WHITE,
                },
                ..Default::default()
            });

        let mat_row = row![
            row![text("🌐").size(12), text("Render Material:").size(11.5)]
                .spacing(4)
                .width(110)
                .align_y(iced::Center),
            mat_picker,
            mat_swatch,
        ]
        .spacing(6)
        .align_y(iced::Center);

        let styles_group = group_box(
            "Default styles",
            column![style_row, mat_row].spacing(10).padding([4, 2]),
        );

        // Bottom checkboxes
        let lock_check = row![
            checkbox(state.object_locked)
                .on_toggle(|_| Message::SurfacePropertiesToggleLocked)
                .size(14),
            text("Object locked").size(11.5),
        ]
        .spacing(6)
        .align_y(iced::Center);

        let tooltips_check = row![
            checkbox(state.show_tooltips)
                .on_toggle(|_| Message::SurfacePropertiesToggleTooltips)
                .size(14),
            text("Show tooltips").size(11.5),
        ]
        .spacing(6)
        .align_y(iced::Center);

        column![
            name_col,
            desc_col,
            styles_group,
            lock_check,
            tooltips_check,
        ]
        .spacing(12)
        .width(Fill)
    };

    // ── Tab 2: Definition (Build Parameters & History) ────────────────────
    let definition_view = {
        let min_elev_row = row![
            checkbox(state.exclude_min_elev_enabled)
                .on_toggle(Message::SurfacePropertiesExcludeMinToggled)
                .size(14),
            text("Exclude elevations less than:").size(11).width(200),
            text_input("0.00", &state.exclude_min_elev)
                .on_input(Message::SurfacePropertiesExcludeMinChanged)
                .size(11)
                .padding([3, 6])
                .width(100),
            text("m").size(11),
        ]
        .spacing(8)
        .align_y(iced::Center);

        let max_elev_row = row![
            checkbox(state.exclude_max_elev_enabled)
                .on_toggle(Message::SurfacePropertiesExcludeMaxToggled)
                .size(14),
            text("Exclude elevations greater than:").size(11).width(200),
            text_input("100.00", &state.exclude_max_elev)
                .on_input(Message::SurfacePropertiesExcludeMaxChanged)
                .size(11)
                .padding([3, 6])
                .width(100),
            text("m").size(11),
        ]
        .spacing(8)
        .align_y(iced::Center);

        let max_tri_row = row![
            checkbox(state.max_tri_length_enabled)
                .on_toggle(Message::SurfacePropertiesMaxTriToggled)
                .size(14),
            text("Maximum triangle length:").size(11).width(200),
            text_input("50.00", &state.max_tri_length)
                .on_input(Message::SurfacePropertiesMaxTriChanged)
                .size(11)
                .padding([3, 6])
                .width(100),
            text("m").size(11),
        ]
        .spacing(8)
        .align_y(iced::Center);

        let build_group = group_box(
            "Build Parameters",
            column![min_elev_row, max_elev_row, max_tri_row].spacing(8),
        );

        let operations_list = column![
            text("• Point Groups: Imported Survey Points").size(11),
            text("• Triangulation: 2D Delaunay Bowyer-Watson").size(11),
            text("• Boundaries: Automatic Convex Hull").size(11),
            text("• Edits: Swap Edge & Raise/Lower operations recorded").size(11),
        ]
        .spacing(4);

        let rebuild_btn = button(
            row![text("🔄").size(11), text("Rebuild Surface").size(11)]
                .spacing(5)
                .align_y(iced::Center),
        )
        .on_press(Message::SurfacePropertiesRebuild)
        .style(button::primary)
        .padding([4, 10]);

        let edits_group = group_box(
            "Surface Definition & Edits History",
            column![operations_list, Space::new().height(4), rebuild_btn].spacing(6),
        );

        column![build_group, edits_group].spacing(12).width(Fill)
    };

    // ── Tab 3: Analysis (Hypsometric Tinting & Slopes) ─────────────────────
    let analysis_view = {
        let dz = (state.max_z - state.min_z) / 5.0;
        let mut bands_col = column![].spacing(5).width(Fill);

        let colors = [
            (Color::from_rgb(0.1, 0.4, 0.8), "Deep Lowland"),
            (Color::from_rgb(0.2, 0.7, 0.8), "Lowland"),
            (Color::from_rgb(0.3, 0.8, 0.3), "Mid Elevation"),
            (Color::from_rgb(0.9, 0.8, 0.2), "Upland"),
            (Color::from_rgb(0.9, 0.2, 0.2), "High Peak"),
        ];

        for (i, &(col, label)) in colors.iter().enumerate() {
            let z_start = state.min_z + dz * (i as f64);
            let z_end = z_start + dz;
            let swatch = container(Space::new().width(18).height(12))
                .style(move |_theme: &Theme| container::Style {
                    background: Some(Background::Color(col)),
                    border: Border {
                        width: 1.0,
                        radius: 2.0.into(),
                        color: Color::WHITE,
                    },
                    ..Default::default()
                });

            let band_row = row![
                text(format!("#{}:", i + 1)).size(11).width(25),
                swatch,
                text(format!("{:.2} m  →  {:.2} m", z_start, z_end))
                    .size(11)
                    .width(160),
                text(label).size(10.5).style(crate::ui::style::common::muted_style),
            ]
            .spacing(8)
            .align_y(iced::Center);

            bands_col = bands_col.push(band_row);
        }

        let analysis_group = group_box(
            "Elevation Ranges (Hypsometric Color Banding)",
            bands_col,
        );

        let legend_note = text("Elevation analysis visualizes terrain relief and gradient variation across the project area.")
            .size(10.5)
            .style(crate::ui::style::common::muted_style);

        column![analysis_group, legend_note].spacing(8).width(Fill)
    };

    // ── Tab 4: Statistics (Civil 3D Layout) ────────────────────────────────
    let statistics_view = {
        let stat_row = |lbl: &'static str, val: String| {
            row![
                text(lbl).size(11).width(200),
                text(val).size(11).style(|theme: &Theme| text::Style {
                    color: Some(theme.palette().primary.strong.color),
                }),
            ]
            .spacing(10)
        };

        let general_col = column![
            stat_row("Revision Number:", "1".to_string()),
            stat_row("Points Count:", state.point_count.to_string()),
            stat_row("Minimum Elevation:", format!("{:.3} m", state.min_z)),
            stat_row("Maximum Elevation:", format!("{:.3} m", state.max_z)),
            stat_row("Mean Elevation:", format!("{:.3} m", state.mean_z)),
        ]
        .spacing(4);

        let extended_col = column![
            stat_row("2D Projected Area:", format!("{:.2} m² ({:.3} ha)", state.area_2d, state.area_2d / 10000.0)),
            stat_row("3D Surface Area:", format!("{:.2} m²", state.area_3d)),
            stat_row("Area Ratio (3D / 2D):", format!("{:.4}", if state.area_2d > 0.0 { state.area_3d / state.area_2d } else { 1.0 })),
        ]
        .spacing(4);

        let tin_col = column![
            stat_row("Number of Triangles:", state.tri_count.to_string()),
            stat_row("Layer Name:", state.layer.clone()),
            stat_row("Surface Handle:", format!("{:X}", state.handle.value())),
        ]
        .spacing(4);

        column![
            group_box("General", general_col),
            group_box("Extended Area", extended_col),
            group_box("TIN Triangulation", tin_col),
        ]
        .spacing(10)
        .width(Fill)
    };

    // ── Active Tab Switcher ───────────────────────────────────────────────
    let active_content: Element<'a, Message> = match state.active_tab {
        SurfacePropertiesTab::Information => information_view.into(),
        SurfacePropertiesTab::Definition => definition_view.into(),
        SurfacePropertiesTab::Analysis => analysis_view.into(),
        SurfacePropertiesTab::Statistics => statistics_view.into(),
    };

    // ── Dialog Bottom Action Buttons ──────────────────────────────────────
    let ok_btn = dialog_button("OK", Message::SurfacePropertiesOk, true);
    let cancel_btn = dialog_button("Cancel", Message::SurfacePropertiesClose, false);
    let apply_btn = dialog_button_styled("Apply", Message::SurfacePropertiesApply, button::secondary);
    let help_btn = dialog_button_styled("Help", Message::SurfacePropertiesHelp, button::text);

    let actions_row = row![
        help_btn,
        Space::new().width(Fill),
        ok_btn,
        cancel_btn,
        apply_btn,
    ]
    .spacing(8)
    .align_y(iced::Center);

    // ── Main Body Layout ──────────────────────────────────────────────────
    let body = column![
        tab_bar,
        Space::new().height(8),
        scrollable(active_content).height(Fill),
        Space::new().height(10),
        actions_row,
    ]
    .width(sizing.width)
    .height(sizing.height);

    container(body)
        .style(container::rounded_box)
        .padding([12, 16])
        .width(sizing.width)
        .height(sizing.height)
        .into()
}
