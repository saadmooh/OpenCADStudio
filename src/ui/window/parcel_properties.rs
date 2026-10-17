use crate::app::commands::civil_parcel::ParcelData;
use crate::app::Message;
use crate::ui::style::form::{dialog_button, dialog_button_styled};
use codec::Handle;
use iced::widget::{button, checkbox, column, container, pick_list, row, scrollable, text, text_input, Space};
use iced::{Background, Border, Color, Element, Fill, Theme};
use std::borrow::Cow;

/// Active tab in the Parcel Properties dialog.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ParcelPropertiesTab {
    #[default]
    Information,
    Composition,
    Analysis,
    Statistics,
}

/// Working state of the Parcel Properties dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct ParcelPropertiesState {
    pub handle: Handle,
    pub name: String,
    pub number: u32,
    pub site: String,
    pub description: String,
    pub active_tab: ParcelPropertiesTab,
    pub selected_style: String,
    pub selected_layer: String,
    pub zoning: String,
    pub object_locked: bool,
    pub show_tooltips: bool,

    // Composition Tab (Legal & Ownership)
    pub owner: String,
    pub deed_no: String,
    pub tax_id: String,
    pub assessment_val: String,
    pub address: String,
    pub land_use: String,

    // Analysis Tab (Mapcheck & Thematic Zoning)
    pub mapcheck_closure_error: f64,
    pub mapcheck_bearing: String,
    pub mapcheck_precision: String,
    pub mapcheck_passed: bool,
    pub thematic_color: Color,
    pub hatch_pattern: String,

    // Statistics Tab
    pub area_2d: f64,
    pub perimeter: f64,
    pub centroid_x: f64,
    pub centroid_y: f64,
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub segment_count: usize,
    pub aspect_ratio: f64,
}

