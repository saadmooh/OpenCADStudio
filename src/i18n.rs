//! Application language selection and embedded Fluent resources.

use i18n_embed::fluent::{fluent_language_loader, FluentLanguageLoader};
#[cfg(all(not(target_arch = "wasm32"), not(test)))]
use i18n_embed::DesktopLanguageRequester;
use i18n_embed::LanguageLoader;
#[cfg(target_arch = "wasm32")]
use i18n_embed::WebLanguageRequester;
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::sync::OnceLock;

#[path = "locale_catalog.rs"]
mod locale_catalog;

#[derive(RustEmbed)]
#[folder = "locales/"]
struct Localizations;

/// User-selectable UI language. `System` keeps following the platform's
/// preferred locale while explicit choices remain stable across restarts.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum Language {
    #[default]
    #[serde(rename = "system")]
    System,
    #[serde(rename = "en-US")]
    EnUs,
    #[serde(rename = "bg-BG")]
    BgBg,
    #[serde(rename = "tr-TR")]
    TrTr,
    #[serde(rename = "nl-NL")]
    NlNl,
    #[serde(rename = "fr-FR")]
    FrFr,
    #[serde(rename = "de-DE")]
    DeDe,
    #[serde(rename = "hi-IN")]
    HiIn,
    #[serde(rename = "ru-RU")]
    RuRu,
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "es-ES")]
    EsEs,
    #[serde(rename = "pt-BR")]
    PtBr,
    #[serde(rename = "ar-SA")]
    ArSa,
    #[serde(rename = "ja-JP")]
    JaJp,
    #[serde(rename = "ko-KR")]
    KoKr,
    #[serde(rename = "lo-LA")]
    LoLa,
    #[serde(rename = "cs-CZ")]
    CsCz,
    #[serde(rename = "it-IT")]
    ItIt,
    #[serde(rename = "fi-FI")]
    FiFi,
    #[serde(rename = "hu-HU")]
    HuHu,
    #[serde(rename = "pl-PL")]
    PlPl,
    #[serde(rename = "zh-TW")]
    ZhTw,
    #[serde(rename = "el-GR")]
    ElGr,
    #[serde(rename = "vi-VN")]
    ViVn,
}

impl Language {
    pub const ALL: [Language; 24] = [
        Language::System,
        Language::EnUs,
        Language::BgBg,
        Language::PtBr,
        Language::CsCz,
        Language::NlNl,
        Language::FrFr,
        Language::FiFi,
        Language::DeDe,
        Language::ElGr,
        Language::HuHu,
        Language::ItIt,
        Language::JaJp,
        Language::KoKr,
        Language::LoLa,
        Language::PlPl,
        Language::RuRu,
        Language::ZhCn,
        Language::EsEs,
        Language::ZhTw,
        Language::TrTr,
        Language::HiIn,
        Language::ArSa,
        Language::ViVn,
    ];

    fn requested(self) -> Vec<i18n_embed::unic_langid::LanguageIdentifier> {
        match self {
            Language::System => system_languages(),
            Language::EnUs => vec!["en-US".parse().expect("valid locale")],
            Language::BgBg => vec!["bg-BG".parse().expect("valid locale")],
            Language::TrTr => vec!["tr-TR".parse().expect("valid locale")],
            Language::NlNl => vec!["nl-NL".parse().expect("valid locale")],
            Language::FrFr => vec!["fr-FR".parse().expect("valid locale")],
            Language::DeDe => vec!["de-DE".parse().expect("valid locale")],
            Language::HiIn => vec!["hi-IN".parse().expect("valid locale")],
            Language::RuRu => vec!["ru-RU".parse().expect("valid locale")],
            Language::ZhCn => vec!["zh-CN".parse().expect("valid locale")],
            Language::EsEs => vec!["es-ES".parse().expect("valid locale")],
            Language::PtBr => vec!["pt-BR".parse().expect("valid locale")],
            Language::ArSa => vec!["ar-SA".parse().expect("valid locale")],
            Language::JaJp => vec!["ja-JP".parse().expect("valid locale")],
            Language::KoKr => vec!["ko-KR".parse().expect("valid locale")],
            Language::LoLa => vec!["lo-LA".parse().expect("valid locale")],
            Language::CsCz => vec!["cs-CZ".parse().expect("valid locale")],
            Language::ItIt => vec!["it-IT".parse().expect("valid locale")],
            Language::FiFi => vec!["fi-FI".parse().expect("valid locale")],
            Language::HuHu => vec!["hu-HU".parse().expect("valid locale")],
            Language::PlPl => vec!["pl-PL".parse().expect("valid locale")],
            Language::ZhTw => vec!["zh-TW".parse().expect("valid locale")],
            Language::ElGr => vec!["el-GR".parse().expect("valid locale")],
            Language::ViVn => vec!["vi-VN".parse().expect("valid locale")],
        }
    }

    pub fn label(self) -> String {
        match self {
            Language::System => crate::tr!("language", "system"),
            Language::EnUs => crate::tr!("language", "english"),
            Language::BgBg => crate::tr!("language", "bulgarian"),
            Language::TrTr => crate::tr!("language", "turkish"),
            Language::NlNl => crate::tr!("language", "dutch"),
            Language::FrFr => crate::tr!("language", "french"),
            Language::DeDe => crate::tr!("language", "german"),
            Language::HiIn => crate::tr!("language", "hindi"),
            Language::RuRu => crate::tr!("language", "russian"),
            Language::ZhCn => crate::tr!("language", "chinese-simplified"),
            Language::EsEs => crate::tr!("language", "spanish"),
            Language::PtBr => crate::tr!("language", "portuguese"),
            Language::ArSa => crate::tr!("language", "arabic"),
            Language::JaJp => crate::tr!("language", "japanese"),
            Language::KoKr => crate::tr!("language", "korean"),
            Language::LoLa => crate::tr!("language", "lao"),
            Language::CsCz => crate::tr!("language", "czech"),
            Language::ItIt => crate::tr!("language", "italian"),
            Language::FiFi => crate::tr!("language", "finnish"),
            Language::HuHu => crate::tr!("language", "hungarian"),
            Language::PlPl => crate::tr!("language", "polish"),
            Language::ZhTw => crate::tr!("language", "chinese-traditional"),
            Language::ElGr => crate::tr!("language", "greek"),
            Language::ViVn => crate::tr!("language", "vietnamese"),
        }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = self.label();
        f.write_str(&label)
    }
}

fn system_languages() -> Vec<i18n_embed::unic_langid::LanguageIdentifier> {
    // The unit tests compare English prompts and messages through one
    // process-wide loader, and every `apply_config` of a default config asks
    // for the system language, so the machine's locale must never reach
    // them — not even from a test that runs in parallel.
    #[cfg(test)]
    {
        vec!["en-US"
            .parse()
            .expect("en-US is a valid language identifier")]
    }

    #[cfg(not(test))]
    {
        #[cfg(target_arch = "wasm32")]
        let mut requested = WebLanguageRequester::requested_languages();
        #[cfg(not(target_arch = "wasm32"))]
        let requested = DesktopLanguageRequester::requested_languages();

        // `navigator.languages` may be empty in privacy-restricted browser
        // contexts. The singular preference is still exposed by mainstream
        // browsers, so keep it as the first fallback before English.
        #[cfg(target_arch = "wasm32")]
        if requested.is_empty() {
            if let Some(language) = web_sys::window()
                .and_then(|window| window.navigator().language())
                .and_then(|language| language.parse().ok())
            {
                requested.push(language);
            }
        }

        requested
    }
}

