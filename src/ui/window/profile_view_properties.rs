use crate::app::Message;
use crate::ui::style::form::{dialog_button, dialog_button_styled};
use iced::widget::{button, checkbox, column, container, mouse_area, pick_list, row, scrollable, text, text_input, Space};
use iced::{Background, Border, Color, Element, Fill, Theme};
use std::borrow::Cow;

/// Surface profile item displayed inside the Profile View dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct ProfileViewSurfaceItem {
    pub name: String,
    pub is_visible: bool,
    pub is_ref: bool,
    pub min_z: f64,
    pub max_z: f64,
    pub layer: String,
}

/// Active tab in the Profile View Properties dialog.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ProfileViewPropertiesTab {
    #[default]
    Surfaces,
    Grid,
    Bands,
    ProfileFG,
}

/// Working state of the Profile View Properties dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct ProfileViewPropertiesState {
    pub view_id: String,
    pub align_name: String,
    pub align_length: f64,
    pub active_tab: ProfileViewPropertiesTab,
    pub surfaces: Vec<ProfileViewSurfaceItem>,
    pub selected_ref_surface: String,
    pub vert_exag: String,
    pub datum_elev: String,
    pub max_elev: String,
    pub band_interval: String,
    pub show_station_band: bool,
    pub show_eg_band: bool,
    pub show_fg_band: bool,
    pub show_diff_band: bool,
}