impl ParcelPropertiesState {
    pub fn from_parcel(p: &ParcelData) -> Self {
        let width = (p.bbox_max[0] - p.bbox_min[0]).abs();
        let depth = (p.bbox_max[1] - p.bbox_min[1]).abs();
        let aspect_ratio = if depth > 1e-4 { width / depth } else { 1.0 };

        let bearing_str = if p.mapcheck_bearing_deg >= 0.0 && p.mapcheck_bearing_deg <= 90.0 {
            format!("N {:.1}° E", p.mapcheck_bearing_deg)
        } else if p.mapcheck_bearing_deg > 90.0 {
            format!("S {:.1}° E", 180.0 - p.mapcheck_bearing_deg)
        } else if p.mapcheck_bearing_deg < -90.0 {
            format!("S {:.1}° W", 180.0 + p.mapcheck_bearing_deg)
        } else {
            format!("N {:.1}° W", -p.mapcheck_bearing_deg)
        };

        let precision_str = if p.mapcheck_precision >= 100_000.0 {
            format!("1 : {:.0} (First-Order Survey Quality)", p.mapcheck_precision)
        } else {
            format!("1 : {:.0}", p.mapcheck_precision)
        };

        let thematic_color = if p.zoning.contains("Commercial") || p.layer.contains("COMMERCIAL") {
            Color::from_rgb(0.90, 0.35, 0.25)
        } else if p.zoning.contains("Civic") || p.layer.contains("CIVIC") {
            Color::from_rgb(0.20, 0.60, 0.90)
        } else if p.zoning.contains("Park") || p.layer.contains("PARK") {
            Color::from_rgb(0.30, 0.75, 0.35)
        } else {
            Color::from_rgb(0.92, 0.72, 0.20) // Residential Gold
        };

        Self {
            handle: p.handle,
            name: p.name.clone(),
            number: p.number,
            site: p.site_name.clone(),
            description: format!("Topological parcel '{}' in site '{}'", p.name, p.site_name),
            active_tab: ParcelPropertiesTab::Information,
            selected_style: if p.layer.contains("RESIDENTIAL") {
                "Single-Family Residential".to_string()
            } else if p.layer.contains("COMMERCIAL") {
                "Commercial / Retail".to_string()
            } else if p.layer.contains("CIVIC") {
                "Civic / Institutional".to_string()
            } else if p.layer.contains("PARK") {
                "Park / Open Space".to_string()
            } else {
                "Standard Parcel Style".to_string()
            },
            selected_layer: p.layer.clone(),
            zoning: p.zoning.clone(),
            object_locked: false,
            show_tooltips: true,

            owner: p.owner.clone(),
            deed_no: p.deed_no.clone(),
            tax_id: p.tax_id.clone(),
            assessment_val: "$185,000".to_string(),
            address: format!("Block 12, Lot {}", p.number),
            land_use: "Urban Residential".to_string(),

            mapcheck_closure_error: p.mapcheck_closure_error,
            mapcheck_bearing: bearing_str,
            mapcheck_precision: precision_str,
            mapcheck_passed: p.mapcheck_precision > 10_000.0,
            thematic_color,
            hatch_pattern: "SOLID".to_string(),

            area_2d: p.area_2d,
            perimeter: p.perimeter,
            centroid_x: p.centroid[0],
            centroid_y: p.centroid[1],
            min_x: p.bbox_min[0],
            min_y: p.bbox_min[1],
            max_x: p.bbox_max[0],
            max_y: p.bbox_max[1],
            segment_count: p.segment_count,
            aspect_ratio,
        }
    }
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
    state: &'a ParcelPropertiesState,
    sizing: crate::ui::modal::ModalSizing,
) -> Element<'a, Message> {
    // ── Tab Bar Strip ────────────────────────────────────────────────────
    let tab_button = |tab: ParcelPropertiesTab, label: &'static str| {
        let is_active = state.active_tab == tab;
        button(text(label).size(11.5))
            .on_press(Message::ParcelPropertiesTabChanged(tab))
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
        tab_button(ParcelPropertiesTab::Information, "Information"),
        tab_button(ParcelPropertiesTab::Composition, "Composition"),
        tab_button(ParcelPropertiesTab::Analysis, "Analysis"),
        tab_button(ParcelPropertiesTab::Statistics, "Statistics"),
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
        "Single-Family Residential".to_string(),
        "Multi-Family Residential".to_string(),
        "Commercial / Retail".to_string(),
        "Civic / Institutional".to_string(),
        "Park / Open Space".to_string(),
        "Industrial / Logistics".to_string(),
        "Right-of-Way Corridor".to_string(),
        "Standard Parcel Style".to_string(),
    ];

    let zoning_list = vec![
        "Single-Family Residential (R-1)".to_string(),
        "Two-Family Residential (R-2)".to_string(),
        "Multi-Family Residential (R-3)".to_string(),
        "Commercial / Retail (C-1)".to_string(),
        "Mixed-Use Town Center (MU-1)".to_string(),
        "Civic / Institutional (PUB)".to_string(),
        "Parks & Open Space (REC)".to_string(),
    ];

    let information_view = {
        // Name and Number row
        let name_col = column![
            text("Parcel Name:").size(11.5),
            text_input("Parcel Name", &state.name)
                .on_input(Message::ParcelPropertiesNameChanged)
                .size(11.5)
                .padding([5, 8])
                .width(Fill),
        ]
        .spacing(4)
        .width(Fill);

        let num_col = column![
            text("Parcel Number:").size(11.5),
            text_input("1", &state.number.to_string())
                .on_input(Message::ParcelPropertiesNumberChanged)
                .size(11.5)
                .padding([5, 8])
                .width(iced::Length::Fixed(90.0)),
        ]
        .spacing(4);

        let identity_row = row![name_col, num_col].spacing(10).width(Fill);

        // Site Name
        let site_col = column![
            text("Site:").size(11.5),
            text_input("Site 1", &state.site)
                .on_input(Message::ParcelPropertiesSiteChanged)
                .size(11.5)
                .padding([5, 8])
                .width(Fill),
        ]
        .spacing(4);

        // Description
        let desc_col = column![
            text("Description:").size(11.5),
            text_input("Description", &state.description)
                .on_input(Message::ParcelPropertiesDescriptionChanged)
                .size(11)
                .padding([5, 8])
                .width(Fill),
        ]
        .spacing(4);

        // Style picker
        let style_picker = pick_list(
            Some(state.selected_style.clone()),
            styles_list,
            |s| s.to_string(),
        )
        .on_select(Message::ParcelPropertiesStyleChanged)
        .text_size(11)
        .padding([4, 6])
        .width(Fill);

        let style_row = row![
            row![text("📐").size(12), text("Parcel style:").size(11.5)]
                .spacing(4)
                .width(110)
                .align_y(iced::Center),
            style_picker,
        ]
        .spacing(6)
        .align_y(iced::Center);

        // Zoning code picker
        let zoning_picker = pick_list(
            Some(state.zoning.clone()),
            zoning_list,
            |s| s.to_string(),
        )
        .on_select(Message::ParcelPropertiesZoningChanged)
        .text_size(11)
        .padding([4, 6])
        .width(Fill);

        let swatch = container(Space::new().width(20).height(20))
            .style({
                let col = state.thematic_color;
                move |_theme: &Theme| container::Style {
                    background: Some(Background::Color(col)),
                    border: Border {
                        width: 1.0,
                        radius: 2.0.into(),
                        color: Color::WHITE,
                    },
                    ..Default::default()
                }
            });

        let zoning_row = row![
            row![text("🏷").size(12), text("Zoning code:").size(11.5)]
                .spacing(4)
                .width(110)
                .align_y(iced::Center),
            zoning_picker,
            swatch,
        ]
        .spacing(6)
        .align_y(iced::Center);

        let styles_group = group_box(
            "Styles & Zoning Classification",
            column![style_row, zoning_row].spacing(10).padding([4, 2]),
        );

        let lock_check = row![
            checkbox(state.object_locked)
                .on_toggle(|_| Message::ParcelPropertiesToggleLocked)
                .size(14),
            text("Object locked (protect from layout tools)").size(11.5),
        ]
        .spacing(6)
        .align_y(iced::Center);

        let tooltips_check = row![
            checkbox(state.show_tooltips)
                .on_toggle(|_| Message::ParcelPropertiesToggleTooltips)
                .size(14),
            text("Show parcel hover tooltips & area callouts").size(11.5),
        ]
        .spacing(6)
        .align_y(iced::Center);

        column![
            identity_row,
            site_col,
            desc_col,
            styles_group,
            lock_check,
            tooltips_check,
        ]
        .spacing(10)
        .width(Fill)
    };

    // ── Tab 2: Composition (Legal, Owner & Taxation) ─────────────────────
    let composition_view = {
        let field_row = |label: &'static str, val: &str, on_change: fn(String) -> Message| {
            row![
                text(label).size(11.5).width(140),
                text_input(label, val)
                    .on_input(on_change)
                    .size(11)
                    .padding([4, 8])
                    .width(Fill),
            ]
            .spacing(8)
            .align_y(iced::Center)
        };

        let legal_col = column![
            field_row("Legal Owner:", &state.owner, Message::ParcelPropertiesOwnerChanged),
            field_row("Deed Reference:", &state.deed_no, Message::ParcelPropertiesDeedChanged),
            field_row("Tax Parcel ID (PIN):", &state.tax_id, Message::ParcelPropertiesTaxIdChanged),
            field_row("Street Address:", &state.address, Message::ParcelPropertiesAddressChanged),
            field_row("Assessment Value:", &state.assessment_val, Message::ParcelPropertiesAssessmentChanged),
            field_row("Land Use Category:", &state.land_use, Message::ParcelPropertiesLandUseChanged),
        ]
        .spacing(8);

        let legal_group = group_box("Legal Identity & Tax Assessment", legal_col);

        let note = text("Ownership and deed data are preserved in the drawing via CAD Extended Data (XDATA OPENCAD_PARCEL).")
            .size(10.5)
            .style(crate::ui::style::common::muted_style);

        column![legal_group, note].spacing(10).width(Fill)
    };

    // ── Tab 3: Analysis (Mapcheck & Thematic Zoning) ──────────────────────
    let analysis_view = {
        let mapcheck_row = |lbl: &'static str, val: String, ok: bool| {
            row![
                text(lbl).size(11).width(180),
                text(val).size(11).style(move |theme: &Theme| text::Style {
                    color: Some(if ok {
                        theme.palette().primary.strong.color
                    } else {
                        theme.palette().danger.base.color
                    }),
                }),
            ]
            .spacing(10)
        };

        let mapcheck_col = column![
            mapcheck_row("Error of Closure:", format!("{:.5} m", state.mapcheck_closure_error), state.mapcheck_closure_error < 0.01),
            mapcheck_row("Closure Bearing:", state.mapcheck_bearing.clone(), true),
            mapcheck_row("Traverse Precision Ratio:", state.mapcheck_precision.clone(), state.mapcheck_passed),
            mapcheck_row("Traverse Closure Criteria:", if state.mapcheck_passed { "PASSED (Class 1 Cadastral)" } else { "EXCEEDS TOLERANCE" }.to_string(), state.mapcheck_passed),
            mapcheck_row("Perimeter Traverse Length:", format!("{:.2} m", state.perimeter), true),
            mapcheck_row("Mathematical Area Check:", format!("{:.2} m²", state.area_2d), true),
        ]
        .spacing(6);

        let mapcheck_group = group_box("Civil 3D Mapcheck Boundary Closure Analysis", mapcheck_col);

        let thematic_swatch = container(Space::new().width(32).height(24))
            .style({
                let col = state.thematic_color;
                move |_theme: &Theme| container::Style {
                    background: Some(Background::Color(col)),
                    border: Border {
                        width: 1.0,
                        radius: 3.0.into(),
                        color: Color::WHITE,
                    },
                    ..Default::default()
                }
            });

        let thematic_info = column![
            row![
                text("Active Zoning Color:").size(11).width(140),
                thematic_swatch,
                text(format!("Hex: #{:02X}{:02X}{:02X}",
                    (state.thematic_color.r * 255.0) as u8,
                    (state.thematic_color.g * 255.0) as u8,
                    (state.thematic_color.b * 255.0) as u8,
                )).size(11),
            ]
            .spacing(8)
            .align_y(iced::Center),
            row![
                text("Hatch Fill Pattern:").size(11).width(140),
                pick_list(
                    Some(state.hatch_pattern.clone()),
                    vec!["SOLID".to_string(), "ANSI31".to_string(), "NET".to_string(), "CROSS".to_string(), "NONE".to_string()],
                    |s| s.to_string(),
                )
                .on_select(Message::ParcelPropertiesHatchChanged)
                .text_size(11)
                .padding([3, 6])
                .width(180),
            ]
            .spacing(8)
            .align_y(iced::Center),
        ]
        .spacing(8);

        let thematic_group = group_box("Thematic Zoning Hatching & Display", thematic_info);

        column![mapcheck_group, thematic_group].spacing(10).width(Fill)
    };

    // ── Tab 4: Statistics ────────────────────────────────────────────────
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

        let area_acres = state.area_2d * 0.000247105;
        let area_sqft = state.area_2d * 10.7639;

        let general_col = column![
            stat_row("Parcel Handle:", format!("{:X}", state.handle.value())),
            stat_row("2D Projected Area:", format!("{:.2} m² ({:.4} ha)", state.area_2d, state.area_2d / 10000.0)),
            stat_row("Imperial Area:", format!("{:.1} sq.ft ({:.3} acres)", area_sqft, area_acres)),
            stat_row("Boundary Perimeter:", format!("{:.2} m", state.perimeter)),
            stat_row("Boundary Segments:", format!("{} vertices / segments", state.segment_count)),
        ]
        .spacing(4);

        let geom_col = column![
            stat_row("Centroid Coordinates (X, Y):", format!("{:.3}, {:.3}", state.centroid_x, state.centroid_y)),
            stat_row("Bounding Box Min (X, Y):", format!("{:.3}, {:.3}", state.min_x, state.min_y)),
            stat_row("Bounding Box Max (X, Y):", format!("{:.3}, {:.3}", state.max_x, state.max_y)),
            stat_row("Width (ΔX):", format!("{:.2} m", (state.max_x - state.min_x).abs())),
            stat_row("Depth (ΔY):", format!("{:.2} m", (state.max_y - state.min_y).abs())),
            stat_row("Aspect Ratio (Width / Depth):", format!("{:.2}", state.aspect_ratio)),
            stat_row("CAD Layer:", state.selected_layer.clone()),
        ]
        .spacing(4);

        column![
            group_box("General Geometric Statistics", general_col),
            group_box("Spatial Bounding & Orientation", geom_col),
        ]
        .spacing(10)
        .width(Fill)
    };

    // ── Active Tab Switcher ───────────────────────────────────────────────
    let active_content: Element<'a, Message> = match state.active_tab {
        ParcelPropertiesTab::Information => information_view.into(),
        ParcelPropertiesTab::Composition => composition_view.into(),
        ParcelPropertiesTab::Analysis => analysis_view.into(),
        ParcelPropertiesTab::Statistics => statistics_view.into(),
    };

    // ── Dialog Bottom Action Buttons ──────────────────────────────────────
    let ok_btn = dialog_button("OK", Message::ParcelPropertiesOk, true);
    let cancel_btn = dialog_button("Cancel", Message::ParcelPropertiesClose, false);
    let apply_btn = dialog_button_styled("Apply", Message::ParcelPropertiesApply, button::secondary);
    let help_btn = dialog_button_styled("Help", Message::ParcelPropertiesHelp, button::text);

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