fn load_language(
    loader: &FluentLanguageLoader,
    language: Language,
) -> Result<(), i18n_embed::I18nEmbedError> {
    let mut requested = language.requested();
    if requested.is_empty() {
        requested.push(loader.fallback_language().clone());
    }
    #[allow(unused_variables)]
    let selected = i18n_embed::select(loader, &Localizations, &requested)?;
    #[cfg(target_arch = "wasm32")]
    if let Some(language) = selected.first() {
        if let Some(root) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.document_element())
        {
            let _ = root.set_attribute("lang", &language.to_string());
            let direction = if language.to_string().starts_with("ar") {
                "rtl"
            } else {
                "ltr"
            };
            let _ = root.set_attribute("dir", direction);
        }
    }
    Ok(())
}

pub fn loader() -> &'static FluentLanguageLoader {
    static LOADER: OnceLock<FluentLanguageLoader> = OnceLock::new();
    LOADER.get_or_init(|| {
        let loader = fluent_language_loader!();
        if let Err(error) = load_language(&loader, Language::System) {
            eprintln!("Unable to load system UI language: {error}");
            loader
                .load_languages(&Localizations, &[loader.fallback_language().clone()])
                .expect("fallback UI language must be embedded");
        }
        loader
    })
}

static CURRENT_LANGUAGE: std::sync::RwLock<Language> = std::sync::RwLock::new(Language::System);

pub fn active_language() -> Language {
    let lang = CURRENT_LANGUAGE
        .read()
        .map(|guard| *guard)
        .unwrap_or(Language::System);
    if lang == Language::System {
        let tag = active_language_tag();
        if tag.starts_with("ar") {
            Language::ArSa
        } else if tag.starts_with("fr") {
            Language::FrFr
        } else {
            Language::EnUs
        }
    } else {
        lang
    }
}

pub fn lang_str<'a>(en: &'a str, ar: &'a str, fr: &'a str) -> &'a str {
    match active_language() {
        Language::ArSa => ar,
        Language::FrFr => fr,
        _ => en,
    }
}

/// Apply a user preference process-wide. The loader swaps resources atomically,
/// so the next Iced view pass immediately receives the new language.
pub fn set_language(language: Language) -> Result<(), i18n_embed::I18nEmbedError> {
    if let Ok(mut lock) = CURRENT_LANGUAGE.write() {
        *lock = language;
    }
    load_language(loader(), language)
}

pub fn active_language_tag() -> String {
    loader()
        .current_languages()
        .into_iter()
        .next()
        .unwrap_or_else(|| loader().fallback_language().clone())
        .to_string()
}

pub fn display_parcel_term(term: &str) -> String {
    let lang = active_language();
    match term {
        // Remainder
        "Place remainder in last parcel" => match lang {
            Language::ArSa => "وضع المتبقي في آخر قطعة".to_string(),
            Language::FrFr => "Placer le reliquat dans la dernière".to_string(),
            _ => term.to_string(),
        },
        "Redistribute remainder" => match lang {
            Language::ArSa => "إعادة توزيع المتبقي".to_string(),
            Language::FrFr => "Redistribuer le reliquat".to_string(),
            _ => term.to_string(),
        },
        "Create remainder parcel" => match lang {
            Language::ArSa => "إنشاء قطعة مستقلة للمتبقي".to_string(),
            Language::FrFr => "Créer une parcelle de reliquat".to_string(),
            _ => term.to_string(),
        },
        // Solution Preferences
        "Use shortest frontage" => match lang {
            Language::ArSa => "استخدام أقصر واجهة".to_string(),
            Language::FrFr => "Utiliser la plus petite façade".to_string(),
            _ => term.to_string(),
        },
        "Use smallest area" => match lang {
            Language::ArSa => "استخدام أصغر مساحة".to_string(),
            Language::FrFr => "Utiliser la plus petite surface".to_string(),
            _ => term.to_string(),
        },
        "Use largest frontage" => match lang {
            Language::ArSa => "استخدام أكبر واجهة".to_string(),
            Language::FrFr => "Utiliser la plus grande façade".to_string(),
            _ => term.to_string(),
        },
        // Parcel Styles
        "Single-Family" => match lang {
            Language::ArSa => "سكني عائلي (Single-Family)".to_string(),
            Language::FrFr => "Résidentiel unifamilial".to_string(),
            _ => term.to_string(),
        },
        "Commercial" => match lang {
            Language::ArSa => "تجاري (Commercial)".to_string(),
            Language::FrFr => "Commercial".to_string(),
            _ => term.to_string(),
        },
        "Civic" => match lang {
            Language::ArSa => "خدمي / عام (Civic)".to_string(),
            Language::FrFr => "Public / Civique".to_string(),
            _ => term.to_string(),
        },
        "Park / Open Space" => match lang {
            Language::ArSa => "حدائق ومساحات مفتوحة".to_string(),
            Language::FrFr => "Parc / Espace vert".to_string(),
            _ => term.to_string(),
        },
        "Standard" => match lang {
            Language::ArSa => "قياسي (Standard)".to_string(),
            Language::FrFr => "Standard".to_string(),
            _ => term.to_string(),
        },
        "Property" => match lang {
            Language::ArSa => "ملكية عقارية (Property)".to_string(),
            Language::FrFr => "Propriété".to_string(),
            _ => term.to_string(),
        },
        "Multi-Family Residential" => match lang {
            Language::ArSa => "سكني متعدد الأسر".to_string(),
            Language::FrFr => "Résidentiel multifamilial".to_string(),
            _ => term.to_string(),
        },
        "Industrial / Logistics" => match lang {
            Language::ArSa => "صناعي / لوجستي".to_string(),
            Language::FrFr => "Industriel / Logistique".to_string(),
            _ => term.to_string(),
        },
        // Area Labels
        "Parcel Name, Area & Perimeter" => match lang {
            Language::ArSa => "الاسم والمساحة والمحيط".to_string(),
            Language::FrFr => "Nom, surface & périmètre".to_string(),
            _ => term.to_string(),
        },
        "Parcel Name & Area" => match lang {
            Language::ArSa => "الاسم والمساحة".to_string(),
            Language::FrFr => "Nom & surface".to_string(),
            _ => term.to_string(),
        },
        "Name Only" => match lang {
            Language::ArSa => "الاسم فقط".to_string(),
            Language::FrFr => "Nom seul".to_string(),
            _ => term.to_string(),
        },
        "Area Only" => match lang {
            Language::ArSa => "المساحة فقط".to_string(),
            Language::FrFr => "Surface seule".to_string(),
            _ => term.to_string(),
        },
        "Square Meters & Acres" => match lang {
            Language::ArSa => "أمتار مربعة وفدان".to_string(),
            Language::FrFr => "Mètres carrés & Acres".to_string(),
            _ => term.to_string(),
        },
        // Line Labels
        "Bearing over Distance" => match lang {
            Language::ArSa => "الاتجاه والمسافة".to_string(),
            Language::FrFr => "Gisement et distance".to_string(),
            _ => term.to_string(),
        },
        "Distance Only" => match lang {
            Language::ArSa => "المسافة فقط".to_string(),
            Language::FrFr => "Distance seule".to_string(),
            _ => term.to_string(),
        },
        "Bearing Only" => match lang {
            Language::ArSa => "الاتجاه فقط".to_string(),
            Language::FrFr => "Gisement seul".to_string(),
            _ => term.to_string(),
        },
        // Curve Labels
        "Distance over Radius" => match lang {
            Language::ArSa => "المسافة ونصف القطر".to_string(),
            Language::FrFr => "Longueur et rayon".to_string(),
            _ => term.to_string(),
        },
        "Delta over Length" => match lang {
            Language::ArSa => "الزاوية والطول".to_string(),
            Language::FrFr => "Delta et longueur".to_string(),
            _ => term.to_string(),
        },
        "Radius over Length" => match lang {
            Language::ArSa => "نصف القطر والطول".to_string(),
            Language::FrFr => "Rayon et longueur".to_string(),
            _ => term.to_string(),
        },
        "Length Only" => match lang {
            Language::ArSa => "الطول فقط".to_string(),
            Language::FrFr => "Longueur seule".to_string(),
            _ => term.to_string(),
        },
        "Radius Only" => match lang {
            Language::ArSa => "نصف القطر فقط".to_string(),
            Language::FrFr => "Rayon seul".to_string(),
            _ => term.to_string(),
        },
        "None" => match lang {
            Language::ArSa => "بدون".to_string(),
            Language::FrFr => "Aucun".to_string(),
            _ => term.to_string(),
        },
        // Zoning
        "Single-Family Residential (R-1)" => match lang {
            Language::ArSa => "سكني فردي (R-1)".to_string(),
            Language::FrFr => "Résidentiel unifamilial (R-1)".to_string(),
            _ => term.to_string(),
        },
        "Two-Family Residential (R-2)" => match lang {
            Language::ArSa => "سكني مزدوج (R-2)".to_string(),
            Language::FrFr => "Résidentiel bifamilial (R-2)".to_string(),
            _ => term.to_string(),
        },
        "Multi-Family Residential (R-3)" => match lang {
            Language::ArSa => "سكني متعدد (R-3)".to_string(),
            Language::FrFr => "Résidentiel multifamilial (R-3)".to_string(),
            _ => term.to_string(),
        },
        "Commercial / Retail (C-1)" => match lang {
            Language::ArSa => "تجاري / تجزئة (C-1)".to_string(),
            Language::FrFr => "Commercial / Vente (C-1)".to_string(),
            _ => term.to_string(),
        },
        "Mixed-Use Town Center (MU-1)" => match lang {
            Language::ArSa => "استخدام مختلط (MU-1)".to_string(),
            Language::FrFr => "Usage mixte (MU-1)".to_string(),
            _ => term.to_string(),
        },
        "Civic / Institutional (PUB)" => match lang {
            Language::ArSa => "مباني عامة ومرافق (PUB)".to_string(),
            Language::FrFr => "Édifices publics (PUB)".to_string(),
            _ => term.to_string(),
        },
        "Parks & Open Space (REC)" => match lang {
            Language::ArSa => "حدائق ومنتزهات (REC)".to_string(),
            Language::FrFr => "Parcs & Loisirs (REC)".to_string(),
            _ => term.to_string(),
        },
        other => other.to_string(),
    }
}