impl Default for ProfileViewPropertiesState {
    fn default() -> Self {
        Self {
            view_id: "PV_ALIGNMENT".to_string(),
            align_name: "Alignment-1".to_string(),
            align_length: 300.0,
            active_tab: ProfileViewPropertiesTab::Surfaces,
            surfaces: Vec::new(),
            selected_ref_surface: "EG".to_string(),
            vert_exag: "10.0".to_string(),
            datum_elev: "90.0".to_string(),
            max_elev: "120.0".to_string(),
            band_interval: "25.0".to_string(),
            show_station_band: true,
            show_eg_band: true,
            show_fg_band: true,
            show_diff_band: true,
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
    state: &'a ProfileViewPropertiesState,
    sizing: crate::ui::modal::ModalSizing,
) -> Element<'a, Message> {
    // ── Tab Bar Strip ────────────────────────────────────────────────────
    let tab_button = |tab: ProfileViewPropertiesTab, label: &'static str| {
        let is_active = state.active_tab == tab;
        button(text(label).size(11.5))
            .on_press(Message::ProfileViewPropertiesTabChanged(tab))
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
        tab_button(ProfileViewPropertiesTab::Surfaces, "Surfaces & Profiles"),
        tab_button(ProfileViewPropertiesTab::Grid, "Grid & Scales"),
        tab_button(ProfileViewPropertiesTab::Bands, "Data Bands"),
        tab_button(ProfileViewPropertiesTab::ProfileFG, "Finished Grade (FG)"),
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

    // ── Tab 1: Surfaces & Profiles (Civil 3D AeccProfileViewProperties) ───
    let surfaces_view = {
        // Alignment & View Details
        let info_col = column![
            row![
                text("Profile View Name:").size(11.5).width(130),
                text(&state.view_id).size(11.5),
            ]
            .spacing(8),
            row![
                text("Parent Alignment:").size(11.5).width(130),
                text(&state.align_name).size(11.5),
            ]
            .spacing(8),
            row![
                text("Alignment Length:").size(11.5).width(130),
                text(format!("{:.2} m", state.align_length)).size(11.5),
            ]
            .spacing(8),
        ]
        .spacing(6);

        // Surfaces checkboxes list
        let mut surf_rows = column![].spacing(6);
        if state.surfaces.is_empty() {
            surf_rows = surf_rows.push(
                text("No TIN surfaces currently available in drawing. Create or import surfaces first.")
                    .size(11)
                    .style(|theme: &Theme| text::Style {
                        color: Some(theme.palette().danger.base.color),
                    }),
            );
        } else {
            for (idx, surf) in state.surfaces.iter().enumerate() {
                let is_checked = surf.is_visible;
                let chk = row![
                    checkbox(is_checked)
                        .size(14)
                        .on_toggle(move |val| Message::ProfileViewPropertiesSurfaceToggled(idx, val)),
                    text(format!(
                        "{} (Layer: {}, Range: {:.1}m .. {:.1}m)",
                        surf.name, surf.layer, surf.min_z, surf.max_z
                    ))
                    .size(11.5),
                ]
                .spacing(6)
                .align_y(iced::Center);

                surf_rows = surf_rows.push(chk);
            }
        }

        // Reference surface dropdown
        let surf_names: Vec<String> = if state.surfaces.is_empty() {
            vec!["EG".to_string()]
        } else {
            state.surfaces.iter().map(|s| s.name.clone()).collect()
        };

        let ref_picker = pick_list(
            Some(state.selected_ref_surface.clone()),
            surf_names,
            |s| s.to_string(),
        )
        .on_select(Message::ProfileViewPropertiesRefChanged)
        .width(Fill);

        let ref_col = column![
            text("Reference Surface (Ground / Datum for Cut & Fill calculation):").size(11.5),
            ref_picker,
        ]
        .spacing(4);

        column![
            group_box("General Profile View Information", info_col),
            group_box("Sampled Surface Profiles (Check to display in View)", surf_rows),
            group_box("Reference Surface (EG Benchmark)", ref_col),
        ]
        .spacing(10)
        .width(Fill)
    };

    // ── Tab 2: Grid & Scales (Vertical Exaggeration & Elevations) ─────────
    let grid_view = {
        let exag_col = column![
            text("Vertical Exaggeration Factor (e.g. 10.0 for 10x):").size(11.5),
            text_input("10.0", &state.vert_exag)
                .on_input(Message::ProfileViewPropertiesScaleChanged)
                .size(11.5)
                .padding([5, 8])
                .width(Fill),
            text("Civil 3D default exaggeration is 10x (1:1000 H / 1:100 V).").size(10.5),
        ]
        .spacing(4);

        let elev_col = column![
            row![
                column![
                    text("Datum Elevation (منسوب المقارنة):").size(11.5),
                    text_input("90.0", &state.datum_elev)
                        .on_input(Message::ProfileViewPropertiesDatumChanged)
                        .size(11.5)
                        .padding([5, 8])
                        .width(Fill),
                ]
                .spacing(4)
                .width(Fill),
                column![
                    text("Maximum Grid Elevation:").size(11.5),
                    text_input("120.0", &state.max_elev)
                        .on_input(Message::ProfileViewPropertiesMaxElevChanged)
                        .size(11.5)
                        .padding([5, 8])
                        .width(Fill),
                ]
                .spacing(4)
                .width(Fill),
            ]
            .spacing(12),
        ]
        .spacing(4);

        let grid_style_info = column![
            row![
                text("Horizontal Station Grid:").size(11).width(180),
                text("Major: 50.0m | Minor: 10.0m").size(11),
            ],
            row![
                text("Vertical Elevation Grid:").size(11).width(180),
                text("Major: 5.0m  | Minor: 1.0m").size(11),
            ],
            row![
                text("Axis Display:").size(11).width(180),
                text("Left & Bottom ticks, Title annotation").size(11),
            ],
        ]
        .spacing(4);

        column![
            group_box("Scale & Aspect Ratio", exag_col),
            group_box("Elevation Limits (Datum & Top)", elev_col),
            group_box("Grid Spacing & Ticks", grid_style_info),
        ]
        .spacing(10)
        .width(Fill)
    };

    // ── Tab 3: Data Bands (Station, EG, FG, Cut/Fill) ─────────────────────
    let bands_view = {
        let interval_col = column![
            text("Band Station Step / Interval (e.g. 25.0 m):").size(11.5),
            text_input("25.0", &state.band_interval)
                .on_input(Message::ProfileViewPropertiesStepChanged)
                .size(11.5)
                .padding([5, 8])
                .width(Fill),
            text("Data bands sample ground and design elevations at each station interval.").size(10.5),
        ]
        .spacing(4);

        let bands_list_col = column![
            row![
                checkbox(state.show_station_band)
                    .size(14)
                    .on_toggle(Message::ProfileViewPropertiesToggleStationBand),
                text("Station Band (محطات المسار: 0+000, 0+025, ...)").size(11.5),
            ]
            .spacing(6)
            .align_y(iced::Center),
            row![
                checkbox(state.show_eg_band)
                    .size(14)
                    .on_toggle(Message::ProfileViewPropertiesToggleEgBand),
                text("Existing Ground Elevation Band (منسوب الأرض الطبيعية EG)").size(11.5),
            ]
            .spacing(6)
            .align_y(iced::Center),
            row![
                checkbox(state.show_fg_band)
                    .size(14)
                    .on_toggle(Message::ProfileViewPropertiesToggleFgBand),
                text("Finished Design Elevation Band (منسوب الطريق التصميمي FG)").size(11.5),
            ]
            .spacing(6)
            .align_y(iced::Center),
            row![
                checkbox(state.show_diff_band)
                    .size(14)
                    .on_toggle(Message::ProfileViewPropertiesToggleDiffBand),
                text("Cut / Fill Elevation Difference Band (فرق المنسوب حفر / ردم)").size(11.5),
            ]
            .spacing(6)
            .align_y(iced::Center),
        ]
        .spacing(8);

        column![
            group_box("Band Sampling Interval", interval_col),
            group_box("Active Data Band Rows (Civil 3D Band Set)", bands_list_col),
        ]
        .spacing(10)
        .width(Fill)
    };

    // ── Tab 4: Finished Grade (FG) Profile Design ────────────────────────
    let profile_fg_view = {
        let fg_intro = column![
            text("Draw interactive finished road grade profile (PVI - Points of Vertical Intersection).")
                .size(11.5),
            text("Clicking 'Start Interactive FG Design' allows you to click on the profile view grid to place vertical tangents and crest/sag curves.")
                .size(10.5),
            Space::new().height(6),
            button(text("✏ Start Interactive FG Road Design (رسم منسوب الطريق)").size(11.5))
                .on_press(Message::ProfileViewPropertiesDrawFg)
                .padding([7, 14])
                .style(button::primary),
        ]
        .spacing(6);

        column![
            group_box("Vertical Alignment Design (Civil 3D AeccProfile)", fg_intro),
        ]
        .spacing(10)
        .width(Fill)
    };

    // ── Active Tab Switcher ───────────────────────────────────────────────
    let active_content: Element<'a, Message> = match state.active_tab {
        ProfileViewPropertiesTab::Surfaces => surfaces_view.into(),
        ProfileViewPropertiesTab::Grid => grid_view.into(),
        ProfileViewPropertiesTab::Bands => bands_view.into(),
        ProfileViewPropertiesTab::ProfileFG => profile_fg_view.into(),
    };

