use crate::app::Message;
use crate::ui::style::form::{dialog_button, dialog_button_styled};
use iced::mouse::Cursor;
use iced::widget::canvas::{self, Canvas, Frame, Geometry, Path, Stroke, Text};
use iced::widget::{button, checkbox, column, container, pick_list, row, scrollable, text, text_input, Space};
use iced::{Background, Border, Color, Element, Fill, Point, Rectangle, Renderer, Size, Theme};
use std::borrow::Cow;

/// State for the native Civil 3D Parcel Layout Tools floating dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct ParcelLayoutToolsState {
    pub min_area: String,
    pub min_frontage: String,
    pub use_offset: bool,
    pub frontage_offset: String,
    pub min_width: String,
    pub min_depth: String,
    pub use_max_depth: bool,
    pub max_depth: String,
    pub solution_preference: String,
    pub automatic_mode: bool,
    pub remainder_mode: String,

    pub active_tool: String,
    pub status_message: String,
}

impl Default for ParcelLayoutToolsState {
    fn default() -> Self {
        Self {
            min_area: "350.00".to_string(),
            min_frontage: "12.00".to_string(),
            use_offset: true,
            frontage_offset: "4.00".to_string(),
            min_width: "10.00".to_string(),
            min_depth: "20.00".to_string(),
            use_max_depth: false,
            max_depth: "80.00".to_string(),
            solution_preference: "Use shortest frontage".to_string(),
            automatic_mode: false,
            remainder_mode: "Place remainder in last parcel".to_string(),

            active_tool: "Selection".to_string(),
            status_message: "Ready. Select a parcel boundary or pick a layout tool from the toolbar.".to_string(),
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

/// Canvas Program rendering the real-time dynamic Civil 3D Parcel Sizing graphic illustration.
pub struct ParcelIllustrationCanvas {
    pub min_area: f64,
    pub min_frontage: f64,
    pub use_offset: bool,
    pub frontage_offset: f64,
    pub min_width: f64,
    pub min_depth: f64,
    pub active_tool: String,
}

impl canvas::Program<Message> for ParcelIllustrationCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let w = bounds.width;
        let h = bounds.height;

        // 1. Dark Blueprint Background
        let bg_rect = Path::rectangle(Point::ORIGIN, bounds.size());
        frame.fill(&bg_rect, Color::from_rgb(0.08, 0.11, 0.15));

        // 2. Blueprint Grid Lines
        let grid_step = 20.0;
        let grid_color = Color::from_rgba(0.2, 0.3, 0.4, 0.25);
        let mut x = 0.0;
        while x < w {
            let line = Path::line(Point::new(x, 0.0), Point::new(x, h));
            frame.stroke(&line, Stroke::default().with_color(grid_color).with_width(1.0));
            x += grid_step;
        }
        let mut y = 0.0;
        while y < h {
            let line = Path::line(Point::new(0.0, y), Point::new(w, y));
            frame.stroke(&line, Stroke::default().with_color(grid_color).with_width(1.0));
            y += grid_step;
        }

        // 3. Coordinate mapping for dynamic lot preview
        // Compute scaled lot rectangle based on frontage, width and depth
        let aspect = (self.min_frontage.max(self.min_width) / self.min_depth.max(10.0)).clamp(0.4, 2.5) as f32;
        let lot_h = (h * 0.55).min(w * 0.55 / aspect);
        let lot_w = lot_h * aspect;

        let ox = (w - lot_w) * 0.5;
        let oy = (h - lot_h) * 0.45;

        // Lot polygon fill
        let lot_poly = Path::new(|p| {
            p.move_to(Point::new(ox, oy));
            p.line_to(Point::new(ox + lot_w, oy));
            p.line_to(Point::new(ox + lot_w * 0.95, oy + lot_h));
            p.line_to(Point::new(ox + lot_w * 0.05, oy + lot_h));
            p.close();
        });
        frame.fill(&lot_poly, Color::from_rgba(0.12, 0.45, 0.75, 0.22));
        frame.stroke(&lot_poly, Stroke::default().with_color(Color::from_rgb(0.25, 0.70, 0.95)).with_width(2.0));

        // 4. Frontage Line (Front boundary along roadway - Emerald / Gold)
        let frontage_line = Path::line(
            Point::new(ox + lot_w * 0.05, oy + lot_h),
            Point::new(ox + lot_w * 0.95, oy + lot_h),
        );
        frame.stroke(&frontage_line, Stroke::default().with_color(Color::from_rgb(0.95, 0.75, 0.15)).with_width(3.5));

        // 5. Frontage Offset (Dashed Setback Line)
        if self.use_offset && self.frontage_offset > 0.0 {
            let offset_ratio = (self.frontage_offset / self.min_depth.max(10.0)).clamp(0.05, 0.35) as f32;
            let offset_y = oy + lot_h - lot_h * offset_ratio;

            let setback_line = Path::line(
                Point::new(ox + lot_w * 0.08, offset_y),
                Point::new(ox + lot_w * 0.92, offset_y),
            );
            frame.stroke(
                &setback_line,
                Stroke::default()
                    .with_color(Color::from_rgb(0.30, 0.85, 0.50))
                    .with_width(1.5),
            );

            // Offset label
            frame.fill_text(Text {
                content: format!("Setback / Offset: {:.1}m", self.frontage_offset),
                position: Point::new(ox + 8.0, offset_y - 12.0),
                color: Color::from_rgb(0.30, 0.85, 0.50),
                size: 10.0.into(),
                ..Default::default()
            });
        }

        // 6. Depth Dimension Arrow
        let depth_x = ox + lot_w + 12.0;
        let depth_line = Path::line(Point::new(depth_x, oy), Point::new(depth_x, oy + lot_h));
        frame.stroke(&depth_line, Stroke::default().with_color(Color::from_rgb(0.85, 0.85, 0.85)).with_width(1.0));

        // Depth arrows
        let d_arr1 = Path::line(Point::new(depth_x - 3.0, oy + 4.0), Point::new(depth_x, oy));
        let d_arr2 = Path::line(Point::new(depth_x + 3.0, oy + 4.0), Point::new(depth_x, oy));
        let d_arr3 = Path::line(Point::new(depth_x - 3.0, oy + lot_h - 4.0), Point::new(depth_x, oy + lot_h));
        let d_arr4 = Path::line(Point::new(depth_x + 3.0, oy + lot_h - 4.0), Point::new(depth_x, oy + lot_h));
        frame.stroke(&d_arr1, Stroke::default().with_color(Color::WHITE).with_width(1.0));
        frame.stroke(&d_arr2, Stroke::default().with_color(Color::WHITE).with_width(1.0));
        frame.stroke(&d_arr3, Stroke::default().with_color(Color::WHITE).with_width(1.0));
        frame.stroke(&d_arr4, Stroke::default().with_color(Color::WHITE).with_width(1.0));

        frame.fill_text(Text {
            content: format!("Depth: {:.1}m", self.min_depth),
            position: Point::new(depth_x + 6.0, oy + lot_h * 0.45),
            color: Color::from_rgb(0.9, 0.9, 0.9),
            size: 10.0.into(),
            ..Default::default()
        });

        // 7. Frontage Dimension Label
        frame.fill_text(Text {
            content: format!("Min Frontage = {:.1}m", self.min_frontage),
            position: Point::new(ox + lot_w * 0.25, oy + lot_h + 8.0),
            color: Color::from_rgb(0.95, 0.75, 0.15),
            size: 10.5.into(),
            ..Default::default()
        });

        // 8. Target Lot Area Badge (Center of lot)
        let badge_w = 140.0;
        let badge_h = 24.0;
        let badge_x = ox + (lot_w - badge_w) * 0.5;
        let badge_y = oy + (lot_h - badge_h) * 0.5;

        let badge_bg = Path::rectangle(Point::new(badge_x, badge_y), Size::new(badge_w, badge_h));
        frame.fill(&badge_bg, Color::from_rgba(0.0, 0.0, 0.0, 0.75));
        frame.stroke(&badge_bg, Stroke::default().with_color(Color::from_rgb(0.25, 0.70, 0.95)).with_width(1.0));

        frame.fill_text(Text {
            content: format!("Target: {:.1} m²", self.min_area),
            position: Point::new(badge_x + 12.0, badge_y + 6.0),
            color: Color::from_rgb(0.35, 0.85, 1.0),
            size: 11.0.into(),
            ..Default::default()
        });

        // 9. Slide Vector Cut Arrow
        let arrow_y = oy + lot_h * 0.25;
        let arrow_line = Path::line(Point::new(ox + 10.0, arrow_y), Point::new(ox + 40.0, arrow_y));
        frame.stroke(&arrow_line, Stroke::default().with_color(Color::from_rgb(1.0, 0.4, 0.4)).with_width(2.0));
        let arr_head1 = Path::line(Point::new(ox + 35.0, arrow_y - 4.0), Point::new(ox + 40.0, arrow_y));
        let arr_head2 = Path::line(Point::new(ox + 35.0, arrow_y + 4.0), Point::new(ox + 40.0, arrow_y));
        frame.stroke(&arr_head1, Stroke::default().with_color(Color::from_rgb(1.0, 0.4, 0.4)).with_width(2.0));
        frame.stroke(&arr_head2, Stroke::default().with_color(Color::from_rgb(1.0, 0.4, 0.4)).with_width(2.0));

        frame.fill_text(Text {
            content: "Slide Cut".to_string(),
            position: Point::new(ox + 10.0, arrow_y - 12.0),
            color: Color::from_rgb(1.0, 0.5, 0.5),
            size: 9.5.into(),
            ..Default::default()
        });

        // 10. Top-Right Mode Badge
        frame.fill_text(Text {
            content: format!("Mode: {}", self.active_tool),
            position: Point::new(w - 120.0, 10.0),
            color: Color::from_rgb(0.35, 0.85, 1.0),
            size: 10.0.into(),
            ..Default::default()
        });

        vec![frame.into_geometry()]
    }
}

pub fn view_window<'a>(
    state: &'a ParcelLayoutToolsState,
    sizing: crate::ui::modal::ModalSizing,
) -> Element<'a, Message> {
    // ── Top Civil 3D Action Toolbar ───────────────────────────────────────
    let tool_btn = |icon: &'static str, label: &'static str, action: &'static str| {
        let is_active = state.active_tool == action;
        button(
            row![
                text(icon).size(13),
                text(label).size(11),
            ]
            .spacing(4)
            .align_y(iced::Center)
        )
        .on_press(Message::ParcelLayoutAction(action.to_string()))
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
                    radius: 4.0.into(),
                },
                ..Default::default()
            }
        })
        .padding([4, 8])
    };

    let toolbar_row1 = row![
        tool_btn("✏", "Create Lot", "create"),
        tool_btn("↔", "Slide Line", "slide"),
        tool_btn("🔄", "Swing Line", "swing"),
        tool_btn("✂", "Split", "split"),
    ]
    .spacing(4);

    let toolbar_row2 = row![
        tool_btn("⚡", "Auto Subdiv", "subdivide"),
        tool_btn("🔗", "Merge", "merge"),
        tool_btn("📐", "Setback", "setback"),
        tool_btn("🎨", "Thematic", "thematic"),
    ]
    .spacing(4);

    let toolbar_container = container(
        column![toolbar_row1, toolbar_row2].spacing(4)
    )
    .padding([6, 8])
    .width(Fill)
    .style(|theme: &Theme| container::Style {
        background: Some(Background::Color(theme.palette().background.weak.color)),
        border: Border {
            width: 1.0,
            radius: 4.0.into(),
            color: theme.palette().background.neutral.color,
        },
        ..Default::default()
    });

    // ── Left Column: Sizing Parameters Controls ───────────────────────────
    let field_input = |lbl: &'static str, unit: &'static str, val: &str, on_change: fn(String) -> Message| {
        row![
            text(lbl).size(11).width(130),
            text_input("0.0", val)
                .on_input(on_change)
                .size(11)
                .padding([3, 6])
                .width(iced::Length::Fixed(80.0)),
            text(unit).size(11),
        ]
        .spacing(6)
        .align_y(iced::Center)
    };

    let min_area_row = field_input("Minimum Area:", "sq.m", &state.min_area, Message::ParcelLayoutMinAreaChanged);
    let min_frontage_row = field_input("Minimum Frontage:", "m", &state.min_frontage, Message::ParcelLayoutMinFrontageChanged);

    let offset_toggle_row = row![
        checkbox(state.use_offset)
            .on_toggle(Message::ParcelLayoutUseOffsetToggled)
            .size(14),
        text("Use Min Frontage at Offset").size(11),
    ]
    .spacing(6)
    .align_y(iced::Center);

    let frontage_offset_row = field_input("Frontage Offset:", "m", &state.frontage_offset, Message::ParcelLayoutOffsetChanged);
    let min_width_row = field_input("Minimum Width:", "m", &state.min_width, Message::ParcelLayoutMinWidthChanged);
    let min_depth_row = field_input("Minimum Depth:", "m", &state.min_depth, Message::ParcelLayoutMinDepthChanged);

    let max_depth_toggle_row = row![
        checkbox(state.use_max_depth)
            .on_toggle(Message::ParcelLayoutUseMaxDepthToggled)
            .size(14),
        text("Use Maximum Depth").size(11),
    ]
    .spacing(6)
    .align_y(iced::Center);

    let max_depth_row = field_input("Maximum Depth:", "m", &state.max_depth, Message::ParcelLayoutMaxDepthChanged);

    let sizing_group = group_box(
        "Parcel Sizing Criteria",
        column![
            min_area_row,
            min_frontage_row,
            offset_toggle_row,
            frontage_offset_row,
            min_width_row,
            min_depth_row,
            max_depth_toggle_row,
            max_depth_row,
        ]
        .spacing(6),
    );

    // Automatic Layout group
    let pref_list = vec![
        "Use shortest frontage".to_string(),
        "Use smallest area".to_string(),
        "Use largest frontage".to_string(),
    ];

    let remainder_list = vec![
        "Place remainder in last parcel".to_string(),
        "Redistribute remainder".to_string(),
        "Create remainder parcel".to_string(),
    ];

    let pref_row = row![
        text("Solution Preference:").size(11).width(130),
        pick_list(
            Some(state.solution_preference.clone()),
            pref_list,
            |s| s.to_string(),
        )
        .on_select(Message::ParcelLayoutPreferenceChanged)
        .text_size(10.5)
        .padding([3, 6])
        .width(iced::Length::Fixed(180.0)),
    ]
    .spacing(6)
    .align_y(iced::Center);

    let auto_toggle_row = row![
        checkbox(state.automatic_mode)
            .on_toggle(Message::ParcelLayoutAutoModeToggled)
            .size(14),
        text("Automatic Mode (Subdivide entire site)").size(11),
    ]
    .spacing(6)
    .align_y(iced::Center);

    let remainder_row = row![
        text("Remainder Distribution:").size(11).width(130),
        pick_list(
            Some(state.remainder_mode.clone()),
            remainder_list,
            |s| s.to_string(),
        )
        .on_select(Message::ParcelLayoutRemainderChanged)
        .text_size(10.5)
        .padding([3, 6])
        .width(iced::Length::Fixed(180.0)),
    ]
    .spacing(6)
    .align_y(iced::Center);

    let auto_group = group_box(
        "Automatic Layout Criteria",
        column![pref_row, auto_toggle_row, remainder_row].spacing(6),
    );

    let left_column = column![sizing_group, auto_group]
        .spacing(8)
        .width(iced::Length::Fixed(360.0));

    // ── Right Column: Dynamic Real-time Canvas Graphic Illustration ───────
    let canvas_prog = ParcelIllustrationCanvas {
        min_area: state.min_area.parse().unwrap_or(350.0),
        min_frontage: state.min_frontage.parse().unwrap_or(12.0),
        use_offset: state.use_offset,
        frontage_offset: state.frontage_offset.parse().unwrap_or(4.0),
        min_width: state.min_width.parse().unwrap_or(10.0),
        min_depth: state.min_depth.parse().unwrap_or(20.0),
        active_tool: state.active_tool.clone(),
    };

    let illustration_canvas = Canvas::new(canvas_prog)
        .width(Fill)
        .height(Fill);

    let canvas_container = container(illustration_canvas)
        .width(Fill)
        .height(Fill)
        .style(|_theme: &Theme| container::Style {
            border: Border {
                width: 1.0,
                radius: 4.0.into(),
                color: Color::from_rgb(0.2, 0.3, 0.4),
            },
            ..Default::default()
        });

    let right_column = group_box(
        "Dynamic Illustration Preview (Real-Time)",
        canvas_container,
    );

    let main_content = row![left_column, right_column]
        .spacing(10)
        .width(Fill)
        .height(Fill);

    // ── Status Bar Strip ──────────────────────────────────────────────────
    let status_strip = container(
        row![
            text("ℹ").size(12),
            text(&state.status_message).size(10.5).style(crate::ui::style::common::muted_style),
        ]
        .spacing(6)
        .align_y(iced::Center)
    )
    .padding([4, 8])
    .width(Fill)
    .style(|theme: &Theme| container::Style {
        background: Some(Background::Color(theme.palette().background.weak.color)),
        border: Border {
            width: 1.0,
            radius: 3.0.into(),
            color: theme.palette().background.neutral.color,
        },
        ..Default::default()
    });

    // ── Bottom Action Buttons ─────────────────────────────────────────────
    let apply_btn = dialog_button("Apply Criteria", Message::ParcelLayoutApply, true);
    let close_btn = dialog_button_styled("Close", Message::ParcelLayoutClose, button::secondary);
    let help_btn = dialog_button_styled("Help", Message::ParcelLayoutHelp, button::text);

    let actions_row = row![
        help_btn,
        Space::new().width(Fill),
        apply_btn,
        close_btn,
    ]
    .spacing(8)
    .align_y(iced::Center);

    // ── Body Layout ───────────────────────────────────────────────────────
    let body = column![
        toolbar_container,
        Space::new().height(4),
        scrollable(main_content).height(Fill),
        Space::new().height(4),
        status_strip,
        Space::new().height(6),
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