fn translate_parcels_term(source: &str) -> Option<String> {
    let candidate = display_parcel_term(source);
    if candidate != source {
        return Some(candidate);
    }
    let lang = active_language();
    match source {
        "Parcels" => Some(match lang {
            Language::ArSa => "قطع الأراضي".to_string(),
            Language::FrFr => "Parcelles".to_string(),
            _ => "Parcels".to_string(),
        }),
        "Parcel Layout" => Some(match lang {
            Language::ArSa => "تخطيط الأراضي".to_string(),
            Language::FrFr => "Disposition des parcelles".to_string(),
            _ => "Parcel Layout".to_string(),
        }),
        "Layout Tools" => Some(match lang {
            Language::ArSa => "أدوات التخطيط".to_string(),
            Language::FrFr => "Outils de disposition".to_string(),
            _ => "Layout Tools".to_string(),
        }),
        "Right-of-Way" => Some(match lang {
            Language::ArSa => "حرم الطريق (ROW)".to_string(),
            Language::FrFr => "Emprise (ROW)".to_string(),
            _ => "Right-of-Way".to_string(),
        }),
        "Create ROW" => Some(match lang {
            Language::ArSa => "إنشاء حرم الطريق".to_string(),
            Language::FrFr => "Créer emprise".to_string(),
            _ => "Create ROW".to_string(),
        }),
        "Tables" => Some(match lang {
            Language::ArSa => "الجداول".to_string(),
            Language::FrFr => "Tables".to_string(),
            _ => "Tables".to_string(),
        }),
        "Area Table" => Some(match lang {
            Language::ArSa => "جدول المساحات".to_string(),
            Language::FrFr => "Table des surfaces".to_string(),
            _ => "Area Table".to_string(),
        }),
        "Line Table" => Some(match lang {
            Language::ArSa => "جدول الخطوط".to_string(),
            Language::FrFr => "Table des lignes".to_string(),
            _ => "Line Table".to_string(),
        }),
        "Curve Table" => Some(match lang {
            Language::ArSa => "جدول المنحنيات".to_string(),
            Language::FrFr => "Table des courbes".to_string(),
            _ => "Curve Table".to_string(),
        }),
        "Styles & Envelopes" => Some(match lang {
            Language::ArSa => "الأنماط والارتدادات".to_string(),
            Language::FrFr => "Styles & Enveloppes".to_string(),
            _ => "Styles & Envelopes".to_string(),
        }),
        "Thematic Zoning" => Some(match lang {
            Language::ArSa => "التخصيص اللوني".to_string(),
            Language::FrFr => "Zonage thématique".to_string(),
            _ => "Thematic Zoning".to_string(),
        }),
        "Setbacks" => Some(match lang {
            Language::ArSa => "الارتدادات".to_string(),
            Language::FrFr => "Marges de recul".to_string(),
            _ => "Setbacks".to_string(),
        }),
        "Analysis & Legal" => Some(match lang {
            Language::ArSa => "التحليل والتوثيق".to_string(),
            Language::FrFr => "Analyse & Juridique".to_string(),
            _ => "Analysis & Legal".to_string(),
        }),
        "Mapcheck" => Some(match lang {
            Language::ArSa => "فحص الإغلاق".to_string(),
            Language::FrFr => "Contrôle de fermeture".to_string(),
            _ => "Mapcheck".to_string(),
        }),
        "Balance Traverse" => Some(match lang {
            Language::ArSa => "موازنة المضلع".to_string(),
            Language::FrFr => "Compenser cheminement".to_string(),
            _ => "Balance Traverse".to_string(),
        }),
        "Legal Deed" => Some(match lang {
            Language::ArSa => "الصك القانوني".to_string(),
            Language::FrFr => "Acte juridique".to_string(),
            _ => "Legal Deed".to_string(),
        }),
        "Renumber" => Some(match lang {
            Language::ArSa => "إعادة الترقيم".to_string(),
            Language::FrFr => "Renuméroter".to_string(),
            _ => "Renumber".to_string(),
        }),
        "Report" => Some(match lang {
            Language::ArSa => "تقرير التقسيم".to_string(),
            Language::FrFr => "Rapport".to_string(),
            _ => "Report".to_string(),
        }),
        "GIS & Interop" => Some(match lang {
            Language::ArSa => "نظم المعلومات والتبادل".to_string(),
            Language::FrFr => "SIG & Interopérabilité".to_string(),
            _ => "GIS & Interop".to_string(),
        }),
        "GeoJSON In" => Some(match lang {
            Language::ArSa => "استيراد GeoJSON".to_string(),
            Language::FrFr => "Importer GeoJSON".to_string(),
            _ => "GeoJSON In".to_string(),
        }),
        "GeoJSON Out" => Some(match lang {
            Language::ArSa => "تصدير GeoJSON".to_string(),
            Language::FrFr => "Exporter GeoJSON".to_string(),
            _ => "GeoJSON Out".to_string(),
        }),
        "LandXML In" => Some(match lang {
            Language::ArSa => "استيراد LandXML".to_string(),
            Language::FrFr => "Importer LandXML".to_string(),
            _ => "LandXML In".to_string(),
        }),
        "Export CSV" => Some(match lang {
            Language::ArSa => "تصدير CSV".to_string(),
            Language::FrFr => "Exporter CSV".to_string(),
            _ => "Export CSV".to_string(),
        }),
        "Import Parcels from GeoJSON" => Some(match lang {
            Language::ArSa => "استيراد قطع الأراضي من ملف GeoJSON".to_string(),
            Language::FrFr => "Importer des parcelles depuis GeoJSON".to_string(),
            _ => "Import Parcels from GeoJSON".to_string(),
        }),
        "GeoJSON file" => Some(match lang {
            Language::ArSa => "ملف GeoJSON".to_string(),
            Language::FrFr => "Fichier GeoJSON".to_string(),
            _ => "GeoJSON file".to_string(),
        }),
        "Import Parcels from LandXML" => Some(match lang {
            Language::ArSa => "استيراد قطع الأراضي من ملف LandXML".to_string(),
            Language::FrFr => "Importer des parcelles depuis LandXML".to_string(),
            _ => "Import Parcels from LandXML".to_string(),
        }),
        "LandXML file" => Some(match lang {
            Language::ArSa => "ملف LandXML".to_string(),
            Language::FrFr => "Fichier LandXML".to_string(),
            _ => "LandXML file".to_string(),
        }),
        "Parcel Layout Tools" => Some(match lang {
            Language::ArSa => "أدوات تخطيط قطع الأراضي".to_string(),
            Language::FrFr => "Outils de disposition des parcelles".to_string(),
            _ => "Parcel Layout Tools".to_string(),
        }),
        "Create Parcels - Layout" => Some(match lang {
            Language::ArSa => "إنشاء قطع الأراضي - التخطيط".to_string(),
            Language::FrFr => "Créer des parcelles - Disposition".to_string(),
            _ => "Create Parcels - Layout".to_string(),
        }),
        "Parcel Properties" => Some(match lang {
            Language::ArSa => "خصائص قطعة الأرض".to_string(),
            Language::FrFr => "Propriétés de la parcelle".to_string(),
            _ => "Parcel Properties".to_string(),
        }),
        "Civil 3D Renumber: Click parcels or press Enter for fence mode." => Some(match lang {
            Language::ArSa => "إعادة الترقيم: انقر على قطع الأراضي أو اضغط Enter لوضع السياج.".to_string(),
            Language::FrFr => "Renumérotation : Cliquez sur les parcelles ou appuyez sur Entrée pour le mode clôture.".to_string(),
            _ => "Civil 3D Renumber: Click parcels or press Enter for fence mode.".to_string(),
        }),
        _ => None,
    }
}

