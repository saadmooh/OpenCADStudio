use crate::app::Message;
use crate::i18n::lang_str;
use crate::ui::style::form::{dialog_button, field_style};
use iced::widget::{
    button, checkbox, column, container, pick_list, row, text, text_input, Space,
};
use iced::{Border, Element, Fill, Length, Theme};
use std::borrow::Cow;

/// Available Civil 3D Parcel Styles.
pub const PARCEL_STYLES: &[&str] = &[
    "Single-Family",
    "Commercial",
    "Civic",
    "Park / Open Space",
    "Standard",
    "Property",
];

/// Available Area Label Styles.
pub const AREA_LABEL_STYLES: &[&str] = &[
    "Parcel Name, Area & Perimeter",
    "Standard",
    "Name Only",
    "Area Only",
];

/// Available Line Segment Label Styles.
pub const LINE_LABEL_STYLES: &[&str] = &[
    "Bearing over Distance",
    "Distance Only",
    "Bearing Only",
    "None",
];

/// Available Curve Segment Label Styles.
pub const CURVE_LABEL_STYLES: &[&str] = &[
    "Distance over Radius",
    "Delta over Length",
    "Radius over Length",
    "None",
];

/// Maps a Parcel Style to its corresponding thematic CAD layer.
pub fn layer_for_style(style: &str) -> &'static str {
    let s = style.to_ascii_lowercase();
    if s.contains("comm") || s.contains("ret") {
        "PC-COMMERCIAL"
    } else if s.contains("civ") || s.contains("pub") || s.contains("inst") {
        "PC-CIVIC"
    } else if s.contains("park") || s.contains("open") || s.contains("green") {
        "PC-PARK"
    } else if s.contains("row") || s.contains("street") || s.contains("road") {
        "PC-ROW"
    } else if s.contains("single") || s.contains("resid") || s.contains("family") || s.contains("r1") || s.contains("r-1") {
        "PC-RESIDENTIAL"
    } else {
        "PC-PARCEL"
    }
}

/// State for the Civil 3D "Create Parcels - Layout" dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct CreateParcelsLayoutState {
    pub site: String,
    pub available_sites: Vec<String>,
    pub parcel_style: String,
    pub parcel_layer: String,
    pub parcel_segment_layer: String,
    pub area_label_style: String,
    pub line_label_style: String,
    pub curve_label_style: String,
    pub auto_add_segment_labels: bool,
    pub target_command: Option<String>,
}

