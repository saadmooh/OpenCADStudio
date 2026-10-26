use crate::modules::{IconKind, ModuleEvent, ToolDef};

pub const ICON: IconKind = IconKind::Svg(include_bytes!("../../../assets/icons/content_browser.svg"));

pub fn tool() -> ToolDef {
    ToolDef {
        id: "TOOLSPACE",
        label: "Toolspace\n(Prospector)",
        icon: ICON,
        event: ModuleEvent::Command("TOOLSPACE".to_string()),
    }
}