pub fn translate_open_infra_term(source: &str) -> Option<String> {
    let lang = active_language();
    match source {
        // Tab & Plugin Names
        "Open Infra — GIS" | "Open Infra - GIS" | "Open Infra" => Some(match lang {
            Language::ArSa => "البنية التحتية — نظم المعلومات الجغرافية".to_string(),
            Language::FrFr => "Open Infra — SIG".to_string(),
            _ => "Open Infra — GIS".to_string(),
        }),
        "GIS-workbench en NLCS-basisdata voor infra en civiele techniek: projectlocatie, WMS/WFS-servers, OpenStreetMap, BGT, kadaster en KLIC conform NLCS. Nederland (PDOK) en wereldwijd."
        | "GIS workbench and NLCS base data for infrastructure and civil engineering: project location, WMS/WFS servers, OpenStreetMap, BGT, cadastre, and utilities." => Some(match lang {
            Language::ArSa => "منصة نظم المعلومات الجغرافية (GIS) والبيانات الأساسية للبنية التحتية: تحديد موقع المشروع، خوادم WMS/WFS، خرائط OpenStreetMap، طبقات BGT، السجل العقاري، وشبكات المرافق.".to_string(),
            Language::FrFr => "Atelier SIG et données de base pour l'infrastructure : géolocalisation de projet, serveurs WMS/WFS, OpenStreetMap, BGT, cadastre et réseaux souterrains.".to_string(),
            _ => "GIS workbench and NLCS base data for infrastructure and civil engineering: project location, WMS/WFS servers, OpenStreetMap, BGT, cadastre, and utilities.".to_string(),
        }),

        // Ribbon Groups
        "Geo-locatie" | "Geo Location" => Some(match lang {
            Language::ArSa => "الموقع الجغرافي".to_string(),
            Language::FrFr => "Géo-localisation".to_string(),
            _ => "Geo Location".to_string(),
        }),
        "Kaartbeeld" | "Map View" => Some(match lang {
            Language::ArSa => "صورة الخريطة".to_string(),
            Language::FrFr => "Image cartographique".to_string(),
            _ => "Map View".to_string(),
        }),
        "Vectordata" | "Vector Data" => Some(match lang {
            Language::ArSa => "البيانات المتجهة".to_string(),
            Language::FrFr => "Données vectorielles".to_string(),
            _ => "Vector Data".to_string(),
        }),
        "Tekenblad" | "Drawing Sheet" => Some(match lang {
            Language::ArSa => "لوحة الرسم".to_string(),
            Language::FrFr => "Feuille de dessin".to_string(),
            _ => "Drawing Sheet".to_string(),
        }),
        "Gebied" | "Region / Area" | "Area" => Some(match lang {
            Language::ArSa => "نطاق المنطقة".to_string(),
            Language::FrFr => "Zone / Région".to_string(),
            _ => "Region / Area".to_string(),
        }),
        "Punt info" | "Point Info" | "Point info" => Some(match lang {
            Language::ArSa => "معلومات النقطة".to_string(),
            Language::FrFr => "Infos point".to_string(),
            _ => "Point Info".to_string(),
        }),
        "Coordinaten" | "Coordinates" => Some(match lang {
            Language::ArSa => "نظام الإحداثيات".to_string(),
            Language::FrFr => "Coordonnées".to_string(),
            _ => "Coordinates".to_string(),
        }),
        "Servers" | "Map Servers" => Some(match lang {
            Language::ArSa => "خوادم الخرائط".to_string(),
            Language::FrFr => "Serveurs de cartes".to_string(),
            _ => "Map Servers".to_string(),
        }),

        // Tools & Dropdowns
        "Locatie" | "Location" => Some(match lang {
            Language::ArSa => "تحديد الموقع".to_string(),
            Language::FrFr => "Emplacement".to_string(),
            _ => "Location".to_string(),
        }),
        "WMS / TMS" | "WMS/TMS" => Some(match lang {
            Language::ArSa => "خرائط WMS / TMS".to_string(),
            Language::FrFr => "Cartes WMS / TMS".to_string(),
            _ => "WMS / TMS".to_string(),
        }),
        "OpenStreetMap" | "OSM" => Some(match lang {
            Language::ArSa => "خرائط الشوارع (OSM)".to_string(),
            Language::FrFr => "OpenStreetMap (OSM)".to_string(),
            _ => "OpenStreetMap".to_string(),
        }),
        "PDOK (NL)" | "PDOK" => Some(match lang {
            Language::ArSa => "خوادم PDOK (هولندا)".to_string(),
            Language::FrFr => "Serveurs PDOK (NL)".to_string(),
            _ => "PDOK (NL)".to_string(),
        }),
        "Luchtfoto" | "Aerial Photo" => Some(match lang {
            Language::ArSa => "صورة جوية".to_string(),
            Language::FrFr => "Photo aérienne".to_string(),
            _ => "Aerial Photo".to_string(),
        }),
        "Satelliet" | "Satellite" => Some(match lang {
            Language::ArSa => "قمر صناعي".to_string(),
            Language::FrFr => "Satellite".to_string(),
            _ => "Satellite".to_string(),
        }),
        "AHN" | "Elevation Model (AHN)" => Some(match lang {
            Language::ArSa => "نموذج الارتفاعات (AHN)".to_string(),
            Language::FrFr => "Modèle d'élévation (AHN)".to_string(),
            _ => "Elevation Model (AHN)".to_string(),
        }),
        "WFS" | "WFS Features" => Some(match lang {
            Language::ArSa => "معالم متجهة (WFS)".to_string(),
            Language::FrFr => "Service d'entités WFS".to_string(),
            _ => "WFS Features".to_string(),
        }),
        "BGT" | "BGT Topography" => Some(match lang {
            Language::ArSa => "طبوغرافيا BGT".to_string(),
            Language::FrFr => "Topographie BGT".to_string(),
            _ => "BGT Topography".to_string(),
        }),
        "Kadaster" | "Cadastre" => Some(match lang {
            Language::ArSa => "السجل العقاري".to_string(),
            Language::FrFr => "Cadastre".to_string(),
            _ => "Cadastre".to_string(),
        }),
        "Percelen" => Some(match lang {
            Language::ArSa => "القطع العقارية".to_string(),
            Language::FrFr => "Parcelles".to_string(),
            _ => "Parcels".to_string(),
        }),
        "BAG" | "Buildings & Addresses" => Some(match lang {
            Language::ArSa => "سجل المباني والعناوين (BAG)".to_string(),
            Language::FrFr => "Bâtiments & Adresses (BAG)".to_string(),
            _ => "Buildings & Addresses (BAG)".to_string(),
        }),
        "NWB" | "Road Network" => Some(match lang {
            Language::ArSa => "شبكة الطرق الوطنية (NWB)".to_string(),
            Language::FrFr => "Réseau routier (NWB)".to_string(),
            _ => "Road Network (NWB)".to_string(),
        }),
        "Hectometer" | "Hectometer Points" => Some(match lang {
            Language::ArSa => "نقاط الهكتومتر".to_string(),
            Language::FrFr => "Bornes hectométriques".to_string(),
            _ => "Hectometer Points".to_string(),
        }),
        "Spoor" | "Railway Tracks" => Some(match lang {
            Language::ArSa => "خطوط السكك الحديدية".to_string(),
            Language::FrFr => "Voies ferrées".to_string(),
            _ => "Railway Tracks".to_string(),
        }),
        "KLIC" | "Utilities & Cables" => Some(match lang {
            Language::ArSa => "شبكات المرافق والكابلات (KLIC)".to_string(),
            Language::FrFr => "Réseaux et câbles (KLIC)".to_string(),
            _ => "Utilities & Cables (KLIC)".to_string(),
        }),
        "KLIC Import" => Some(match lang {
            Language::ArSa => "استيراد شبكات KLIC".to_string(),
            Language::FrFr => "Importer réseaux KLIC".to_string(),
            _ => "KLIC Import".to_string(),
        }),
        "BRK" | "BRK Cadastre" => Some(match lang {
            Language::ArSa => "السجل العقاري الهولندي (BRK)".to_string(),
            Language::FrFr => "Cadastre BRK".to_string(),
            _ => "BRK Cadastre".to_string(),
        }),
        "Site" | "Project Site" => Some(match lang {
            Language::ArSa => "موقع المشروع".to_string(),
            Language::FrFr => "Site du projet".to_string(),
            _ => "Project Site".to_string(),
        }),

        // Tooltips & Descriptions
        "Projectlocatie instellen" => Some(match lang {
            Language::ArSa => "تحديد موقع المشروع وإحداثيات الموقع الجغرافي".to_string(),
            Language::FrFr => "Définir l'emplacement et les coordonnées du projet".to_string(),
            _ => "Set project location and geographic coordinates".to_string(),
        }),
        "WMS/TMS achtergrondkaarten laden" => Some(match lang {
            Language::ArSa => "تحميل خرائط الأساس WMS/TMS كخلفية للرسم".to_string(),
            Language::FrFr => "Charger les cartes d'arrière-plan WMS/TMS".to_string(),
            _ => "Load WMS/TMS background maps".to_string(),
        }),
        "OpenStreetMap vectoren en tegels importeren" => Some(match lang {
            Language::ArSa => "استيراد متجهات وبلاطات خريطة OpenStreetMap".to_string(),
            Language::FrFr => "Importer les vecteurs et tuiles OpenStreetMap".to_string(),
            _ => "Import OpenStreetMap vectors and tiles".to_string(),
        }),
        "WFS vectorlagen ophalen" => Some(match lang {
            Language::ArSa => "جلب طبقات المعالم المتجهة عبر خدمة WFS".to_string(),
            Language::FrFr => "Récupérer les couches vectorielles WFS".to_string(),
            _ => "Fetch WFS vector layers".to_string(),
        }),
        "BGT grootschalige basiskaart" => Some(match lang {
            Language::ArSa => "خريطة الأساس الطبوغرافية التفصيلية BGT".to_string(),
            Language::FrFr => "Carte de base topographique détaillée BGT".to_string(),
            _ => "BGT detailed base map".to_string(),
        }),
        "Kadastrale percelen en grenzen" => Some(match lang {
            Language::ArSa => "القطع والحدود العقارية الرسمية".to_string(),
            Language::FrFr => "Parcelles et limites cadastrales officielles".to_string(),
            _ => "Cadastral parcels and official boundaries".to_string(),
        }),
        "Kabels en leidingen conform KLIC" => Some(match lang {
            Language::ArSa => "كابلات وشبكات الأنابيب والمرافق العامة".to_string(),
            Language::FrFr => "Câbles et canalisations souterraines".to_string(),
            _ => "Underground utility cables and pipes".to_string(),
        }),
        "Tekenblad kaders en noordpijl" => Some(match lang {
            Language::ArSa => "إطارات لوحة الرسم وسهم الشمال ومقياس الرسم".to_string(),
            Language::FrFr => "Cadres de dessin, flèche nord et échelle".to_string(),
            _ => "Drawing sheet frames, north arrow and scale".to_string(),
        }),
        "Gebied selecteren en downloaden" => Some(match lang {
            Language::ArSa => "تحديد نطاق المنطقة الجغرافية وتنزيل البيانات".to_string(),
            Language::FrFr => "Sélectionner la zone et télécharger les données".to_string(),
            _ => "Select geographic area and download data".to_string(),
        }),
        "Informatie over geselecteerd punt" => Some(match lang {
            Language::ArSa => "معلومات وتفاصيل النقطة الجغرافية المحددة".to_string(),
            Language::FrFr => "Informations sur le point sélectionné".to_string(),
            _ => "Information about selected geographic point".to_string(),
        }),
        "Coordinatenstelsel configureren" => Some(match lang {
            Language::ArSa => "ضبط وتكوين نظام الإحداثيات والمرجع الجغرافي (CRS)".to_string(),
            Language::FrFr => "Configurer le système de référence de coordonnées (CRS)".to_string(),
            _ => "Configure coordinate reference system (CRS)".to_string(),
        }),
        "WMS/WFS serverlijst beheren" => Some(match lang {
            Language::ArSa => "إدارة قائمة خوادم الخرائط WMS/WFS".to_string(),
            Language::FrFr => "Gérer la liste des serveurs cartographiques WMS/WFS".to_string(),
            _ => "Manage WMS/WFS map servers list".to_string(),
        }),

        _ => None,
    }
}