impl Default for CreateParcelsLayoutState {
    fn default() -> Self {
        Self {
            site: "Site 1".to_string(),
            available_sites: vec!["Site 1".to_string()],
            parcel_style: "Single-Family".to_string(),
            parcel_layer: "PC-RESIDENTIAL".to_string(),
            parcel_segment_layer: "C-PROP-LINE".to_string(),
            area_label_style: "Parcel Name, Area & Perimeter".to_string(),
            line_label_style: "Bearing over Distance".to_string(),
            curve_label_style: "Distance over Radius".to_string(),
            auto_add_segment_labels: true,
            target_command: None,
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
        .spacing(8),
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

/// Render the Civil 3D "Create Parcels - Layout" modal dialog.
pub fn view_window<'a>(
    state: &'a CreateParcelsLayoutState,
    _sizing: crate::ui::modal::ModalSizing,
) -> Element<'a, Message> {
    // 1. Site row
    let site_pick = pick_list(
        Some(state.site.clone()),
        state.available_sites.clone(),
        |s| s.to_string(),
    )
    .on_select(Message::CreateParcelsLayoutSiteChanged)
    .text_size(12)
    .padding([4, 8])
    .width(Fill);

    let new_site_btn = button(text("+").size(14))
        .on_press(Message::CreateParcelsLayoutNewSite)
        .padding([4, 10])
        .style(button::secondary);

    let site_row = row![
        text(lang_str("Site:", "الموقع / المشروع:", "Site:"))
            .size(12)
            .width(Length::Fixed(120.0)),
        site_pick,
        new_site_btn,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    // 2. Parcel Style row
    let style_pick = pick_list(
        Some(state.parcel_style.clone()),
        PARCEL_STYLES.iter().map(|&s| s.to_string()).collect::<Vec<_>>(),
        |s| crate::i18n::display_parcel_term(s),
    )
    .on_select(Message::CreateParcelsLayoutStyleChanged)
    .text_size(12)
    .padding([4, 8])
    .width(Fill);

    let style_row = row![
        text(lang_str("Parcel style:", "نمط قطعة الأرض:", "Style de parcelle:"))
            .size(12)
            .width(Length::Fixed(120.0)),
        style_pick,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    // 3. Layers Group Box
    let parcel_layer_input = text_input("", &state.parcel_layer)
        .on_input(Message::CreateParcelsLayoutLayerChanged)
        .size(12)
        .padding([4, 8])
        .style(field_style)
        .width(Fill);

    let seg_layer_input = text_input("", &state.parcel_segment_layer)
        .on_input(Message::CreateParcelsLayoutSegmentLayerChanged)
        .size(12)
        .padding([4, 8])
        .style(field_style)
        .width(Fill);

    let layers_content = column![
        row![
            text(lang_str("Parcel layer:", "طبقة قطعة الأرض:", "Calque de la parcelle:"))
                .size(12)
                .width(Length::Fixed(140.0)),
            parcel_layer_input,
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
        row![
            text(lang_str("Parcel segment layer:", "طبقة أضلاع القطعة:", "Calque des segments:"))
                .size(12)
                .width(Length::Fixed(140.0)),
            seg_layer_input,
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    ]
    .spacing(6);

    let layers_group = group_box(lang_str("Layers", "الطبقات (Layers)", "Calques"), layers_content);

    // 4. Label Styles Group Box
    let area_style_pick = pick_list(
        Some(state.area_label_style.clone()),
        AREA_LABEL_STYLES.iter().map(|&s| s.to_string()).collect::<Vec<_>>(),
        |s| crate::i18n::display_parcel_term(s),
    )
    .on_select(Message::CreateParcelsLayoutAreaLabelStyleChanged)
    .text_size(12)
    .padding([4, 8])
    .width(Fill);

    let line_style_pick = pick_list(
        Some(state.line_label_style.clone()),
        LINE_LABEL_STYLES.iter().map(|&s| s.to_string()).collect::<Vec<_>>(),
        |s| crate::i18n::display_parcel_term(s),
    )
    .on_select(Message::CreateParcelsLayoutLineLabelStyleChanged)
    .text_size(12)
    .padding([4, 8])
    .width(Fill);

    let curve_style_pick = pick_list(
        Some(state.curve_label_style.clone()),
        CURVE_LABEL_STYLES.iter().map(|&s| s.to_string()).collect::<Vec<_>>(),
        |s| crate::i18n::display_parcel_term(s),
    )
    .on_select(Message::CreateParcelsLayoutCurveLabelStyleChanged)
    .text_size(12)
    .padding([4, 8])
    .width(Fill);

    let auto_labels_check = row![
        checkbox(state.auto_add_segment_labels)
            .on_toggle(Message::CreateParcelsLayoutAutoLabelsToggled)
            .size(14),
        text(lang_str("Automatically add segment labels", "إضافة نصوص الأضلاع تلقائياً", "Ajouter automatiquement les étiquettes")).size(11),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);

    let label_styles_content = column![
        row![
            text(lang_str("Area label style:", "نمط نص المساحة والاسم:", "Style d'étiquette de surface:"))
                .size(12)
                .width(Length::Fixed(160.0)),
            area_style_pick,
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
        row![
            text(lang_str("Line segment label style:", "نمط نص الأضلاع المستقيمة:", "Style d'étiquette de ligne:"))
                .size(12)
                .width(Length::Fixed(160.0)),
            line_style_pick,
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
        row![
            text(lang_str("Curve segment label style:", "نمط نص المنحنيات:", "Style d'étiquette de courbe:"))
                .size(12)
                .width(Length::Fixed(160.0)),
            curve_style_pick,
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
        auto_labels_check,
    ]
    .spacing(6);

    let label_styles_group = group_box(lang_str("Label styles", "أنماط النصوص والبيانات", "Styles d'étiquettes"), label_styles_content);

    // 5. Action buttons footer (OK, Cancel, Help)
    let footer_row = row![
        Space::new().width(Fill),
        dialog_button(lang_str("OK", "موافق", "OK"), Message::CreateParcelsLayoutConfirm, true),
        dialog_button(lang_str("Cancel", "إلغاء", "Annuler"), Message::CloseModal, false),
        dialog_button(lang_str("Help", "تعليمات", "Aide"), Message::CreateParcelsLayoutHelp, false),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    container(
        column![
            site_row,
            style_row,
            layers_group,
            label_styles_group,
            Space::new().height(Length::Fixed(4.0)),
            footer_row,
        ]
        .spacing(12),
    )
    .padding([14, 16])
    .width(Fill)
    .into()
}
