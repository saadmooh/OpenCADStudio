use crate::app::commands::civil_parcel::ParcelData;
use crate::app::Message;
use crate::i18n::lang_str;
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

    // Style & Label Configuration (Civil 3D)
    pub area_label_style: String,
    pub line_label_style: String,
    pub curve_label_style: String,
    pub auto_add_segment_labels: bool,
    pub segment_layer: String,

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
            selected_style: if !p.zoning.is_empty() {
                p.zoning.clone()
            } else if p.layer.contains("RESIDENTIAL") {
                "Single-Family".to_string()
            } else if p.layer.contains("COMMERCIAL") {
                "Commercial".to_string()
            } else if p.layer.contains("CIVIC") {
                "Civic".to_string()
            } else if p.layer.contains("PARK") {
                "Park / Open Space".to_string()
            } else {
                "Standard".to_string()
            },
            selected_layer: p.layer.clone(),
            zoning: p.zoning.clone(),
            object_locked: false,
            show_tooltips: true,

            area_label_style: if !p.area_label_style.is_empty() {
                p.area_label_style.clone()
            } else {
                "Parcel Name, Area & Perimeter".to_string()
            },
            line_label_style: if !p.line_label_style.is_empty() {
                p.line_label_style.clone()
            } else {
                "Bearing over Distance".to_string()
            },
            curve_label_style: if !p.curve_label_style.is_empty() {
                p.curve_label_style.clone()
            } else {
                "Distance over Radius".to_string()
            },
            auto_add_segment_labels: p.auto_add_segment_labels,
            segment_layer: if !p.segment_layer.is_empty() {
                p.segment_layer.clone()
            } else {
                "C-PROP-LINE".to_string()
            },

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
            hatch_pattern: "ANSI31".to_string(),

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
        tab_button(ParcelPropertiesTab::Information, lang_str("Information", "المعلومات العامة", "Informations")),
        tab_button(ParcelPropertiesTab::Composition, lang_str("Composition", "الملكية والتركيب", "Composition")),
        tab_button(ParcelPropertiesTab::Analysis, lang_str("Analysis", "التحليل والمطابقة", "Analyse")),
        tab_button(ParcelPropertiesTab::Statistics, lang_str("Statistics", "البيانات والقياسات", "Statistiques")),
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
        "Single-Family".to_string(),
        "Commercial".to_string(),
        "Civic".to_string(),
        "Park / Open Space".to_string(),
        "Standard".to_string(),
        "Property".to_string(),
        "Multi-Family Residential".to_string(),
        "Industrial / Logistics".to_string(),
    ];

    let area_label_styles_list = vec![
        "Parcel Name, Area & Perimeter".to_string(),
        "Parcel Name & Area".to_string(),
        "Name Only".to_string(),
        "Area Only".to_string(),
        "Square Meters & Acres".to_string(),
    ];

    let line_label_styles_list = vec![
        "Bearing over Distance".to_string(),
        "Distance Only".to_string(),
        "Bearing Only".to_string(),
        "None".to_string(),
    ];

    let curve_label_styles_list = vec![
        "Distance over Radius".to_string(),
        "Delta over Length".to_string(),
        "Radius over Length".to_string(),
        "Length Only".to_string(),
        "Radius Only".to_string(),
        "None".to_string(),
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
            text(lang_str("Parcel Name:", "اسم قطعة الأرض:", "Nom de la parcelle:")).size(11.5),
            text_input("Parcel Name", &state.name)
                .on_input(Message::ParcelPropertiesNameChanged)
                .size(11.5)
                .padding([5, 8])
                .width(Fill),
        ]
        .spacing(4)
        .width(Fill);

        let num_col = column![
            text(lang_str("Parcel Number:", "رقم القطعة:", "Numéro de parcelle:")).size(11.5),
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
            text(lang_str("Site:", "الموقع / المشروع:", "Site:")).size(11.5),
            text_input("Site 1", &state.site)
                .on_input(Message::ParcelPropertiesSiteChanged)
                .size(11.5)
                .padding([5, 8])
                .width(Fill),
        ]
        .spacing(4);

        // Description
        let desc_col = column![
            text(lang_str("Description:", "الوصف:", "Description:")).size(11.5),
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
            |s| crate::i18n::display_parcel_term(s),
        )
        .on_select(Message::ParcelPropertiesStyleChanged)
        .text_size(11)
        .padding([4, 6])
        .width(Fill);

        let style_row = row![
            row![text("📐").size(12), text(lang_str("Parcel style:", "نمط قطعة الأرض:", "Style de parcelle:")).size(11.5)]
                .spacing(4)
                .width(130)
                .align_y(iced::Center),
            style_picker,
        ]
        .spacing(6)
        .align_y(iced::Center);

        // Parcel Layer
        let layer_row = row![
            row![text("📑").size(12), text(lang_str("Parcel CAD layer:", "طبقة الكاد:", "Calque CAO:")).size(11.5)]
                .spacing(4)
                .width(130)
                .align_y(iced::Center),
            text_input("Parcel Layer", &state.selected_layer)
                .on_input(Message::ParcelPropertiesLayerChanged)
                .size(11)
                .padding([4, 6])
                .width(Fill),
        ]
        .spacing(6)
        .align_y(iced::Center);

        // Area Label Style picker
        let area_style_picker = pick_list(
            Some(state.area_label_style.clone()),
            area_label_styles_list,
            |s| crate::i18n::display_parcel_term(s),
        )
        .on_select(Message::ParcelPropertiesAreaLabelStyleChanged)
        .text_size(11)
        .padding([4, 6])
        .width(Fill);

        let area_style_row = row![
            row![text("🏷").size(12), text(lang_str("Area label style:", "نمط نص المساحة والاسم:", "Style d'étiquette de surface:")).size(11.5)]
                .spacing(4)
                .width(130)
                .align_y(iced::Center),
            area_style_picker,
        ]
        .spacing(6)
        .align_y(iced::Center);

        // Line Label Style picker
        let line_style_picker = pick_list(
            Some(state.line_label_style.clone()),
            line_label_styles_list,
            |s| crate::i18n::display_parcel_term(s),
        )
        .on_select(Message::ParcelPropertiesLineLabelStyleChanged)
        .text_size(11)
        .padding([4, 6])
        .width(Fill);

        let line_style_row = row![
            row![text("📏").size(12), text(lang_str("Line label style:", "نمط نص الخطوط:", "Style d'étiquette de ligne:")).size(11.5)]
                .spacing(4)
                .width(130)
                .align_y(iced::Center),
            line_style_picker,
        ]
        .spacing(6)
        .align_y(iced::Center);

        // Curve Label Style picker
        let curve_style_picker = pick_list(
            Some(state.curve_label_style.clone()),
            curve_label_styles_list,
            |s| crate::i18n::display_parcel_term(s),
        )
        .on_select(Message::ParcelPropertiesCurveLabelStyleChanged)
        .text_size(11)
        .padding([4, 6])
        .width(Fill);

        let curve_style_row = row![
            row![text("⌒").size(12), text(lang_str("Curve label style:", "نمط نص المنحنيات:", "Style d'étiquette de courbe:")).size(11.5)]
                .spacing(4)
                .width(130)
                .align_y(iced::Center),
            curve_style_picker,
        ]
        .spacing(6)
        .align_y(iced::Center);

        // Auto Add Segment Labels Checkbox & Layer
        let auto_seg_check = row![
            checkbox(state.auto_add_segment_labels)
                .on_toggle(Message::ParcelPropertiesToggleSegmentLabels)
                .size(14),
            text(lang_str("Automatically add boundary segment labels", "إضافة نصوص أضلاع الحدود تلقائياً", "Ajouter automatiquement les étiquettes")).size(11.5),
        ]
        .spacing(6)
        .align_y(iced::Center);

        let seg_layer_row = row![
            row![text("🔤").size(12), text(lang_str("Segment layer:", "طبقة أضلاع القطعة:", "Calque des segments:")).size(11.5)]
                .spacing(4)
                .width(130)
                .align_y(iced::Center),
            text_input("Segment Layer", &state.segment_layer)
                .on_input(Message::ParcelPropertiesSegmentLayerChanged)
                .size(11)
                .padding([4, 6])
                .width(Fill),
        ]
        .spacing(6)
        .align_y(iced::Center);

        // Zoning code picker
        let zoning_picker = pick_list(
            Some(state.zoning.clone()),
            zoning_list,
            |s| crate::i18n::display_parcel_term(s),
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
            row![text("🏛").size(12), text(lang_str("Zoning code:", "رمز التنظيم والتخصيص:", "Code de zonage:")).size(11.5)]
                .spacing(4)
                .width(130)
                .align_y(iced::Center),
            zoning_picker,
            swatch,
        ]
        .spacing(6)
        .align_y(iced::Center);

        let styles_group = group_box(
            lang_str("Civil 3D Styles & Annotation Labels", "أنماط Civil 3D ونصوص البيانات", "Styles Civil 3D & Étiquettes"),
            column![
                style_row,
                layer_row,
                area_style_row,
                line_style_row,
                curve_style_row,
                auto_seg_check,
                seg_layer_row,
                zoning_row,
            ]
            .spacing(8)
            .padding([4, 2]),
        );

        let lock_check = row![
            checkbox(state.object_locked)
                .on_toggle(|_| Message::ParcelPropertiesToggleLocked)
                .size(14),
            text(lang_str("Object locked (protect from layout tools)", "قفل العنصر (حماية من التعديل التلقائي)", "Objet verrouillé (protéger des outils)")).size(11.5),
        ]
        .spacing(6)
        .align_y(iced::Center);

        let tooltips_check = row![
            checkbox(state.show_tooltips)
                .on_toggle(|_| Message::ParcelPropertiesToggleTooltips)
                .size(14),
            text(lang_str("Show parcel hover tooltips & area callouts", "إظهار تلميحات المرور وتسميات المساحة", "Afficher les info-bulles au survol")).size(11.5),
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
            field_row(lang_str("Legal Owner:", "المالك القانوني:", "Propriétaire légal:"), &state.owner, Message::ParcelPropertiesOwnerChanged),
            field_row(lang_str("Deed Reference:", "رقم صك الملكية:", "Réf. de l'acte:"), &state.deed_no, Message::ParcelPropertiesDeedChanged),
            field_row(lang_str("Tax Parcel ID (PIN):", "الرقم الضريبي (PIN):", "ID fiscal (PIN):"), &state.tax_id, Message::ParcelPropertiesTaxIdChanged),
            field_row(lang_str("Street Address:", "عنوان العقار / الشارع:", "Adresse physique:"), &state.address, Message::ParcelPropertiesAddressChanged),
            field_row(lang_str("Assessment Value:", "القيمة التقديرية للأرض:", "Valeur fiscale estimée:"), &state.assessment_val, Message::ParcelPropertiesAssessmentChanged),
            field_row(lang_str("Land Use Category:", "فئة استخدام الأرض:", "Catégorie d'utilisation:"), &state.land_use, Message::ParcelPropertiesLandUseChanged),
        ]
        .spacing(8);

        let legal_group = group_box(lang_str("Legal Identity & Tax Assessment", "البيانات القانونية والتقييم العقاري", "Identité juridique & Évaluation"), legal_col);

        let note = text(lang_str(
            "Ownership and deed data are preserved in the drawing via CAD Extended Data (XDATA OPENCAD_PARCEL).",
            "تُحفظ بيانات الملكية والصك في ملف الرسم عبر البيانات الموسعة XDATA OPENCAD_PARCEL.",
            "Les données de propriété sont conservées via les données étendues XDATA OPENCAD_PARCEL."
        ))
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
            mapcheck_row(lang_str("Error of Closure:", "خطأ الإغلاق:", "Erreur de fermeture:"), format!("{:.5} m", state.mapcheck_closure_error), state.mapcheck_closure_error < 0.01),
            mapcheck_row(lang_str("Closure Bearing:", "اتجاه خطأ الإغلاق:", "Gisement de fermeture:"), state.mapcheck_bearing.clone(), true),
            mapcheck_row(lang_str("Traverse Precision Ratio:", "نسبة دقة المضلع:", "Rapport de précision:"), state.mapcheck_precision.clone(), state.mapcheck_passed),
            mapcheck_row(lang_str("Traverse Closure Criteria:", "معيار دقة إغلاق المضلع:", "Critère de fermeture:"), if state.mapcheck_passed { lang_str("PASSED (Class 1 Cadastral)", "ناجح (مطابق للمساحة الطبوغرافية)", "RÉUSSI (Classe 1 Cadastrale)") } else { lang_str("EXCEEDS TOLERANCE", "يتجاوز التفاوت المسموح", "HORS TOLÉRANCE") }.to_string(), state.mapcheck_passed),
            mapcheck_row(lang_str("Perimeter Traverse Length:", "طول محيط المضلع:", "Périmètre du cheminement:"), format!("{:.2} m", state.perimeter), true),
            mapcheck_row(lang_str("Mathematical Area Check:", "التحقق الرياضي من المساحة:", "Contrôle de surface:"), format!("{:.2} m²", state.area_2d), true),
        ]
        .spacing(6);

        let mapcheck_group = group_box(lang_str("Civil 3D Mapcheck Boundary Closure Analysis", "تحليل إغلاق الحدود الهندسية (Mapcheck)", "Analyse de fermeture de limite (Mapcheck)"), mapcheck_col);

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
                text(lang_str("Active Zoning Color:", "لون التخصيص النشط:", "Couleur de zonage:")).size(11).width(140),
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
                text(lang_str("Hatch Fill Pattern:", "نمط التهشير (Hatch):", "Motif de hachure:")).size(11).width(140),
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

        let thematic_group = group_box(lang_str("Thematic Zoning Hatching & Display", "عرض وتلوين التخصيص العقاري", "Hachures & affichage du zonage"), thematic_info);

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
            stat_row(lang_str("Parcel Handle:", "معرّف الكائن (Handle):", "Handle de l'entité:"), format!("{:X}", state.handle.value())),
            stat_row(lang_str("2D Projected Area:", "المساحة المسقطة 2D:", "Surface projetée 2D:"), format!("{:.2} m² ({:.4} ha)", state.area_2d, state.area_2d / 10000.0)),
            stat_row(lang_str("Imperial Area:", "المساحة بالوحدات الإمبراطورية:", "Surface impériale:"), format!("{:.1} sq.ft ({:.3} acres)", area_sqft, area_acres)),
            stat_row(lang_str("Boundary Perimeter:", "محيط الحدود:", "Périmètre:"), format!("{:.2} m", state.perimeter)),
            stat_row(lang_str("Boundary Segments:", "أضلاع الحدود:", "Segments de limite:"), format!("{} {}", state.segment_count, lang_str("vertices / segments", "نقاط / أضلاع", "sommets / segments"))),
        ]
        .spacing(4);

        let geom_col = column![
            stat_row(lang_str("Centroid Coordinates (X, Y):", "إحداثيات مركز الثقل (X, Y):", "Barycentre (X, Y):"), format!("{:.3}, {:.3}", state.centroid_x, state.centroid_y)),
            stat_row(lang_str("Bounding Box Min (X, Y):", "أدنى إحداثيات (Min X, Y):", "Boîte englobante Min (X, Y):"), format!("{:.3}, {:.3}", state.min_x, state.min_y)),
            stat_row(lang_str("Bounding Box Max (X, Y):", "أقصى إحداثيات (Max X, Y):", "Boîte englobante Max (X, Y):"), format!("{:.3}, {:.3}", state.max_x, state.max_y)),
            stat_row(lang_str("Width (ΔX):", "العرض (ΔX):", "Largeur (ΔX):"), format!("{:.2} m", (state.max_x - state.min_x).abs())),
            stat_row(lang_str("Depth (ΔY):", "العمق (ΔY):", "Profondeur (ΔY):"), format!("{:.2} m", (state.max_y - state.min_y).abs())),
            stat_row(lang_str("Aspect Ratio (Width / Depth):", "نسبة العرض إلى العمق:", "Rapport de forme (L/P):"), format!("{:.2}", state.aspect_ratio)),
            stat_row(lang_str("CAD Layer:", "طبقة الكاد:", "Calque CAO:"), state.selected_layer.clone()),
        ]
        .spacing(4);

        column![
            group_box(lang_str("General Geometric Statistics", "البيانات الهندسية العامة", "Statistiques géométriques générales"), general_col),
            group_box(lang_str("Spatial Bounding & Orientation", "الحدود المكانية والاتجاهات", "Limites spatiales & orientation"), geom_col),
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
    let ok_btn = dialog_button(lang_str("OK", "موافق", "OK"), Message::ParcelPropertiesOk, true);
    let cancel_btn = dialog_button(lang_str("Cancel", "إلغاء", "Annuler"), Message::ParcelPropertiesClose, false);
    let apply_btn = dialog_button_styled(lang_str("Apply", "تطبيق", "Appliquer"), Message::ParcelPropertiesApply, button::secondary);
    let help_btn = dialog_button_styled(lang_str("Help", "تعليمات", "Aide"), Message::ParcelPropertiesHelp, button::text);
    let tools_btn = dialog_button_styled(lang_str("⚡ Parcel Layout Tools", "⚡ أدوات التخطيط", "⚡ Outils de disposition"), Message::OpenParcelLayoutTools, button::secondary);

    let actions_row = row![
        help_btn,
        tools_btn,
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