/// Catalog ids of a source string, memoized per thread. The catalog is one
/// string `match` over thousands of arms, a linear scan per call; labels are
/// looked up on every view pass and per selected object in the Properties
/// panel, so the scan dominated those paths (#1999).
fn catalog_ids(source: &str) -> Option<(&'static str, &'static str)> {
    type Ids = rustc_hash::FxHashMap<Box<str>, Option<(&'static str, &'static str)>>;
    thread_local! {
        static IDS: std::cell::RefCell<Ids> = std::cell::RefCell::default();
    }
    IDS.with(|ids| {
        if let Some(found) = ids.borrow().get(source) {
            return *found;
        }
        let found = locale_catalog::message_attribute(source);
        let mut ids = ids.borrow_mut();
        // Runtime text (names, values) can reach `t!` too; stay bounded.
        if ids.len() >= 16_384 {
            ids.clear();
        }
        ids.insert(source.into(), found);
        found
    })
}

/// Translate an application-facing source label from the complete UI catalog.
///
/// Stable semantic ids remain preferable for new code. This compatibility
/// layer lets existing UI surfaces move to Fluent without turning command
/// tokens, property ids, file-format values, or plug-in supplied text into
/// translatable data.
pub fn translate(source: impl AsRef<str>) -> Cow<'static, str> {
    let source = source.as_ref();
    if let Some(parcels_term) = translate_parcels_term(source) {
        return Cow::Owned(parcels_term);
    }
    if let Some(infra_term) = translate_open_infra_term(source) {
        return Cow::Owned(infra_term);
    }
    catalog_ids(source)
        .map(|(message_id, attribute_id)| Cow::Owned(loader().get_attr(message_id, attribute_id)))
        .unwrap_or_else(|| Cow::Owned(source.to_string()))
}