    // ── Dialog Bottom Action Buttons ──────────────────────────────────────
    let ok_btn = dialog_button("OK", Message::ProfileViewPropertiesOk, true);
    let cancel_btn = dialog_button("Cancel", Message::ProfileViewPropertiesClose, false);
    let apply_btn = dialog_button_styled("Apply", Message::ProfileViewPropertiesApply, button::secondary);
    let draw_fg_btn = dialog_button_styled("Draw FG", Message::ProfileViewPropertiesDrawFg, button::primary);
    let float_btn = dialog_button_styled("🔲 Float to Canvas", Message::OpenProfileLayoutTools, button::secondary);
    let help_btn = dialog_button_styled("Help", Message::ProfileViewPropertiesHelp, button::text);

    let actions_row = row![
        help_btn,
        float_btn,
        Space::new().width(Fill),
        draw_fg_btn,
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

/// Modeless floating toolbar & palette for Civil 3D Profile View & Road Design.
/// Renders directly on the canvas without any blocking backdrop, allowing
/// the user to interactively draw, pan, zoom, and pick points in model space.
pub fn view_floating_palette<'a>(
    state: &'a ProfileViewPropertiesState,
    collapsed: bool,
) -> Element<'a, Message> {
    // 1. Draggable title bar with 4 position shortcuts, collapse toggle, and close
    let drag_handle = mouse_area(
        row![
            text("✥").size(13),
            text("Profile Layout Tools (أدوات البروفيل)").size(11.5).font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            }),
            Space::new().width(Fill),
        ]
        .spacing(4)
        .align_y(iced::Center)
    )
    .on_press(Message::ProfilePaletteDragStart)
    .interaction(iced::mouse::Interaction::Grab);

    let title_row = row![
        drag_handle,
        // Position shift buttons (4 corners)
        button(text("↖").size(10))
            .on_press(Message::ProfilePaletteMoveCorner(0))
            .padding([1, 4])
            .style(button::subtle),
        button(text("↗").size(10))
            .on_press(Message::ProfilePaletteMoveCorner(1))
            .padding([1, 4])
            .style(button::subtle),
        button(text("↘").size(10))
            .on_press(Message::ProfilePaletteMoveCorner(2))
            .padding([1, 4])
            .style(button::subtle),
        button(text("↙").size(10))
            .on_press(Message::ProfilePaletteMoveCorner(3))
            .padding([1, 4])
            .style(button::subtle),
        // Collapse toggle
        button(
            text(if collapsed { "▼ Options" } else { "▲ Compact" })
                .size(10.5)
        )
        .on_press(Message::ProfilePaletteToggleCollapse)
        .padding([2, 6])
        .style(button::subtle),
        // Close button
        button(text("✕").size(11))
            .on_press(Message::ProfilePaletteClose)
            .padding([2, 6])
            .style(button::danger),
    ]
    .spacing(4)
    .align_y(iced::Center);

    // 2. Action buttons row (Modeless CAD commands)
    let tool_btn = |icon: &'static str, label: &'static str, msg: Message, is_primary: bool| {
        button(
            row![
                text(icon).size(12),
                text(label).size(11),
            ]
            .spacing(3)
            .align_y(iced::Center)
        )
        .on_press(msg)
        .style(if is_primary { button::primary } else { button::secondary })
        .padding([3, 6])
    };

    let tools_row = row![
        tool_btn("🔄", "تحديث الشبكة (Apply)", Message::ProfileViewPropertiesApply, true),
        tool_btn("✏", "خط البروفيل (Design Profile)", Message::ProfileViewPropertiesDrawFg, false),
        tool_btn("⊞", "بروفيل جديد (New View)", Message::StartProfileViewCreation, false),
        tool_btn(if collapsed { "▼" } else { "▲" }, if collapsed { "خيارات" } else { "طي" }, Message::ProfilePaletteToggleCollapse, false),
    ]
    .spacing(4)
    .align_y(iced::Center);

    // 3. Status strip
    let active_surfs_count = state.surfaces.iter().filter(|s| s.is_visible).count();
    let status_strip = row![
        text("ℹ").size(11),
        text(format!(
            "Align: '{}' ({:.1}m) | Ref EG: '{}' | Scale: {}x | Active Surfs: {}/{}",
            state.align_name, state.align_length, state.selected_ref_surface, state.vert_exag,
            active_surfs_count, state.surfaces.len()
        ))
        .size(10.5)
        .style(crate::ui::style::common::muted_style),
    ]
    .spacing(4)
    .align_y(iced::Center);

    let mut body = column![title_row, tools_row].spacing(6);

    if !collapsed {
        // Tab Bar inside floating window
        let tab_btn = |tab: ProfileViewPropertiesTab, label: &'static str| {
            let is_active = state.active_tab == tab;
            button(text(label).size(10.5))
                .on_press(Message::ProfileViewPropertiesTabChanged(tab))
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
                                top_left: 3.0,
                                top_right: 3.0,
                                bottom_right: 0.0,
                                bottom_left: 0.0,
                            },
                        },
                        ..Default::default()
                    }
                })
                .padding([3, 8])
        };

        let tabs_strip = row![
            tab_btn(ProfileViewPropertiesTab::Surfaces, "🏔 Surfaces"),
            tab_btn(ProfileViewPropertiesTab::Grid, "📐 Grid & Scale"),
            tab_btn(ProfileViewPropertiesTab::Bands, "📊 Data Bands"),
            tab_btn(ProfileViewPropertiesTab::ProfileFG, "📈 Design Profile"),
        ]
        .spacing(2)
        .width(Fill);

        // Tab Content
        let tab_body: Element<'a, Message> = match state.active_tab {
            ProfileViewPropertiesTab::Surfaces => {
                let surf_names: Vec<String> = if state.surfaces.is_empty() {
                    vec!["EG".to_string()]
                } else {
                    state.surfaces.iter().map(|s| s.name.clone()).collect()
                };

                let ref_row = row![
                    text("Ref Ground (EG):").size(10.5),
                    pick_list(
                        Some(state.selected_ref_surface.clone()),
                        surf_names,
                        |s| s.to_string(),
                    )
                    .on_select(Message::ProfileViewPropertiesRefChanged)
                    .text_size(10.0)
                    .padding([2, 4])
                    .width(iced::Length::Fixed(140.0)),
                    Space::new().width(Fill),
                    text(format!("Total: {} surfaces", state.surfaces.len())).size(10.0),
                ]
                .spacing(6)
                .align_y(iced::Center);

                let mut surf_list = column![].spacing(4);
                if state.surfaces.is_empty() {
                    surf_list = surf_list.push(
                        text("No TIN surfaces found in drawing. Import LandXML or create TIN surfaces.")
                            .size(10.5)
                            .style(|theme: &Theme| text::Style {
                                color: Some(theme.palette().danger.base.color),
                            })
                    );
                } else {
                    for (idx, surf) in state.surfaces.iter().enumerate() {
                        let is_checked = surf.is_visible;
                        surf_list = surf_list.push(
                            row![
                                checkbox(is_checked)
                                    .size(13)
                                    .on_toggle(move |val| Message::ProfileViewPropertiesSurfaceToggled(idx, val)),
                                text(format!(
                                    "{} (Layer: {}, {:.1}m .. {:.1}m)",
                                    surf.name, surf.layer, surf.min_z, surf.max_z
                                ))
                                .size(10.5),
                            ]
                            .spacing(5)
                            .align_y(iced::Center)
                        );
                    }
                }

                column![
                    ref_row,
                    container(scrollable(surf_list).height(iced::Length::Fixed(110.0)))
                        .padding([4, 6])
                        .style(|theme: &Theme| container::Style {
                            background: Some(Background::Color(theme.palette().background.weakest.color)),
                            border: Border {
                                width: 1.0,
                                radius: 3.0.into(),
                                color: theme.palette().background.neutral.color,
                            },
                            ..Default::default()
                        }),
                ]
                .spacing(6)
                .into()
            }
            ProfileViewPropertiesTab::Grid => {
                let scale_row = row![
                    text("Vertical Scale:").size(10.5),
                    text_input("10.0", &state.vert_exag)
                        .on_input(Message::ProfileViewPropertiesScaleChanged)
                        .size(10.5)
                        .padding([2, 4])
                        .width(iced::Length::Fixed(50.0)),
                    text("x").size(10.5),
                    button(text("5x").size(10.0))
                        .on_press(Message::ProfileViewPropertiesScaleChanged("5.0".to_string()))
                        .padding([1, 4])
                        .style(button::subtle),
                    button(text("10x").size(10.0))
                        .on_press(Message::ProfileViewPropertiesScaleChanged("10.0".to_string()))
                        .padding([1, 4])
                        .style(button::subtle),
                    button(text("20x").size(10.0))
                        .on_press(Message::ProfileViewPropertiesScaleChanged("20.0".to_string()))
                        .padding([1, 4])
                        .style(button::subtle),
                ]
                .spacing(4)
                .align_y(iced::Center);

                let elev_row = row![
                    text("Datum:").size(10.5),
                    text_input("90.0", &state.datum_elev)
                        .on_input(Message::ProfileViewPropertiesDatumChanged)
                        .size(10.5)
                        .padding([2, 4])
                        .width(iced::Length::Fixed(60.0)),
                    text("m").size(10.5),
                    Space::new().width(10),
                    text("Max Elev:").size(10.5),
                    text_input("120.0", &state.max_elev)
                        .on_input(Message::ProfileViewPropertiesMaxElevChanged)
                        .size(10.5)
                        .padding([2, 4])
                        .width(iced::Length::Fixed(60.0)),
                    text("m").size(10.5),
                ]
                .spacing(4)
                .align_y(iced::Center);

                let grid_info = text("Station Ticks: Major 50m / Minor 10m  |  Elevation Ticks: Major 5m / Minor 1m")
                    .size(10.0)
                    .style(crate::ui::style::common::muted_style);

                column![scale_row, elev_row, grid_info]
                    .spacing(8)
                    .into()
            }
            ProfileViewPropertiesTab::Bands => {
                let interval_row = row![
                    text("Band Interval:").size(10.5),
                    text_input("25.0", &state.band_interval)
                        .on_input(Message::ProfileViewPropertiesStepChanged)
                        .size(10.5)
                        .padding([2, 4])
                        .width(iced::Length::Fixed(50.0)),
                    text("m").size(10.5),
                    button(text("10m").size(10.0))
                        .on_press(Message::ProfileViewPropertiesStepChanged("10.0".to_string()))
                        .padding([1, 4])
                        .style(button::subtle),
                    button(text("25m").size(10.0))
                        .on_press(Message::ProfileViewPropertiesStepChanged("25.0".to_string()))
                        .padding([1, 4])
                        .style(button::subtle),
                    button(text("50m").size(10.0))
                        .on_press(Message::ProfileViewPropertiesStepChanged("50.0".to_string()))
                        .padding([1, 4])
                        .style(button::subtle),
                ]
                .spacing(4)
                .align_y(iced::Center);

                let bands_col = column![
                    row![
                        checkbox(state.show_station_band)
                            .size(13)
                            .on_toggle(Message::ProfileViewPropertiesToggleStationBand),
                        text("Station Band (محطات المسار)").size(10.5),
                    ]
                    .spacing(4)
                    .align_y(iced::Center),
                    row![
                        checkbox(state.show_eg_band)
                            .size(13)
                            .on_toggle(Message::ProfileViewPropertiesToggleEgBand),
                        text("Existing Ground (EG) Band (منسوب الأرض الطبيعية)").size(10.5),
                    ]
                    .spacing(4)
                    .align_y(iced::Center),
                    row![
                        checkbox(state.show_fg_band)
                            .size(13)
                            .on_toggle(Message::ProfileViewPropertiesToggleFgBand),
                        text("Finished Grade (FG) Band (منسوب الطريق التصميمي)").size(10.5),
                    ]
                    .spacing(4)
                    .align_y(iced::Center),
                    row![
                        checkbox(state.show_diff_band)
                            .size(13)
                            .on_toggle(Message::ProfileViewPropertiesToggleDiffBand),
                        text("Cut / Fill Difference Band (فرق منسوب الحفر والردم)").size(10.5),
                    ]
                    .spacing(4)
                    .align_y(iced::Center),
                ]
                .spacing(4);

                column![interval_row, bands_col]
                    .spacing(6)
                    .into()
            }
            ProfileViewPropertiesTab::ProfileFG => {
                let fg_desc = column![
                    text("Draw vertical design profile (Civil 3D AeccProfile parity):")
                        .size(10.5),
                    text("Place PVIs (Points of Vertical Intersection) interactively on the grid for roads, pipes, canals, or site grading.")
                        .size(10.0)
                        .style(crate::ui::style::common::muted_style),
                    Space::new().height(4),
                    button(
                        row![
                            text("✏").size(12),
                            text("Start Interactive Design Profile (رسم خط البروفيل التصميمي)").size(11),
                        ]
                        .spacing(4)
                        .align_y(iced::Center)
                    )
                    .on_press(Message::ProfileViewPropertiesDrawFg)
                    .padding([5, 10])
                    .style(button::primary),
                ]
                .spacing(4);

                fg_desc.into()
            }
        };

        let content_box = container(
            column![tabs_strip, tab_body].spacing(6)
        )
        .padding([6, 8])
        .style(|theme: &Theme| container::Style {
            background: Some(Background::Color(theme.palette().background.weakest.color)),
            border: Border {
                width: 1.0,
                radius: 4.0.into(),
                color: theme.palette().background.neutral.color,
            },
            ..Default::default()
        });

        body = body.push(content_box);
    }

    body = body.push(status_strip);

    container(body)
        .padding([6, 8])
        .width(if collapsed { iced::Length::Shrink } else { iced::Length::Fixed(480.0) })
        .style(|theme: &Theme| {
            let palette = theme.palette();
            container::Style {
                background: Some(Background::Color(palette.background.weak.color)),
                border: Border {
                    color: palette.primary.strong.color,
                    width: 1.5,
                    radius: 5.0.into(),
                },
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.45),
                    offset: iced::Vector::new(0.0, 3.0),
                    blur_radius: 10.0,
                },
                ..Default::default()
            }
        })
        .into()
}