/// Translate a catalog message and replace the named values used by legacy
/// command prompts. Markers preserve values through Fluent parsing.
pub fn translate_args(source: impl AsRef<str>, args: &[(&str, String)]) -> Cow<'static, str> {
    let source = source.as_ref();
    let mut translated = catalog_ids(source)
        .map(|(message_id, attribute_id)| loader().get_attr(message_id, attribute_id))
        .unwrap_or_else(|| source.to_string());
    for (name, value) in args {
        translated = translated.replace(&format!("__ocs_arg_{name}__"), value);
        translated = translated.replace(&format!("%{{{name}}}"), value);
    }
    Cow::Owned(translated)
}

/// Translate a Rust formatting template after its values have been rendered.
/// The catalog stores positional markers, while this function recovers each
/// rendered value from the English template and places it in the localized
/// sentence. File names, handles, counts, and command values therefore remain
/// data instead of being sent through translation.
pub fn translate_format(template: &str, rendered: String) -> Cow<'static, str> {
    let Some((message_id, attribute_id)) = catalog_ids(template) else {
        return Cow::Owned(rendered);
    };
    let Some(values) = format_values(template, &rendered) else {
        return Cow::Owned(rendered);
    };
    let mut translated = loader().get_attr(message_id, attribute_id);
    for (index, value) in values.iter().enumerate() {
        translated = translated.replace(&format!("__ocs_fmt_{index}__"), value);
    }
    Cow::Owned(translated)
}

fn format_values(template: &str, rendered: &str) -> Option<Vec<String>> {
    let mut literals = vec![String::new()];
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        match (ch, chars.peek().copied()) {
            ('{', Some('{')) => {
                chars.next();
                literals.last_mut()?.push('{');
            }
            ('}', Some('}')) => {
                chars.next();
                literals.last_mut()?.push('}');
            }
            ('{', _) => {
                let mut closed = false;
                for inner in chars.by_ref() {
                    if inner == '}' {
                        closed = true;
                        break;
                    }
                }
                if !closed {
                    return None;
                }
                literals.push(String::new());
            }
            ('}', _) => return None,
            _ => literals.last_mut()?.push(ch),
        }
    }

    let mut cursor = 0;
    if !rendered.starts_with(&literals[0]) {
        return None;
    }
    cursor += literals[0].len();
    let mut values = Vec::with_capacity(literals.len().saturating_sub(1));
    for index in 1..literals.len() {
        let separator = &literals[index];
        if index + 1 == literals.len() && separator.is_empty() {
            values.push(rendered[cursor..].to_string());
            cursor = rendered.len();
        } else {
            let offset = rendered[cursor..].find(separator)?;
            values.push(rendered[cursor..cursor + offset].to_string());
            cursor += offset + separator.len();
        }
    }
    (cursor == rendered.len()).then_some(values)
}

/// Built-in ribbon modules have stable ids; plug-ins keep their supplied title
/// until they provide their own localization bundle.
pub fn ribbon_module_title(id: &str, fallback: &str) -> String {
    match id {
        "draw" => crate::tr!("ribbon-tab", "draw"),
        "parametric" => crate::tr!("ribbon-tab", "parametric"),
        "annotate" => crate::tr!("ribbon-tab", "annotate"),
        "insert" => crate::tr!("ribbon-tab", "insert"),
        "model" => crate::tr!("ribbon-tab", "model"),
        "layout" => crate::tr!("ribbon-tab", "layout"),
        "manage" => crate::tr!("ribbon-tab", "manage"),
        "view" => crate::tr!("ribbon-tab", "view"),
        "block_editor" => crate::tr!("ribbon-tab", "block-editor"),
        "parcels" => match active_language() {
            Language::ArSa => "قطع الأراضي".to_string(),
            Language::FrFr => "Parcelles".to_string(),
            _ => "Parcels".to_string(),
        },
        "openaec.open-infra" | "open-infra" => match active_language() {
            Language::ArSa => "البنية التحتية — نظم المعلومات الجغرافية".to_string(),
            Language::FrFr => "Open Infra — SIG".to_string(),
            _ => "Open Infra — GIS".to_string(),
        },
        _ => {
            if let Some(term) = translate_open_infra_term(fallback) {
                term
            } else {
                fallback.to_string()
            }
        }
    }
}

#[macro_export]
macro_rules! tr {
    ($message_id:literal, $attribute_id:literal $(,)?) => {
        i18n_embed_fl::fl!($crate::i18n::loader(), $message_id, $attribute_id)
    };
    ($message_id:literal, $attribute_id:literal, $($name:ident = $value:expr),+ $(,)?) => {
        i18n_embed_fl::fl!(
            $crate::i18n::loader(),
            $message_id,
            $attribute_id,
            $($name = $value),+
        )
    };
    ($message_id:literal $(,)?) => {
        i18n_embed_fl::fl!($crate::i18n::loader(), $message_id)
    };
    ($message_id:literal, $($name:ident = $value:expr),+ $(,)?) => {
        i18n_embed_fl::fl!(
            $crate::i18n::loader(),
            $message_id,
            $($name = $value),+
        )
    };
}

/// Source-catalog compatibility macro used while the existing interface is
/// migrated to semantic Fluent ids. Named arguments mirror the command prompt
/// placeholders already present in the source catalog.
#[macro_export]
macro_rules! t {
    ($source:expr $(,)?) => {
        $crate::i18n::translate($source)
    };
    ($source:expr, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::i18n::translate_args(
            $source,
            &[$((stringify!($name), ($value).to_string())),+],
        )
    };
}