/// Modeless floating Creation Dialog for Profile View (Civil 3D Create Profile View parity).
/// Appears BEFORE placing the grid on the canvas, allowing the user to configure
/// the Alignment, Surfaces to display, Reference Ground (EG), Scale, and Data Bands.
/// Once approved with "✔ Pick Insertion Point", the dialog closes and the user is
/// prompted on the drawing canvas to click the insertion point for the profile view grid.
pub fn view_creation_dialog<'a>(
    state: &'a ProfileViewPropertiesState,
    alignments: &'a [String],
) -> Element<'a, Message> {
    // 1. Draggable title bar
    let drag_title = mouse_area(
        row![
            text("✥").size(14),
            text("إنشاء شبكة مقطع طولي (Create Profile View)").size(12).font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            }),
            Space::new().width(Fill),
        ]
        .spacing(6)
        .align_y(iced::Center)
    )
    .on_press(Message::ProfileCreationDialogDragStart)
    .interaction(iced::mouse::Interaction::Grab);

    let title_bar = row![
        drag_title,
        button(text("✕").size(11))
            .on_press(Message::ProfileViewCreationCancel)
            .padding([2, 6])
            .style(button::danger),
    ]
    .spacing(4)
    .align_y(iced::Center);

    // 2. Alignment picker
    let align_options: Vec<String> = if alignments.is_empty() {
        vec![state.align_name.clone()]
    } else {
        alignments.to_vec()
    };

    let align_section = group_box(
        "1. اختيار المسار الأفقي (Horizontal Alignment)",
        row![
            text("المسار:").size(11),
            pick_list(
                Some(state.align_name.clone()),
                align_options,
                |s| s.to_string(),
            )
            .on_select(Message::ProfileViewCreationAlignChanged)
            .text_size(10.5)
            .padding([3, 6])
            .width(iced::Length::Fixed(180.0)),
            Space::new().width(10),
            text(format!("طول المسار: {:.1}m", state.align_length))
                .size(10.5)
                .style(crate::ui::style::common::muted_style),
        ]
        .spacing(6)
        .align_y(iced::Center),
    );

    // 3. Surfaces checklist & Ref surface
    let surf_names: Vec<String> = if state.surfaces.is_empty() {
        vec!["EG".to_string()]
    } else {
        state.surfaces.iter().map(|s| s.name.clone()).collect()
    };

    let ref_row = row![
        text("السطح المرجعي (Reference Ground EG):").size(10.5),
        pick_list(
            Some(state.selected_ref_surface.clone()),
            surf_names,
            |s| s.to_string(),
        )
        .on_select(Message::ProfileViewPropertiesRefChanged)
        .text_size(10.5)
        .padding([2, 5])
        .width(iced::Length::Fixed(150.0)),
    ]
    .spacing(6)
    .align_y(iced::Center);

    let mut surf_list = column![].spacing(3);
    if state.surfaces.is_empty() {
        surf_list = surf_list.push(
            text("لا توجد أسطح TIN في الرسم حالياً. سيتم رسم شبكة فارغة ومقارنتها.")
                .size(10.0)
                .style(crate::ui::style::common::muted_style),
        );
    } else {
        for (idx, surf) in state.surfaces.iter().enumerate() {
            let is_checked = surf.is_visible;
            surf_list = surf_list.push(
                row![
                    checkbox(is_checked)
                        .size(13)
                        .on_toggle(move |val| Message::ProfileViewPropertiesSurfaceToggled(idx, val)),
                    text(format!(
                        "{} (طبقة: {}, منسوب: {:.1}m .. {:.1}m)",
                        surf.name, surf.layer, surf.min_z, surf.max_z
                    ))
                    .size(10.5),
                ]
                .spacing(5)
                .align_y(iced::Center),
            );
        }
    }

    let surfs_section = group_box(
        "2. الأسطح والمساحات المطلوب إظهار بروفيلها في الشبكة",
        column![
            ref_row,
            container(scrollable(surf_list).height(iced::Length::Fixed(85.0)))
                .padding([4, 6])
                .style(|theme: &Theme| container::Style {
                    background: Some(Background::Color(theme.palette().background.weakest.color)),
                    border: Border {
                        width: 1.0,
                        radius: 3.0.into(),
                        color: theme.palette().background.neutral.color,
                    },
                    ..Default::default()
                }),
        ]
        .spacing(6),
    );

    // 4. Scales and Data Bands
    let scale_row = row![
        text("المقياس الرأسي (Vertical Exaggeration):").size(10.5),
        text_input("10.0", &state.vert_exag)
            .on_input(Message::ProfileViewPropertiesScaleChanged)
            .size(10.5)
            .padding([2, 5])
            .width(iced::Length::Fixed(50.0)),
        button(text("5x").size(10.0))
            .on_press(Message::ProfileViewPropertiesScaleChanged("5.0".to_string()))
            .padding([1, 4])
            .style(button::subtle),
        button(text("10x").size(10.0))
            .on_press(Message::ProfileViewPropertiesScaleChanged("10.0".to_string()))
            .padding([1, 4])
            .style(button::subtle),
        button(text("20x").size(10.0))
            .on_press(Message::ProfileViewPropertiesScaleChanged("20.0".to_string()))
            .padding([1, 4])
            .style(button::subtle),
    ]
    .spacing(4)
    .align_y(iced::Center);

    let bands_row = row![
        checkbox(state.show_station_band)
            .size(13)
            .on_toggle(Message::ProfileViewPropertiesToggleStationBand),
        text("المحطات (Stations)").size(10.0),
        Space::new().width(6),
        checkbox(state.show_eg_band)
            .size(13)
            .on_toggle(Message::ProfileViewPropertiesToggleEgBand),
        text("منسوب الأرض (EG)").size(10.0),
        Space::new().width(6),
        checkbox(state.show_fg_band)
            .size(13)
            .on_toggle(Message::ProfileViewPropertiesToggleFgBand),
        text("منسوب التصميم (Design)").size(10.0),
        Space::new().width(6),
        checkbox(state.show_diff_band)
            .size(13)
            .on_toggle(Message::ProfileViewPropertiesToggleDiffBand),
        text("فرق المناسيب (Cut/Fill)").size(10.0),
    ]
    .spacing(3)
    .align_y(iced::Center);

    let grid_bands_section = group_box(
        "3. مقياس الرسم وأشرطة البيانات (Scales & Data Bands)",
        column![scale_row, bands_row].spacing(6),
    );

    // 5. Actions: Confirm & Pick Insertion Point or Cancel
    let confirm_btn = button(
        row![
            text("✔").size(13),
            text("موافقة وتحديد نقطة الإدراج على مساحة الرسم (OK - Pick Insertion Point)").size(11.5),
        ]
        .spacing(6)
        .align_y(iced::Center)
    )
    .on_press(Message::ProfileViewCreationConfirm)
    .padding([6, 14])
    .style(button::primary);

    let cancel_btn = button(text("إلغاء (Cancel)").size(11))
        .on_press(Message::ProfileViewCreationCancel)
        .padding([6, 12])
        .style(button::secondary);

    let actions_row = row![
        Space::new().width(Fill),
        cancel_btn,
        confirm_btn,
    ]
    .spacing(8)
    .align_y(iced::Center);

    let body = column![
        title_bar,
        align_section,
        surfs_section,
        grid_bands_section,
        actions_row,
    ]
    .spacing(8);

    container(body)
        .padding([10, 14])
        .width(iced::Length::Fixed(520.0))
        .style(|theme: &Theme| {
            let palette = theme.palette();
            container::Style {
                background: Some(Background::Color(palette.background.weak.color)),
                border: Border {
                    color: palette.primary.strong.color,
                    width: 1.5,
                    radius: 6.0.into(),
                },
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                    offset: iced::Vector::new(0.0, 4.0),
                    blur_radius: 12.0,
                },
                ..Default::default()
            }
        })
        .into()
}