/// Localized counterpart of `format!` for application-facing messages.
#[macro_export]
macro_rules! tf {
    ($template:literal $($args:tt)*) => {{
        let rendered = format!($template $($args)*);
        $crate::i18n::translate_format($template, rendered)
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn block_grip_messages_render_in_every_locale_without_translating_authored_data() {
        let expected = BTreeSet::from(
            [
                "visibility-label",
                "visibility-value",
                "table-label",
                "table-row",
                "alignment",
                "flipped",
                "not-flipped",
            ]
            .map(String::from),
        );
        for language in Language::ALL
            .into_iter()
            .filter(|language| *language != Language::System)
        {
            let loader = FluentLanguageLoader::new("opencadstudio", "en-US".parse().unwrap());
            load_language(&loader, language).expect("Fluent resources must parse");
            let actual: BTreeSet<_> =
                loader.with_message_iter(&language.requested()[0], |messages| {
                    messages
                        .filter(|message| message.id.name == "block-grip")
                        .flat_map(|message| {
                            message
                                .attributes
                                .iter()
                                .map(|attribute| attribute.id.name.to_owned())
                        })
                        .collect()
                });
            assert_eq!(
                actual, expected,
                "{language:?}: messages must exist in this locale, not fall back"
            );
            let authored_state = "  Façade — مسافة — 距離 { $state } \\ literal ";
            let mode = loader.get_attr("common", "tangent");
            let args = [
                ("state", authored_state),
                ("row", "17"),
                ("mode", mode.as_str()),
            ]
            .into_iter()
            .collect();
            let render = |attribute: &str| {
                loader
                    .with_fluent_message_and_bundle("block-grip", |message, bundle| {
                        let pattern = message.get_attribute(attribute).unwrap();
                        let mut errors = Vec::new();
                        let text = bundle
                            .format_pattern(pattern.value(), Some(&args), &mut errors)
                            .into_owned();
                        assert!(errors.is_empty(), "{language:?}/{attribute}: {errors:?}");
                        text
                    })
                    .unwrap()
            };
            for attribute in &expected {
                assert!(!render(attribute).is_empty());
            }
            for attribute in ["base-state-label", "flipped-state-label"] {
                assert!(!loader.get_attr("block-flip", attribute).is_empty());
            }
            assert!(render("visibility-value").contains(authored_state));
            assert!(render("table-row").contains("17"));
            assert!(render("alignment").contains(&mode));
            if language != Language::EnUs {
                assert_ne!(render("visibility-label"), "Visibility state");
                assert_ne!(render("flipped"), "Flipped");
                assert_ne!(render("not-flipped"), "Not flipped");
            }
        }
    }

    #[test]
    fn lookup_help_preserves_interval_examples_in_every_locale() {
        for language in Language::ALL
            .into_iter()
            .filter(|language| *language != Language::System)
        {
            let loader = FluentLanguageLoader::new("opencadstudio", "en-US".parse().unwrap());
            load_language(&loader, language).expect("Fluent resources must parse");
            let args = [("closed", "[2,6]"), ("open", "(2,6]")]
                .into_iter()
                .collect();
            let rendered = loader
                .with_fluent_message_and_bundle("block-lookup", |message, bundle| {
                    let pattern = message.get_attribute("input-help").unwrap();
                    let mut errors = Vec::new();
                    let text = bundle
                        .format_pattern(pattern.value(), Some(&args), &mut errors)
                        .into_owned();
                    assert!(errors.is_empty(), "{language:?}: {errors:?}");
                    text
                })
                .unwrap();
            assert!(rendered.contains("[2,6]"), "{language:?}: {rendered}");
            assert!(rendered.contains("(2,6]"), "{language:?}: {rendered}");
            for attribute in ["invalid-table", "invalid-value", "incompatible-values"] {
                assert!(!loader.get_attr("block-lookup", attribute).is_empty());
            }
        }
    }

    #[test]
    fn donation_text_is_translated_in_every_locale() {
        for language in Language::ALL
            .into_iter()
            .filter(|language| *language != Language::System)
        {
            let loader = FluentLanguageLoader::new("opencadstudio", "en-US".parse().unwrap());
            load_language(&loader, language).expect("Fluent resources must parse");
            let attributes: BTreeSet<String> =
                loader.with_message_iter(&language.requested()[0], |messages| {
                    messages
                        .filter(|message| message.id.name == "donation")
                        .flat_map(|message| {
                            message
                                .attributes
                                .iter()
                                .map(|attr| attr.id.name.to_owned())
                        })
                        .collect()
                });
            assert_eq!(
                attributes,
                BTreeSet::from(["title", "heading", "body", "decline"].map(String::from))
            );
            for attribute in attributes {
                assert!(!loader.get_attr("donation", &attribute).is_empty());
            }
        }
    }

    #[test]
    fn every_catalog_covers_and_formats_the_source_catalog() {
        let loader = FluentLanguageLoader::new("opencadstudio", "en-US".parse().unwrap());
        load_language(&loader, Language::EnUs).expect("English Fluent resources must parse");
        let keys = |loader: &FluentLanguageLoader, language: Language| {
            loader.with_message_iter(&language.requested()[0], |messages| {
                let entries: Vec<_> = messages
                    .flat_map(|message| {
                        std::iter::once((message.id.name.to_owned(), String::new())).chain(
                            message.attributes.iter().map(|attribute| {
                                (message.id.name.to_owned(), attribute.id.name.to_owned())
                            }),
                        )
                    })
                    .collect();
                let unique: BTreeSet<_> = entries.iter().cloned().collect();
                assert_eq!(
                    entries.len(),
                    unique.len(),
                    "duplicate keys in {language:?}"
                );
                unique
            })
        };
        let source = keys(&loader, Language::EnUs);
        assert!(!source.is_empty());

        let english = Localizations::get("en-US/opencadstudio.ftl").unwrap();
        let english = std::str::from_utf8(&english.data).unwrap();
        let variables: Vec<_> = english
            .split('$')
            .skip(1)
            .map(|tail| {
                tail.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_' && ch != '-')
                    .next()
                    .unwrap()
            })
            .collect();
        let args = variables.into_iter().map(|name| (name, 2)).collect();
        let render = |loader: &FluentLanguageLoader, message: &str, attribute: &str| {
            loader
                .with_fluent_message_and_bundle(message, |fluent_message, bundle| {
                    let pattern = if attribute.is_empty() {
                        fluent_message.value()
                    } else {
                        fluent_message
                            .get_attribute(attribute)
                            .map(|attr| attr.value())
                    };
                    pattern
                        .map(|pattern| {
                            let mut errors = Vec::new();
                            let text = bundle.format_pattern(pattern, Some(&args), &mut errors);
                            assert!(errors.is_empty(), "{message}.{attribute}: {errors:?}");
                            text.into_owned()
                        })
                        .unwrap_or_default()
                })
                .unwrap()
        };
        let markers = |text: &str| -> BTreeSet<String> {
            text.split("__ocs_")
                .skip(1)
                .filter_map(|tail| tail.split_once("__").map(|(marker, _)| marker.to_owned()))
                .collect()
        };
        for language in Language::ALL.into_iter().filter(|l| *l != Language::System) {
            let resource =
                Localizations::get(&format!("{}/opencadstudio.ftl", language.requested()[0]))
                    .unwrap();
            let text = std::str::from_utf8(&resource.data).unwrap();
            if let Err((_, errors)) = fluent_syntax::parser::parse(text) {
                panic!("Invalid Fluent syntax in {language:?}: {errors:?}");
            }
            let localized = FluentLanguageLoader::new("opencadstudio", "en-US".parse().unwrap());
            load_language(&localized, language).expect("Fluent resources must parse");
            let actual = keys(&localized, language);
            let extra: Vec<_> = actual.difference(&source).collect();
            assert!(extra.is_empty(), "extra keys in {language:?}: {extra:?}");
            let missing: Vec<_> = source.difference(&actual).collect();
            assert!(
                missing.is_empty(),
                "{} missing keys in {language:?}: {:?}",
                missing.len(),
                &missing[..missing.len().min(10)]
            );
            for (message, attribute) in &source {
                let expected = render(&loader, message, attribute);
                let translated = render(&localized, message, attribute);
                assert_eq!(
                    markers(&translated),
                    markers(&expected),
                    "{language:?}: {message}.{attribute}"
                );
                assert_eq!(
                    translated.is_empty(),
                    expected.is_empty(),
                    "{language:?}: {message}.{attribute}"
                );
            }
        }
    }

    #[test]
    fn greek_language_setting_round_trips() {
        let loader = FluentLanguageLoader::new("opencadstudio", "en-US".parse().unwrap());
        load_language(&loader, Language::ElGr).expect("Greek Fluent resources must parse");
        assert_eq!(loader.get_attr("language", "greek"), "Ελληνικά");
        assert_eq!(serde_json::to_string(&Language::ElGr).unwrap(), "\"el-GR\"");
    }

    #[test]
    fn open_infra_translation_round_trips() {
        // Test Arabic
        set_language(Language::ArSa).unwrap();
        assert_eq!(translate("Geo-locatie"), "الموقع الجغرافي");
        assert_eq!(translate("Kaartbeeld"), "صورة الخريطة");
        assert_eq!(translate("Vectordata"), "البيانات المتجهة");
        assert_eq!(translate("OpenStreetMap"), "خرائط الشوارع (OSM)");
        assert_eq!(translate("Kadaster"), "السجل العقاري");
        assert_eq!(translate("KLIC"), "شبكات المرافق والكابلات (KLIC)");
        assert_eq!(
            ribbon_module_title("openaec.open-infra", "Open Infra - GIS"),
            "البنية التحتية — نظم المعلومات الجغرافية"
        );

        // Test French
        set_language(Language::FrFr).unwrap();
        assert_eq!(translate("Geo-locatie"), "Géo-localisation");
        assert_eq!(translate("Kaartbeeld"), "Image cartographique");
        assert_eq!(translate("Vectordata"), "Données vectorielles");
        assert_eq!(translate("OpenStreetMap"), "OpenStreetMap (OSM)");
        assert_eq!(translate("Kadaster"), "Cadastre");
        assert_eq!(
            ribbon_module_title("openaec.open-infra", "Open Infra - GIS"),
            "Open Infra — SIG"
        );

        // Test English
        set_language(Language::EnUs).unwrap();
        assert_eq!(translate("Geo-locatie"), "Geo Location");
        assert_eq!(translate("Kaartbeeld"), "Map View");
        assert_eq!(translate("Vectordata"), "Vector Data");
        assert_eq!(translate("OpenStreetMap"), "OpenStreetMap");
        assert_eq!(translate("Kadaster"), "Cadastre");
        assert_eq!(
            ribbon_module_title("openaec.open-infra", "Open Infra - GIS"),
            "Open Infra — GIS"
        );

        // Reset
        let _ = set_language(Language::System);
    }
}
