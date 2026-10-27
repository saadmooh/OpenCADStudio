use super::{Message, OpenCADStudio};
use crate::command::CadCommand;
use iced::Task;
use std::path::PathBuf;

pub(crate) mod civil_surface;
pub mod civil_parcel;
pub mod parcel_interactive;
mod attdef;
mod attman;
mod block_cmdline;
mod blocks;
mod blockvars;
mod block_editor_vars;
mod bvisibility;
mod count;
mod dash;
mod dgn_import;
mod dim;
mod dim_cmd;
mod dim_inspect;
mod dim_layer;
mod mleader_layer;
pub(crate) mod display;
mod draw;
mod field;
mod fileops;
mod hyperlink;
pub(crate) use hyperlink::{hyperlink_options, set_hyperlink_options};
mod inquiry;
mod layerprops;
mod layers;
mod model_import;
mod ole;
pub(crate) use ole::{ole_quality, set_ole_quality};
mod pc_colormap;
mod pc_extract;
mod pdf_dialogs;
pub(crate) mod pdf_import;
mod pdf_password;
mod pdf_shx_text;
mod pdf_underlay;
mod plotvars;
mod point_cloud;
mod refedit;
pub(crate) mod sheet_set;
mod spell;
mod styleprops;
mod text_edit;
mod text_layer;
mod ucsfollow;
mod view;
mod view_presets;
mod xclip;
mod xref_attach;

// `DrawOrderRefCommand` lives in the `view` family file but is referenced by
// path (`commands::DrawOrderRefCommand`) from `update.rs`, so re-export it at
// the module root to keep that path valid.
pub(crate) use view::DrawOrderCommand;

impl OpenCADStudio {

    pub(in crate::app) fn dispatch_civil3d_commands(
        &mut self,
        cmd: &str,
        i: usize,
    ) -> Option<Task<Message>> {
        let trimmed_cmd = cmd.trim();

        let upper_cmd = trimmed_cmd.to_ascii_uppercase();



        // ── Civil 3D Parcel Layout Tools Dialog ──────────────────────────

        if upper_cmd == "PARCELLAYOUT" || upper_cmd == "PARCELSIZING" || upper_cmd == "PARCELTOOLS" || upper_cmd == "PARCELLAYOUTTOOLS"

            || upper_cmd == "PC_LAYOUT_TOOLS" || upper_cmd == "PC_LAYOUT" {

            self.command_line.record_recent(cmd);

            if self.parcel_layout_state.is_none() {

                self.parcel_layout_state = Some(crate::ui::window::parcel_layout_tools::ParcelLayoutToolsState::default());

            }

            self.show_parcel_layout_tools = true;

            self.active_modal = None;

            return Some(Task::none());

        }



        // ── Civil 3D Parcel Properties Dialog ────────────────────────────

        if upper_cmd == "PARCELPROPS" || upper_cmd.starts_with("PARCELPROPS ") || upper_cmd.starts_with("PARCELPROPS:")

            || upper_cmd == "PARCELPROPERTIES" || upper_cmd.starts_with("PARCELPROPERTIES ") {

            let rest = if upper_cmd.starts_with("PARCELPROPS ") {

                trimmed_cmd[12..].trim()

            } else if upper_cmd.starts_with("PARCELPROPERTIES ") {

                trimmed_cmd[17..].trim()

            } else {

                ""

            };

            let parts: Vec<&str> = rest.split_whitespace().collect();

            let mut tab_kind = None;

            let hex_str = if let Some(&p0) = parts.first() {

                p0.trim_start_matches("0x").trim_start_matches("0X")

            } else {

                ""

            };

            if let Some(&p1) = parts.get(1) {

                let p1_up = p1.to_ascii_uppercase();

                if p1_up.starts_with("COMP") || p1_up.starts_with("LEG") {

                    tab_kind = Some(crate::ui::window::parcel_properties::ParcelPropertiesTab::Composition);

                } else if p1_up.starts_with("ANA") || p1_up.starts_with("MAP") {

                    tab_kind = Some(crate::ui::window::parcel_properties::ParcelPropertiesTab::Analysis);

                } else if p1_up.starts_with("STAT") {

                    tab_kind = Some(crate::ui::window::parcel_properties::ParcelPropertiesTab::Statistics);

                }

            }

            let target_handle = if let Ok(val) = u64::from_str_radix(hex_str, 16) {

                Some(codec::Handle::new(val))

            } else {

                self.tabs[i].scene.selected_entities().into_iter().find_map(|(_, e)| civil_parcel::ParcelData::from_entity(e).map(|p| p.handle))

                    .or_else(|| civil_parcel::ParcelData::all_parcels(&self.tabs[i].scene.document).first().map(|p| p.handle))

            };



            if let Some(h) = target_handle {

                self.command_line.record_recent(cmd);

                if let Some(tab) = self.tabs.get(i) {

                    if let Some(p) = civil_parcel::ParcelData::from_document_entity(&tab.scene.document, h) {

                        let mut state = crate::ui::window::parcel_properties::ParcelPropertiesState::from_parcel(&p);

                        if let Some(tk) = tab_kind {

                            state.active_tab = tk;

                        }

                        self.parcel_properties_state = Some(state);

                        self.active_modal = Some(crate::app::ModalKind::ParcelProperties);

                    }

                }

                return Some(Task::none());

            } else {

                self.command_line.push_error("No parcel found in drawing to open properties.");

                return Some(Task::none());

            }

        }



        // ── Civil 3D Parcel Dynamic Synchronization ──────────────────────

        if upper_cmd == "PARCELUPDATE" || upper_cmd == "PARCELSYNC" || upper_cmd == "PARCELREFRESH"

            || upper_cmd == "PC_UPDATE" || upper_cmd == "PC_SYNC" {

            self.command_line.record_recent(cmd);

            if let Some(tab) = self.tabs.get_mut(i) {

                let count = civil_parcel::ParcelData::sync_dynamic_parcels(&mut tab.scene.document, None, None);

                tab.scene.bump_geometry();

                tab.dirty = true;

                self.command_line.push_output(&format!("✔ Dynamically synchronized {} parcel(s), centroid labels, and area table.", count));

            }

            return Some(Task::none());

        }



        // ── Civil 3D Interactive Slide Line Subdivision ──────────────────

        if upper_cmd == "SLIDELINE" || upper_cmd == "PARCELSLIDELINE"

            || (upper_cmd == "PC_SLIDE_LINE" && trimmed_cmd == "PC_SLIDE_LINE")

            || upper_cmd.starts_with("SLIDELINE ")

        {

            let criteria = if let Some(st) = &self.parcel_layout_state {
                parcel_interactive::ParcelSizingCriteria::from_state(st)
            } else {
                parcel_interactive::ParcelSizingCriteria::default()
            };
            let cmd_obj = parcel_interactive::SlideLineCommand::new_with_criteria(criteria);

            self.command_line.record_recent(cmd);

            self.command_line.push_info(&cmd_obj.prompt());

            self.tabs[i].active_cmd = Some(Box::new(cmd_obj));

            self.active_modal = None;

            return Some(Task::none());

        }



        // ── Civil 3D Resolve Parcel Overlaps (Site Topology) ───────────────
        if upper_cmd == "PARCELRESOLVEOVERLAPS" || upper_cmd == "RESOLVEPARCELS"
            || upper_cmd == "PARCELTOPOLOGY" || upper_cmd == "PC_RESOLVE"
        {
            self.command_line.record_recent(cmd);
            let tab_idx = self.active_tab;
            let count = crate::app::commands::civil_parcel::ParcelData::resolve_all_site_overlaps(
                &mut self.tabs[tab_idx].scene.document,
            );
            crate::app::commands::civil_parcel::ParcelData::sync_dynamic_parcels(
                &mut self.tabs[tab_idx].scene.document,
                None,
                None,
            );
            self.tabs[tab_idx].scene.populate_hatches_from_document();
            self.tabs[tab_idx].scene.bump_geometry();
            self.tabs[tab_idx].dirty = true;
            if count > 0 {
                self.command_line.push_output(&format!(
                    "✔ Successfully resolved {} overlapping parcel boundary(ies) in Site topology.",
                    count
                ));
            } else {
                self.command_line.push_output("✔ All parcels within each Site have non-overlapping topology.");
            }
            return Some(Task::none());
        }

        // ── Civil 3D Create Parcel from Objects ───────────────────────────

        if upper_cmd == "PARCELFROMOBJECTS" || upper_cmd == "CREATEPARCELFROMOBJECTS"

            || (upper_cmd == "PC_CREATE" && trimmed_cmd == "PC_CREATE")

        {

            self.command_line.record_recent(cmd);

            return Some(Task::done(Message::OpenCreateParcelsLayout(Some("objects".to_string()))));

        }



        // ── Civil 3D Interactive Parcel Right-of-Way ───────────────────────

        if upper_cmd == "PARCELROW" || upper_cmd == "ROW" || upper_cmd == "CREATEROW"

            || (upper_cmd == "PC_ROW" && trimmed_cmd == "PC_ROW")

            || upper_cmd.starts_with("PARCELROW ")

        {

            let width = if upper_cmd.starts_with("PARCELROW ") {

                trimmed_cmd[10..].trim().parse::<f64>().unwrap_or(20.0)

            } else {

                20.0

            };

            let cmd_obj = parcel_interactive::ParcelRowCommand::new(width);

            self.command_line.record_recent(cmd);

            self.command_line.push_info(&cmd_obj.prompt());

            self.tabs[i].active_cmd = Some(Box::new(cmd_obj));

            self.active_modal = None;

            return Some(Task::none());

        }



        // ── Civil 3D Swing Line ───────────────────────────────────────────

        if upper_cmd == "SWINGLINE" || upper_cmd == "PARCELSWINGLINE"

            || (upper_cmd == "PC_SWING_LINE" && trimmed_cmd == "PC_SWING_LINE")

            || upper_cmd.starts_with("SWINGLINE ")

        {

            let criteria = if let Some(st) = &self.parcel_layout_state {
                parcel_interactive::ParcelSizingCriteria::from_state(st)
            } else {
                parcel_interactive::ParcelSizingCriteria::default()
            };
            let cmd_obj = parcel_interactive::SwingLineCommand::new_with_criteria(criteria);

            self.command_line.record_recent(cmd);

            self.command_line.push_info(&cmd_obj.prompt());

            self.tabs[i].active_cmd = Some(Box::new(cmd_obj));

            self.show_parcel_layout_tools = true;

            if let Some(st) = &mut self.parcel_layout_state {

                st.active_tool = "Swing Line".to_string();

            }

            self.active_modal = None;

            return Some(Task::none());

        }



        // ── Civil 3D Swing Line Edit ──────────────────────────────────────

        if upper_cmd == "SWINGLINEEDIT" || upper_cmd == "PARCELSWINGLINEEDIT"

            || upper_cmd == "SWINGLINE_EDIT"

            || (upper_cmd == "PC_SWING_LINE_EDIT" && trimmed_cmd == "PC_SWING_LINE_EDIT")

            || upper_cmd.starts_with("SWINGLINEEDIT ")

        {

            let criteria = if let Some(st) = &self.parcel_layout_state {
                parcel_interactive::ParcelSizingCriteria::from_state(st)
            } else {
                parcel_interactive::ParcelSizingCriteria::default()
            };
            let cmd_obj = parcel_interactive::SwingLineEditCommand::new_with_criteria(criteria);

            self.command_line.record_recent(cmd);

            self.command_line.push_info(&cmd_obj.prompt());

            self.tabs[i].active_cmd = Some(Box::new(cmd_obj));

            self.show_parcel_layout_tools = true;

            if let Some(st) = &mut self.parcel_layout_state {

                st.active_tool = "Swing Edit".to_string();

            }

            self.active_modal = None;

            return Some(Task::none());

        }



        // ── Civil 3D Create Rectangular Parcel ───────────────────────────
        if upper_cmd == "RECTLOT" || upper_cmd == "RECT_LOT" || upper_cmd == "RECT LOT"
            || upper_cmd == "PARCELRECT" || upper_cmd == "PARCEL_RECT" || upper_cmd == "RECTPARCEL"
            || upper_cmd == "CREATERECT" || upper_cmd == "CREATE_RECT"
            || (upper_cmd == "PC_CREATE_RECT" && trimmed_cmd == "PC_CREATE_RECT")
            || upper_cmd == "PC_RECT"
        {
            self.command_line.record_recent(cmd);
            return Some(Task::done(Message::OpenCreateParcelsLayout(Some("rect".to_string()))));
        }

        // ── Civil 3D Split Parcel by Cutting Line ────────────────────────
        if upper_cmd == "PARCELSPLIT" || upper_cmd == "SPLIT" || upper_cmd == "SPLITLINE"
            || upper_cmd == "SPLIT_LINE" || upper_cmd == "PARCEL_SPLIT"
            || (upper_cmd == "PC_SPLIT" && trimmed_cmd == "PC_SPLIT")
        {
            let cmd_obj = parcel_interactive::ParcelSplitCommand::new();
            self.command_line.record_recent(cmd);
            self.command_line.push_info(&cmd_obj.prompt());
            self.tabs[i].active_cmd = Some(Box::new(cmd_obj));
            self.active_modal = None;
            return Some(Task::none());
        }

        // ── Civil 3D Automated Parcel Subdivision ────────────────────────
        if upper_cmd == "PARCELSUBDIVIDE" || upper_cmd == "SUBDIVIDE" || upper_cmd == "SUBDIVIDED"
            || upper_cmd == "AUTOSUBDIVIDE" || upper_cmd == "AUTO_SUBDIVIDE"
            || upper_cmd == "PARCEL_SUBDIVIDE"
            || (upper_cmd == "PC_SUBDIVIDE" && trimmed_cmd == "PC_SUBDIVIDE")
        {
            let (min_area, min_frontage, remainder_mode) = if let Some(st) = &self.parcel_layout_state {
                (
                    st.min_area.parse::<f64>().unwrap_or(350.0),
                    st.min_frontage.parse::<f64>().unwrap_or(12.0),
                    st.remainder_mode.clone(),
                )
            } else {
                (350.0, 12.0, "redistribute".to_string())
            };
            let cmd_obj = parcel_interactive::ParcelSubdivideCommand::new(min_area, min_frontage, remainder_mode);
            self.command_line.record_recent(cmd);
            self.command_line.push_info(&cmd_obj.prompt());
            self.tabs[i].active_cmd = Some(Box::new(cmd_obj));
            self.active_modal = None;
            return Some(Task::none());
        }

        // ── Civil 3D Parcel Merge / Union ─────────────────────────────────
        if upper_cmd == "PARCELMERGE" || upper_cmd == "MERGE" || upper_cmd == "PARCEL_MERGE"
            || (upper_cmd == "PC_MERGE" && trimmed_cmd == "PC_MERGE")
        {
            let cmd_obj = parcel_interactive::ParcelMergeCommand::new();
            self.command_line.record_recent(cmd);
            self.command_line.push_info(&cmd_obj.prompt());
            self.tabs[i].active_cmd = Some(Box::new(cmd_obj));
            self.active_modal = None;
            return Some(Task::none());
        }

        // ── Civil 3D Parcel Setback Offset ────────────────────────────────
        if upper_cmd == "PARCELSETBACK" || upper_cmd == "SETBACK" || upper_cmd == "PARCEL_SETBACK"
            || (upper_cmd == "PC_SETBACK" && trimmed_cmd == "PC_SETBACK")
        {
            let dist = if let Some(st) = &self.parcel_layout_state {
                st.frontage_offset.parse::<f64>().unwrap_or(4.0)
            } else {
                4.0
            };
            let cmd_obj = parcel_interactive::ParcelSetbackCommand::new(dist);
            self.command_line.record_recent(cmd);
            self.command_line.push_info(&cmd_obj.prompt());
            let opts = cmd_obj.options();
            self.command_line.set_step_options(opts);
            self.tabs[i].active_cmd = Some(Box::new(cmd_obj));
            self.active_modal = None;
            return Some(Task::none());
        }

        // ── Civil 3D Parcel Renumber / Rename (Interactive Mouse & Fence) ──
        if upper_cmd == "PARCELRENUMBER" || upper_cmd == "RENUMBER" || upper_cmd == "PARCEL_RENUMBER"
            || (upper_cmd == "PC_RENUMBER" && trimmed_cmd == "PC_RENUMBER")
        {
            let cmd_obj = parcel_interactive::ParcelRenumberCommand::new("Lot_".to_string(), 1, 1);
            self.command_line.record_recent(cmd);
            self.command_line.push_info(&cmd_obj.prompt());
            let opts = cmd_obj.options();
            self.command_line.set_step_options(opts);
            self.tabs[i].active_cmd = Some(Box::new(cmd_obj));
            self.active_modal = None;
            return Some(Task::none());
        }

        // ── Civil 3D Parcel Centroid Label ────────────────────────────────
        if upper_cmd == "PARCELLABEL" || upper_cmd == "PARCEL_LABEL"
            || (upper_cmd == "PC_LABEL" && trimmed_cmd == "PC_LABEL")
        {
            let cmd_obj = parcel_interactive::ParcelLabelCommand::new();
            self.command_line.record_recent(cmd);
            self.command_line.push_info(&cmd_obj.prompt());
            self.tabs[i].active_cmd = Some(Box::new(cmd_obj));
            self.active_modal = None;
            return Some(Task::none());
        }

        // ── Civil 3D Parcel Segment Labels ────────────────────────────────
        if upper_cmd == "PARCELLABELOGSEGS" || upper_cmd == "PARCEL_LABEL_SEGS" || upper_cmd == "PARCELLABEL_SEGS"
            || (upper_cmd == "PC_LABEL_SEGS" && trimmed_cmd == "PC_LABEL_SEGS")
        {
            let cmd_obj = parcel_interactive::ParcelLabelSegsCommand::new();
            self.command_line.record_recent(cmd);
            self.command_line.push_info(&cmd_obj.prompt());
            self.tabs[i].active_cmd = Some(Box::new(cmd_obj));
            self.active_modal = None;
            return Some(Task::none());
        }

        // ── Civil 3D Parcel Area Table Insertion ──────────────────────────
        if upper_cmd == "PARCELTABLE" || upper_cmd == "PARCEL_TABLE" || upper_cmd == "TABLEAREA"
            || (upper_cmd == "PC_TABLE_AREA" && trimmed_cmd == "PC_TABLE_AREA")
        {
            let cmd_obj = parcel_interactive::ParcelTableInsertCommand::new();
            self.command_line.record_recent(cmd);
            self.command_line.push_info(&cmd_obj.prompt());
            self.tabs[i].active_cmd = Some(Box::new(cmd_obj));
            self.active_modal = None;
            return Some(Task::none());
        }

        // ── Civil 3D Parcel Curve Table Insertion ─────────────────────────
        if upper_cmd == "PARCELCURVETABLE" || upper_cmd == "PARCEL_CURVE_TABLE" || upper_cmd == "TABLECURVES"
            || (upper_cmd == "PC_TABLE_CURVES" && trimmed_cmd == "PC_TABLE_CURVES")
        {
            let cmd_obj = parcel_interactive::ParcelCurveTableInsertCommand::new();
            self.command_line.record_recent(cmd);
            self.command_line.push_info(&cmd_obj.prompt());
            self.tabs[i].active_cmd = Some(Box::new(cmd_obj));
            self.active_modal = None;
            return Some(Task::none());
        }

        // ── Civil 3D Parcel Line Table Insertion ──────────────────────────
        if upper_cmd == "PARCELLINETABLE" || upper_cmd == "PARCEL_LINE_TABLE" || upper_cmd == "TABLELINES"
            || (upper_cmd == "PC_TABLE_LINES" && trimmed_cmd == "PC_TABLE_LINES")
        {
            let cmd_obj = parcel_interactive::ParcelLineTableCommand::new();
            self.command_line.record_recent(cmd);
            self.command_line.push_info(&cmd_obj.prompt());
            self.tabs[i].active_cmd = Some(Box::new(cmd_obj));
            self.active_modal = None;
            return Some(Task::none());
        }

        // ── Civil 3D Parcel Mapcheck Survey Verification ──────────────────
        if upper_cmd == "PARCELMAPCHECK" || upper_cmd == "PARCEL_MAPCHECK"
            || (upper_cmd == "PC_MAPCHECK" && trimmed_cmd == "PC_MAPCHECK")
        {
            let cmd_obj = parcel_interactive::ParcelMapcheckCommand::new();
            self.command_line.record_recent(cmd);
            self.command_line.push_info(&cmd_obj.prompt());
            self.tabs[i].active_cmd = Some(Box::new(cmd_obj));
            self.active_modal = None;
            return Some(Task::none());
        }

        // ── Civil 3D Parcel Thematic Styling & Styles Manager ──────────────
        if upper_cmd == "PARCELSTYLE" || upper_cmd == "PARCEL_STYLE"
            || upper_cmd == "PC_THEMATIC" || upper_cmd == "PARCELTHEMATIC" || upper_cmd == "THEMATIC"
            || (upper_cmd == "PC_SET_STYLE" && trimmed_cmd == "PC_SET_STYLE")
        {
            self.command_line.record_recent(cmd);
            if self.parcel_styles_state.is_none() {
                self.parcel_styles_state = Some(crate::ui::window::parcel_styles::ParcelStylesDialogState::default());
            }
            self.active_modal = Some(crate::app::ModalKind::ParcelStyles);
            return Some(Task::none());
        }

        // ── Civil 3D Parcel Zoning Legend Insertion ───────────────────────
        if upper_cmd == "PARCELLEGEND" || upper_cmd == "PARCEL_LEGEND" || upper_cmd == "PC_LEGEND" {
            self.command_line.record_recent(cmd);
            let tab = &mut self.tabs[i];
            let default_styles = crate::ui::window::parcel_styles::ParcelStylesDialogState::default();
            let styles = self.parcel_styles_state.as_ref().map(|s| &s.styles).unwrap_or(&default_styles.styles);
            let parcels = crate::app::commands::civil_parcel::ParcelData::all_parcels(&tab.scene.document);
            let origin = if let Some(first) = parcels.first() {
                let mut max_x = first.bbox_max[0];
                let mut max_y = first.bbox_max[1];
                for p in &parcels {
                    max_x = max_x.max(p.bbox_max[0]);
                    max_y = max_y.max(p.bbox_max[1]);
                }
                [max_x + 12.0, max_y]
            } else {
                [10.0, 50.0]
            };
            let rows = crate::ui::window::parcel_styles::draw_parcel_zoning_legend(
                &mut tab.scene.document,
                styles,
                origin,
                2.5,
            );
            tab.scene.bump_geometry();
            tab.dirty = true;
            self.command_line.push_output(&format!(
                "✔ Inserted Parcel Zoning Legend ({} category rows) at ({:.1}, {:.1}).",
                rows, origin[0], origin[1]
            ));
            return Some(Task::none());
        }

        if !self.suppress_plugin_dispatch && crate::plugin::try_dispatch(self, i, cmd) {

            // try_dispatch returns true for both finished commands and interactive

            // commands that it just installed. If no command is now active, the

            // tool was a one-shot and we must turn the ribbon highlight off here —

            // normally apply_cmd_result does that, but plugin dispatch can return

            // without producing a CmdResult.

            self.tabs[i].last_cmd = Some(cmd.to_string());

            self.command_line.record_recent(cmd);

            if self.tabs[i].active_cmd.is_none() {

                self.ribbon.deactivate_tool();

            }

            return Some(self.finish_dispatch(cmd));

        }



        // ── Civil 3D Parcel Line Table Native Handler / Fallback ───────────

        if upper_cmd.starts_with("PC_TABLE_LINES ") || upper_cmd.starts_with("PARCELLINETABLE ") {

            let rest = if upper_cmd.starts_with("PC_TABLE_LINES ") {

                trimmed_cmd[15..].trim()

            } else {

                trimmed_cmd[16..].trim()

            };

            let mut parts = Vec::new();

            let mut cur = String::new();

            let mut in_quotes = false;

            for c in rest.chars() {

                if c == '"' {

                    in_quotes = !in_quotes;

                } else if c.is_whitespace() && !in_quotes {

                    if !cur.is_empty() {

                        parts.push(cur.clone());

                        cur.clear();

                    }

                } else {

                    cur.push(c);

                }

            }

            if !cur.is_empty() {

                parts.push(cur);

            }



            let parcel_name = parts.get(0).cloned().unwrap_or_default();

            let ox: f64 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.0);

            let oy: f64 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0);



            if let Some(tab) = self.tabs.get_mut(i) {

                let parcels = civil_parcel::ParcelData::all_parcels(&tab.scene.document);

                let target_p = parcels.iter().find(|p| {

                    p.name.eq_ignore_ascii_case(&parcel_name)

                        || format!("{:X}", p.handle.value()).eq_ignore_ascii_case(&parcel_name)

                        || format!("Lot_{:X}", p.handle.value()).eq_ignore_ascii_case(&parcel_name)

                }).cloned();



                if let Some(p) = target_p {

                    let text_h = 2.0;

                    civil_parcel::draw_parcel_line_table(&mut tab.scene.document, &p, [ox, oy], text_h);

                    tab.scene.bump_geometry();

                    tab.dirty = true;

                    self.command_line.record_recent(cmd);

                    self.command_line.push_info(&format!(

                        "PC_TABLE_LINES: Inserted Line Table for '{}' at ({:.2}, {:.2})",

                        p.name, ox, oy

                    ));

                    return Some(Task::none());

                } else {

                    self.command_line.push_error(&format!("Parcel '{}' not found in drawing.", parcel_name));

                    return Some(Task::none());

                }

            }

        }



        // Direct entity selection by handle (e.g. from the Browser / Prospector Tree: `SELECT 9D5`)

        if upper_cmd.starts_with("SELECT ") || upper_cmd.starts_with("SELECT:") {

            let rest = if upper_cmd.starts_with("SELECT ") {

                &trimmed_cmd[7..]

            } else {

                &trimmed_cmd[7..]

            }.trim();

            let mut parsed_handles = Vec::new();

            for part in rest.split(|c: char| c.is_whitespace() || c == ',') {

                let s = part.trim().trim_start_matches("0x").trim_start_matches("0X");

                if !s.is_empty() {

                    if let Ok(val) = u64::from_str_radix(s, 16) {

                        parsed_handles.push(codec::Handle::new(val));

                    }

                }

            }

            if !parsed_handles.is_empty() {

                self.tabs[i].scene.deselect_all();

                for &h in &parsed_handles {

                    self.tabs[i].scene.select_entity(h, false);

                }

                self.tabs[i].scene.remember_current_view();

                let _ = self.tabs[i].scene.zoom_to_entities(&parsed_handles);

                self.refresh_selected_grips();

                self.refresh_properties();

                self.command_line.record_recent(cmd);

                return Some(Task::none());

            }

        }



        // ── Surface Operations: Statistics ──────────────────────────────

        if upper_cmd == "SURFACESTATS" || upper_cmd.starts_with("SURFACESTATS ") || upper_cmd.starts_with("SURFACESTATS:") {

            let rest = if upper_cmd.starts_with("SURFACESTATS ") {

                trimmed_cmd[12..].trim()

            } else if upper_cmd.starts_with("SURFACESTATS:") {

                trimmed_cmd[13..].trim()

            } else {

                ""

            };

            let hex_str = rest.trim_start_matches("0x").trim_start_matches("0X");

            let target_surf = if let Ok(val) = u64::from_str_radix(hex_str, 16) {

                civil_surface::TinSurfaceData::from_document_entity(&self.tabs[i].scene.document, codec::Handle::new(val))

            } else {

                self.tabs[i].scene.selected_entities().into_iter().find_map(|(_, e)| civil_surface::TinSurfaceData::from_entity(e))

                    .or_else(|| self.tabs[i].scene.document.entities().filter_map(civil_surface::TinSurfaceData::from_entity).next())

            };



            if let Some(surf) = target_surf {

                let report = civil_surface::format_surface_statistics(&surf);

                self.command_line.push_info(&report);

                self.command_line.record_recent(cmd);

                return Some(Task::none());

            } else {

                self.command_line.push_error("No TIN surface found in drawing or selection.");

                return Some(Task::none());

            }

        }



        // ── Surface Operations: Create Surface from Points ──────────────

        if upper_cmd == "SURFACECREATE" || upper_cmd.starts_with("SURFACECREATE ") || upper_cmd.starts_with("SURFACECREATE:") {

            let rest = if upper_cmd.starts_with("SURFACECREATE ") {

                trimmed_cmd[14..].trim()

            } else if upper_cmd.starts_with("SURFACECREATE:") {

                trimmed_cmd[15..].trim()

            } else {

                ""

            };

            let parts: Vec<&str> = rest.split_whitespace().collect();

            let surf_name = parts.first().copied().unwrap_or("Surface1");

            let layer_filter = parts.get(1).copied();



            match civil_surface::create_surface_from_points(&mut self.tabs[i].scene.document, surf_name, layer_filter) {

                Ok((handle, pt_count, tri_count)) => {

                    self.tabs[i].scene.bump_geometry();

                    self.tabs[i].dirty = true;

                    self.command_line.record_recent(cmd);

                    self.command_line.push_output(&format!(

                        "✔ Created TIN Surface '{}' [{:X}] from {} points ({} triangles).",

                        surf_name, handle.value(), pt_count, tri_count

                    ));

                    return Some(Task::none());

                }

                Err(err) => {

                    self.command_line.push_error(&format!("SURFACECREATE failed: {}", err));

                    return Some(Task::none());

                }

            }

        }



        // ── Surface Operations: Surface Properties Dialog ──────────────

        if upper_cmd == "SURFACEPROPS" || upper_cmd.starts_with("SURFACEPROPS ") || upper_cmd.starts_with("SURFACEPROPS:")

            || upper_cmd == "SURFACEPROPERTIES" || upper_cmd.starts_with("SURFACEPROPERTIES ") {

            let rest = if upper_cmd.starts_with("SURFACEPROPS ") {

                trimmed_cmd[13..].trim()

            } else if upper_cmd.starts_with("SURFACEPROPERTIES ") {

                trimmed_cmd[18..].trim()

            } else {

                ""

            };

            let parts: Vec<&str> = rest.split_whitespace().collect();

            let mut tab_kind = None;

            let hex_str = if let Some(&p0) = parts.first() {

                p0.trim_start_matches("0x").trim_start_matches("0X")

            } else {

                ""

            };

            if let Some(&p1) = parts.get(1) {

                let p1_up = p1.to_ascii_uppercase();

                if p1_up.starts_with("DEF") {

                    tab_kind = Some(crate::ui::window::surface_properties::SurfacePropertiesTab::Definition);

                } else if p1_up.starts_with("ANA") {

                    tab_kind = Some(crate::ui::window::surface_properties::SurfacePropertiesTab::Analysis);

                } else if p1_up.starts_with("STAT") {

                    tab_kind = Some(crate::ui::window::surface_properties::SurfacePropertiesTab::Statistics);

                }

            }

            let target_handle = if let Ok(val) = u64::from_str_radix(hex_str, 16) {

                Some(codec::Handle::new(val))

            } else {

                self.tabs[i].scene.selected_entities().into_iter().find_map(|(_, e)| civil_surface::TinSurfaceData::from_entity(e).map(|s| s.handle))

                    .or_else(|| self.tabs[i].scene.document.entities().filter_map(civil_surface::TinSurfaceData::from_entity).map(|s| s.handle).next())

            };



            if let Some(h) = target_handle {

                self.command_line.record_recent(cmd);

                if let Some(tab) = self.tabs.get(i) {

                    if let Some(surf) = civil_surface::TinSurfaceData::from_document_entity(&tab.scene.document, h) {

                        let mut state = crate::ui::window::surface_properties::SurfacePropertiesState::from_surface(&surf);

                        if let Some(tk) = tab_kind {

                            state.active_tab = tk;

                        }

                        self.surface_properties_state = Some(state);

                        self.active_modal = Some(crate::app::ModalKind::SurfaceProperties);

                    }

                }

                return Some(Task::none());

            } else {

                self.command_line.push_error("No surface found to open properties.");

                return Some(Task::none());

            }

        }



        // ── Surface Operations: Generate Contours ───────────────────────

        if upper_cmd == "SURFACECONTOURS" || upper_cmd.starts_with("SURFACECONTOURS ") || upper_cmd.starts_with("SURFACECONTOURS:") {

            let rest = if upper_cmd.starts_with("SURFACECONTOURS ") {

                trimmed_cmd[15..].trim()

            } else if upper_cmd.starts_with("SURFACECONTOURS:") {

                trimmed_cmd[16..].trim()

            } else {

                ""

            };

            let parts: Vec<&str> = rest.split_whitespace().collect();

            let mut handle_opt = None;

            let mut interval = 1.0;



            if let Some(&p0) = parts.first() {

                let hex_str = p0.trim_start_matches("0x").trim_start_matches("0X");

                if let Ok(val) = u64::from_str_radix(hex_str, 16) {

                    handle_opt = Some(codec::Handle::new(val));

                    if let Some(p1) = parts.get(1) {

                        interval = p1.parse().unwrap_or(1.0);

                    }

                } else if let Ok(val) = p0.parse::<f64>() {

                    interval = val;

                }

            }



            let target_surf = handle_opt

                .and_then(|h| civil_surface::TinSurfaceData::from_document_entity(&self.tabs[i].scene.document, h))

                .or_else(|| self.tabs[i].scene.selected_entities().into_iter().find_map(|(_, e)| civil_surface::TinSurfaceData::from_entity(e)))

                .or_else(|| self.tabs[i].scene.document.entities().filter_map(civil_surface::TinSurfaceData::from_entity).next());



            if let Some(surf) = target_surf {

                civil_surface::ensure_layer_with_color(&mut self.tabs[i].scene.document, "LS-CONTOUR-MAJOR", 1);

                civil_surface::ensure_layer_with_color(&mut self.tabs[i].scene.document, "LS-CONTOUR-MINOR", 8);



                let (major, minor) = civil_surface::generate_contours_chained_smoothed(&surf, interval, true);

                let major_count = major.len();

                let minor_count = minor.len();



                for pl in major {

                    let _ = self.tabs[i].scene.document.add_entity(codec::EntityType::Polyline3D(pl));

                }

                for pl in minor {

                    let _ = self.tabs[i].scene.document.add_entity(codec::EntityType::Polyline3D(pl));

                }



                self.tabs[i].scene.bump_geometry();

                self.tabs[i].dirty = true;

                self.command_line.record_recent(cmd);

                self.command_line.push_output(&format!(

                    "✔ Generated {} major & {} minor smoothed contour polylines for surface '{}' (Minor: {:.2}m, Major: {:.2}m).",

                    major_count, minor_count, surf.name, interval, interval * 5.0

                ));

                return Some(Task::none());

            } else {

                self.command_line.push_error("No TIN surface found in drawing or selection for contour generation.");

                return Some(Task::none());

            }

        }



        // ── Surface Operations: Earthwork Cut & Fill Volumes ────────────

        if upper_cmd == "SURFACEVOLUME" || upper_cmd.starts_with("SURFACEVOLUME ") || upper_cmd.starts_with("SURFACEVOLUME:") {

            let rest = if upper_cmd.starts_with("SURFACEVOLUME ") {

                trimmed_cmd[13..].trim()

            } else if upper_cmd.starts_with("SURFACEVOLUME:") {

                trimmed_cmd[14..].trim()

            } else {

                ""

            };



            let parts: Vec<&str> = rest.split_whitespace().collect();

            let (base_surf, comp_surf) = if parts.len() >= 2 {

                let h0 = u64::from_str_radix(parts[0].trim_start_matches("0x").trim_start_matches("0X"), 16).ok().map(codec::Handle::new);

                let h1 = u64::from_str_radix(parts[1].trim_start_matches("0x").trim_start_matches("0X"), 16).ok().map(codec::Handle::new);

                (

                    h0.and_then(|h| civil_surface::TinSurfaceData::from_document_entity(&self.tabs[i].scene.document, h)),

                    h1.and_then(|h| civil_surface::TinSurfaceData::from_document_entity(&self.tabs[i].scene.document, h)),

                )

            } else {

                let all_surfs: Vec<civil_surface::TinSurfaceData> = self.tabs[i]

                    .scene

                    .document

                    .entities()

                    .filter_map(civil_surface::TinSurfaceData::from_entity)

                    .collect();

                if all_surfs.len() >= 2 {

                    let base_idx = all_surfs.iter().position(|s| s.name.to_ascii_uppercase().contains("EXIST") || s.name.to_ascii_uppercase() == "EG").unwrap_or(0);

                    let comp_idx = if base_idx == 0 { 1 } else { 0 };

                    (Some(all_surfs[base_idx].clone()), Some(all_surfs[comp_idx].clone()))

                } else {

                    (None, None)

                }

            };



            match (base_surf, comp_surf) {

                (Some(base), Some(comp)) => {

                    if let Some(vol) = civil_surface::calculate_volumes(&base, &comp) {

                        let report = civil_surface::format_volume_report(&vol);

                        self.command_line.push_info(&report);

                        self.command_line.record_recent(cmd);

                        return Some(Task::none());

                    } else {

                        self.command_line.push_error("The selected surfaces do not have an overlapping boundary in XY.");

                        return Some(Task::none());

                    }

                }

                _ => {

                    self.command_line.push_error("SURFACEVOLUME requires 2 TIN surfaces in the drawing (or specify handles: SURFACEVOLUME <base> <comp>).");

                    return Some(Task::none());

                }

            }

        }



        // ── Surface Operations: Export to LandXML ───────────────────────

        if upper_cmd == "SURFACELANDXML" || upper_cmd.starts_with("SURFACELANDXML ") || upper_cmd.starts_with("SURFACELANDXML:") {

            let rest = if upper_cmd.starts_with("SURFACELANDXML ") {

                trimmed_cmd[14..].trim()

            } else if upper_cmd.starts_with("SURFACELANDXML:") {

                trimmed_cmd[15..].trim()

            } else {

                ""

            };

            let hex_str = rest.trim_start_matches("0x").trim_start_matches("0X");

            let target_surf = if let Ok(val) = u64::from_str_radix(hex_str, 16) {

                civil_surface::TinSurfaceData::from_document_entity(&self.tabs[i].scene.document, codec::Handle::new(val))

            } else {

                self.tabs[i].scene.selected_entities().into_iter().find_map(|(_, e)| civil_surface::TinSurfaceData::from_entity(e))

                    .or_else(|| self.tabs[i].scene.document.entities().filter_map(civil_surface::TinSurfaceData::from_entity).next())

            };



            if let Some(surf) = target_surf {

                let xml = civil_surface::export_to_landxml(&surf);

                let out_path = std::env::var("USERPROFILE")

                    .map(|p| std::path::PathBuf::from(p).join("Desktop").join("Grading_Project").join(format!("{}.xml", surf.name)))

                    .unwrap_or_else(|_| std::path::PathBuf::from(format!("{}.xml", surf.name)));

                if std::fs::write(&out_path, xml).is_ok() {

                    self.command_line.push_output(&format!("✔ Exported surface '{}' to LandXML: {}", surf.name, out_path.display()));

                } else {

                    self.command_line.push_error(&format!("Failed to write LandXML to {}", out_path.display()));

                }

                self.command_line.record_recent(cmd);

                return Some(Task::none());

            } else {

                self.command_line.push_error("No TIN surface found to export.");

                return Some(Task::none());

            }

        }



        // ── Surface Operations: Swap Edge ──────────────────────────────

        if upper_cmd == "SURFACESWAPEDGE" || upper_cmd.starts_with("SURFACESWAPEDGE ") || upper_cmd.starts_with("SURFACESWAPEDGE:") {

            let rest = if upper_cmd.starts_with("SURFACESWAPEDGE ") {

                trimmed_cmd[16..].trim()

            } else if upper_cmd.starts_with("SURFACESWAPEDGE:") {

                trimmed_cmd[17..].trim()

            } else {

                ""

            };

            let parts: Vec<&str> = rest.split_whitespace().collect();

            let mut handle_opt = None;

            let mut tri_idx = None;



            if let Some(&p0) = parts.first() {

                let hex_str = p0.trim_start_matches("0x").trim_start_matches("0X");

                if let Ok(val) = u64::from_str_radix(hex_str, 16) {

                    handle_opt = Some(codec::Handle::new(val));

                    if let Some(p1) = parts.get(1) {

                        tri_idx = p1.parse::<usize>().ok();

                    }

                }

            }



            let target_handle = handle_opt

                .or_else(|| self.tabs[i].scene.selected_entities().into_iter().find_map(|(_, e)| civil_surface::TinSurfaceData::from_entity(e).map(|s| s.handle)))

                .or_else(|| self.tabs[i].scene.document.entities().filter_map(civil_surface::TinSurfaceData::from_entity).map(|s| s.handle).next());



            if let Some(handle) = target_handle {

                if let Some(idx) = tri_idx {

                    match civil_surface::swap_edge_in_surface(&mut self.tabs[i].scene.document, handle, Some(idx)) {

                        Ok(msg) => {

                            self.tabs[i].scene.bump_geometry();

                            self.tabs[i].dirty = true;

                            self.command_line.record_recent(cmd);

                            self.command_line.push_output(&msg);

                            return Some(Task::none());

                        }

                        Err(err) => {

                            self.command_line.push_error(&format!("SURFACESWAPEDGE failed: {}", err));

                            return Some(Task::none());

                        }

                    }

                } else if let Some(ent) = self.tabs[i].scene.document.get_entity(handle).cloned() {

                    let swap_cmd = civil_surface::SurfaceSwapEdgeCommand::new(handle, ent);

                    self.command_line.push_info(&swap_cmd.prompt());

                    let opts = swap_cmd.options();

                    self.command_line.set_step_options(opts);

                    self.tabs[i].active_cmd = Some(Box::new(swap_cmd));

                    return Some(self.finish_dispatch(cmd));

                } else {

                    self.command_line.push_error("Surface entity not found.");

                    return Some(Task::none());

                }

            } else {

                self.command_line.push_error("No TIN surface found for SURFACESWAPEDGE.");

                return Some(Task::none());

            }

        }



        // ── Surface Operations: Raise / Lower Surface ──────────────────

        if upper_cmd == "SURFACERAISELOWER" || upper_cmd.starts_with("SURFACERAISELOWER ") || upper_cmd.starts_with("SURFACERAISELOWER:") {

            let rest = if upper_cmd.starts_with("SURFACERAISELOWER ") {

                trimmed_cmd[18..].trim()

            } else if upper_cmd.starts_with("SURFACERAISELOWER:") {

                trimmed_cmd[19..].trim()

            } else {

                ""

            };

            let parts: Vec<&str> = rest.split_whitespace().collect();

            let mut handle_opt = None;

            let mut delta_z = 1.0;



            if let Some(&p0) = parts.first() {

                let hex_str = p0.trim_start_matches("0x").trim_start_matches("0X");

                if let Ok(val) = u64::from_str_radix(hex_str, 16) {

                    handle_opt = Some(codec::Handle::new(val));

                    if let Some(p1) = parts.get(1) {

                        delta_z = p1.parse::<f64>().unwrap_or(1.0);

                    }

                } else if let Ok(val) = p0.parse::<f64>() {

                    delta_z = val;

                }

            }



            let target_handle = handle_opt

                .or_else(|| self.tabs[i].scene.selected_entities().into_iter().find_map(|(_, e)| civil_surface::TinSurfaceData::from_entity(e).map(|s| s.handle)))

                .or_else(|| self.tabs[i].scene.document.entities().filter_map(civil_surface::TinSurfaceData::from_entity).map(|s| s.handle).next());



            if let Some(handle) = target_handle {

                match civil_surface::raise_lower_surface(&mut self.tabs[i].scene.document, handle, delta_z) {

                    Ok(msg) => {

                        self.tabs[i].scene.bump_geometry();

                        self.tabs[i].dirty = true;

                        self.command_line.record_recent(cmd);

                        self.command_line.push_output(&msg);

                        return Some(Task::none());

                    }

                    Err(err) => {

                        self.command_line.push_error(&format!("SURFACERAISELOWER failed: {}", err));

                        return Some(Task::none());

                    }

                }

            } else {

                self.command_line.push_error("No TIN surface found for SURFACERAISELOWER.");

                return Some(Task::none());

            }

        }



        // ── Surface Operations: Delete Triangle ────────────────────────

        if upper_cmd == "SURFACEDELETETRI" || upper_cmd.starts_with("SURFACEDELETETRI ") || upper_cmd.starts_with("SURFACEDELETETRI:") {

            let rest = if upper_cmd.starts_with("SURFACEDELETETRI ") {

                trimmed_cmd[17..].trim()

            } else if upper_cmd.starts_with("SURFACEDELETETRI:") {

                trimmed_cmd[18..].trim()

            } else {

                ""

            };

            let parts: Vec<&str> = rest.split_whitespace().collect();

            let mut handle_opt = None;

            let mut tri_idx = None;



            if let Some(&p0) = parts.first() {

                let hex_str = p0.trim_start_matches("0x").trim_start_matches("0X");

                if let Ok(val) = u64::from_str_radix(hex_str, 16) {

                    handle_opt = Some(codec::Handle::new(val));

                    if let Some(p1) = parts.get(1) {

                        tri_idx = p1.parse::<usize>().ok();

                    }

                } else if let Ok(val) = p0.parse::<usize>() {

                    tri_idx = Some(val);

                }

            }



            let target_handle = handle_opt

                .or_else(|| self.tabs[i].scene.selected_entities().into_iter().find_map(|(_, e)| civil_surface::TinSurfaceData::from_entity(e).map(|s| s.handle)))

                .or_else(|| self.tabs[i].scene.document.entities().filter_map(civil_surface::TinSurfaceData::from_entity).map(|s| s.handle).next());



            if let Some(handle) = target_handle {

                if let Some(idx) = tri_idx {

                    match civil_surface::delete_triangle_in_surface(&mut self.tabs[i].scene.document, handle, Some(idx)) {

                        Ok(msg) => {

                            self.tabs[i].scene.bump_geometry();

                            self.tabs[i].dirty = true;

                            self.command_line.record_recent(cmd);

                            self.command_line.push_output(&msg);

                            return Some(Task::none());

                        }

                        Err(err) => {

                            self.command_line.push_error(&format!("SURFACEDELETETRI failed: {}", err));

                            return Some(Task::none());

                        }

                    }

                } else if let Some(ent) = self.tabs[i].scene.document.get_entity(handle).cloned() {

                    let del_cmd = civil_surface::SurfaceDeleteTriangleCommand::new(handle, ent);

                    self.command_line.push_info(&del_cmd.prompt());

                    let opts = del_cmd.options();

                    self.command_line.set_step_options(opts);

                    self.tabs[i].active_cmd = Some(Box::new(del_cmd));

                    return Some(self.finish_dispatch(cmd));

                } else {

                    self.command_line.push_error("Surface entity not found.");

                    return Some(Task::none());

                }

            } else {

                self.command_line.push_error("No TIN surface found for SURFACEDELETETRI.");

                return Some(Task::none());

            }

        }



        // ── Surface Operations: Surface Style & Visibility ─────────────

        if upper_cmd == "SURFACESTYLE" || upper_cmd.starts_with("SURFACESTYLE ") || upper_cmd.starts_with("SURFACESTYLE:") {

            let rest = if upper_cmd.starts_with("SURFACESTYLE ") {

                trimmed_cmd[13..].trim()

            } else if upper_cmd.starts_with("SURFACESTYLE:") {

                trimmed_cmd[14..].trim()

            } else {

                ""

            };

            let parts: Vec<&str> = rest.split_whitespace().collect();

            let mut handle_opt = None;

            let mut style = "CONTOURS";



            if let Some(&p0) = parts.first() {

                let hex_str = p0.trim_start_matches("0x").trim_start_matches("0X");

                if let Ok(val) = u64::from_str_radix(hex_str, 16) {

                    handle_opt = Some(codec::Handle::new(val));

                    if let Some(p1) = parts.get(1) {

                        style = p1;

                    }

                } else {

                    style = p0;

                }

            }



            let target_handle = handle_opt

                .or_else(|| self.tabs[i].scene.selected_entities().into_iter().find_map(|(_, e)| civil_surface::TinSurfaceData::from_entity(e).map(|s| s.handle)))

                .or_else(|| self.tabs[i].scene.document.entities().filter_map(civil_surface::TinSurfaceData::from_entity).map(|s| s.handle).next());



            if let Some(handle) = target_handle {

                match civil_surface::apply_surface_style(&mut self.tabs[i].scene.document, handle, style) {

                    Ok(msg) => {

                        self.tabs[i].scene.bump_geometry();

                        self.tabs[i].dirty = true;

                        self.command_line.record_recent(cmd);

                        self.command_line.push_output(&msg);

                        return Some(Task::none());

                    }

                    Err(err) => {

                        self.command_line.push_error(&format!("SURFACESTYLE failed: {}", err));

                        return Some(Task::none());

                    }

                }

            } else {

                self.command_line.push_error("No TIN surface found for SURFACESTYLE.");

                return Some(Task::none());

            }

        }
        None
    }

    /// First `"{prefix}{n}"` (n ≥ 1) not already used by a block record in the
    /// active drawing. Used to auto-name a paste-as-block definition.
    pub(super) fn unique_block_name(&self, prefix: &str) -> String {
        let i = self.active_tab;
        let mut n = 1;
        loop {
            let name = format!("{prefix}{n}");
            if self.tabs[i]
                .scene
                .document
                .block_records
                .get(&name)
                .is_none()
            {
                return name;
            }
            n += 1;
        }
    }

    /// Per-command start state every new command begins from — also used by
    /// entry points that install a command without going through dispatch
    /// (the block palette), so they don't inherit the previous command's last
    /// point, snaps or dynamic-input values. (#1525)
    pub(in crate::app) fn reset_command_start_state(&mut self, i: usize) {
        self.tabs[i].measurement.clear();
        // A command parked behind a transparent zoom / MTP goes with it.
        self.tabs[i].suspended_cmd = None;
        self.tabs[i].transparent_resume = false;
        // Starting any command leaves interactive navigation modes (their own
        // command arms below re-enable the selected one).
        self.tabs[i].pan_mode = false;
        self.tabs[i].orbit_mode = false;
        self.tabs[i].zoom_dynamic_mode = false;
        // Reset the last committed point so the first click of the new command
        // is not constrained by ortho/polar relative to a previous command's endpoint.
        self.last_point = None;
        // A new command collects its own points, so the previous command's
        // accepted snaps must not leak into it.
        self.clear_accepted_snaps();
        self.snapper.clear_override();
        // Starting a command restarts the right-click cycle, so its first
        // right-click acts as Enter rather than opening the context menu.
        self.tabs[i]
            .scene
            .selection
            .borrow_mut()
            .input
            .right_click_entered = false;
        // A fresh command starts at the polar/cartesian default — clear
        // any `,`-driven reshape and locked dynamic-input values from a
        // previous command. Otherwise a bare Enter on the first point prompt
        // can commit that stale coordinate instead of accepting the command's
        // default (LIMITS then compares an unintended lower-left point with
        // the displayed default upper-right).
        self.dyn_user_reshaped = false;
        self.dyn_coord_absolute = false;
        self.tabs[i].dyn_fields.clear();
        self.tabs[i].dyn_active = 0;
    }

    pub(super) fn dispatch_command(&mut self, cmd: &str) -> Task<Message> {
        let task = self.dispatch_command_inner(cmd, false);
        self.clear_idle_tool_highlight();
        task
    }

    /// Run a ribbon command. A `;` stands for Enter, as in a menu macro: each
    /// part answers the running command's prompt, or starts a command when
    /// none runs (`PDFIMPORT;FILE`).
    pub(super) fn dispatch_ribbon_command(&mut self, cmd: &str) -> Task<Message> {
        if !cmd.contains(';') {
            return self.dispatch_command(cmd);
        }
        let i = self.active_tab;
        let mut tasks = Vec::new();
        for part in cmd.split(';') {
            tasks.push(if self.tabs[i].active_cmd.is_some() {
                self.feed_active_cmd(part)
            } else {
                self.dispatch_command(part)
            });
        }
        Task::batch(tasks)
    }

    /// Turn the ribbon highlight off when the command just dispatched left
    /// nothing running: no interactive command, dialog or navigation mode.
    /// One-shot commands (view changes, clipboard, toggles, audits…) would
    /// otherwise leave it lit forever; interactive commands and dialog owners
    /// keep theirs until the command end / modal close clears it. (#355)
    pub(in crate::app) fn clear_idle_tool_highlight(&mut self) {
        let i = self.active_tab;
        if self.tabs[i].active_cmd.is_none()
            && self.active_modal.is_none()
            && !self.tabs[i].pan_mode
            && !self.tabs[i].orbit_mode
            && !self.tabs[i].zoom_dynamic_mode
        {
            self.ribbon.deactivate_tool();
        }
    }

    /// Dispatch a verb typed at the interactive command line, falling back to
    /// the closest autocomplete suggestion when the verb matches no command
    /// family. Lets a partial command run on Enter (`BAC` → `BACKGROUND`),
    /// the standard DWG command-line behavior. The fallback only fires for
    /// genuinely unknown verbs, so complete aliases that resolve through a
    /// dispatch family (`LT`, `ZO`, …) still run as typed. Programmatic
    /// callers (ribbon, plugins, headless automation) use `dispatch_command`
    /// and never get silent substitution.
    pub(super) fn dispatch_command_or_suggest(&mut self, cmd: &str) -> Task<Message> {
        let task = self.dispatch_command_inner(cmd, true);
        self.clear_idle_tool_highlight();
        task
    }

    fn dispatch_command_inner(&mut self, cmd: &str, allow_suggest: bool) -> Task<Message> {
        self.try_dispatch_command(cmd, allow_suggest)
            .unwrap_or_else(|| {
                self.command_line
                    .push_error(crate::tf!("Unknown command: {cmd}").as_ref());
                self.finish_dispatch(cmd)
            })
    }

    /// Dispatch `cmd`, or `None` when no command answers to it (nothing is
    /// reported then). A script line uses it to tell a command that took
    /// the words after it from one that did not.
    pub(in crate::app) fn try_dispatch_command(
        &mut self,
        cmd: &str,
        allow_suggest: bool,
    ) -> Option<Task<Message>> {
        let i = self.active_tab;
        // Expand a command alias ("L" → "LINE") on the leading token before any
        // routing, so every path below (Start-tab gate, plugins, all dispatch
        // families, the Repeat menu) sees the canonical command. Arguments after
        // the first space are left untouched. A non-alias passes through as-is.
        // The transparent prefix of commercial solutions: `'PAN` / `'ZOOM` run in the middle
        // of another command and hand control back to it afterwards.
        let (cmd, quoted) = match cmd.trim().strip_prefix('\'') {
            Some(rest) => (rest.trim(), true),
            None => (cmd, false),
        };
        let resolved = self.resolve_alias(cmd);
        let cmd = resolved.as_deref().unwrap_or(cmd);
        if let Some(task) = self.dispatch_civil3d_commands(cmd, i) {
            return Some(task);
        }
        if let Some(task) = self.dispatch_block_editor_vars(cmd, i) {
            return Some(task);
        }
        if cmd.trim_start_matches(['_', '.']) == "BGRIPSET" {
            return Some(self.start_block_grip_set());
        }
        let construction = cmd.trim_start_matches(['_', '.']);
        if construction == "BCONSTRUCTION" || construction.starts_with("BCONSTRUCTION ") {
            return Some(self.start_block_construction(construction.strip_prefix("BCONSTRUCTION").unwrap().trim()));
        }
        if cmd.trim_start_matches(['_', '.']) == "BCYCLEORDER" {
            return Some(self.open_block_cycle_order());
        }
        let verb = cmd
            .split([' ', ':'])
            .next()
            .unwrap_or(cmd)
            .trim_start_matches(['_', '.']);
        if self.block_editor_preferences.locked
            && matches!(verb, "BEDIT" | "BEDIT_BEGIN" | "BEDIT_OPEN" | "BEDIT_NEW")
        {
            self.command_line.push_error(
                crate::t!("The Block Editor is locked by BLOCKEDITLOCK.").as_ref(),
            );
            return Some(Task::none());
        }
        if cmd == "BACTIONSET" {
            if let Some(error) = self.action_set_error() {
                self.command_line.push_error(crate::t!(error).as_ref());
                return Some(Task::none());
            }
        }
        if cmd == "BASSOCIATE" {
            if let Some(error)=self.action_association_error() {
                self.command_line.push_error(crate::t!(error).as_ref());
                return Some(Task::none());
            }
        }
        if cmd == "BACTIONBAR" || cmd.starts_with("BACTIONBAR ") {
            if let Some(error)=self.action_bar_error() {
                self.command_line.push_error(crate::t!(error).as_ref());
                return Some(Task::none());
            }
        }
        if self.block_editor_preferences.action_bars && self.tabs[i].active_block_edit_session().is_some()
            && matches!(verb,"BACTIONSET"|"BASSOCIATE"|"BLOOKUPTABLE") {
            self.command_line.push_error(crate::t!("Use the action bar to edit this action, or set BACTIONBARMODE to 0.").as_ref());
            return Some(Task::none());
        }
        if matches!(
            cmd.split(' ').next(),
            Some("BACTION" | "BACTIONTOOL" | "BARRAY")
        ) {
            let args = if cmd == "BARRAY" || cmd.starts_with("BARRAY ") {
                "ARRAY"
            } else {
                cmd.split_once(' ').map_or("", |(_, a)| a.trim())
            };
            if !args.is_empty() && crate::scene::block_actions::Kind::parse(args).is_none() {
                self.command_line.push_error(
                    crate::t!(
                        "Supported actions: Move, Scale, Stretch, Polar Stretch, Rotate, Flip, Array. Other action types are not wired yet."
                    )
                    .as_ref(),
                );
                return Some(Task::none());
            }
            if let Some(error) = self.action_error(crate::scene::block_actions::Kind::parse(args)) {
                self.command_line.push_error(crate::t!(error).as_ref());
                return Some(Task::none());
            }
        }
        if cmd == "BPARAMETER" || cmd.starts_with("BPARAMETER ") {
            if let Some(error) =
                self.parameter_placement_error(cmd.strip_prefix("BPARAMETER").unwrap().trim())
            {
                self.command_line.push_error(crate::t!(error).as_ref());
                return Some(Task::none());
            }
        }
        if cmd == "BLOOKUPTABLE" || cmd.starts_with("BLOOKUPTABLE ") {
            if let Some(error) = self.lookup_table_error() {
                self.command_line.push_error(crate::t!(error).as_ref());
                return Some(Task::none());
            }
        }
        if cmd == "BTABLE" || cmd == "BPROPERTIESTABLE" || cmd.starts_with("BPROPERTIESTABLE ") {
            if let Some(error) = self.properties_table_error() {
                self.command_line.push_error(crate::t!(error).as_ref());
                return Some(Task::none());
            }
        }
        if matches!(cmd, "RESETBLOCK" | "RESETBLOCK APPLY")
            && self.tabs[i].active_block_edit_session().is_some()
        {
            self.command_line.push_error(
                crate::t!("RESETBLOCK is not available inside the Block Editor.").as_ref(),
            );
            return Some(Task::none());
        }
        if self.tabs[i].block_test.is_some() {
            let name = cmd.split([' ', ':']).next().unwrap_or(cmd);
            if matches!(
                name,
                "BEDIT"
                    | "BEDIT_BEGIN"
                    | "BEDIT_OPEN"
                    | "REFEDIT"
                    | "REFEDIT_BEGIN"
                    | "BSAVE"
                    | "BSAVEAS"
                    | "SAVE"
                    | "QSAVE"
                    | "SAVEAS"
                    | "SAVEALL"
                    | "WBLOCK"
                    | "EXPORT"
                    | "ETRANSMIT"
                    | "ARCHIVE"
            ) {
                self.command_line.push_error(
                    crate::t!("Close Test Block before saving or opening another block editor.")
                        .as_ref(),
                );
                return Some(Task::none());
            }
        }
        // V7 (reference check): inside a block editor session a set of
        // commands is refused outright. The hook sits after alias resolution
        // and before the transparent split and the running-command teardown,
        // so a rejected command leaves whatever is running untouched.
        if self.tabs[i].active_block_edit.is_some() {
            let name = cmd.split([' ', ':']).next().unwrap_or(cmd);
            if matches!(
                name,
                "LAYOUT"
                    | "-LAYOUT"
                    | "-PURGE"
                    | "-RENAME"
                    | "-PLOT"
                    | "VPORTS"
                    | "MVIEW"
                    | "WBLOCK"
                    | "PURGE"
                    | "RENAME"
                    | "REFEDIT"
                    | "REFEDIT_BEGIN"
                    | "PLOT"
            ) {
                self.command_line.push_error(
                    crate::tf!("{} command not allowed in block editor", name).as_ref(),
                );
                return Some(Task::none());
            }
        }
        if is_spacemouse_command(cmd) {
            return Some(self.run_action(cmd));
        }
        // A drafting aid only flips a flag, so it must not disturb whatever is
        // already running: pressing F8 partway through a LINE means "constrain
        // the rest of this line", not "abandon it". Everything below tears the
        // running command down, so a transparent one skips straight past it.
        // (#677) A quoted navigation command does the same; an interactive
        // one (ZOOM Window) parks the running command and resumes it when the
        // zoom ends, as does a ZOOM the zoom prompt itself hands off to.
        let transparent = is_transparent(cmd)
            || (quoted && is_transparent_capable(cmd))
            || (self.tabs[i].transparent_resume && cmd.starts_with("ZOOM"));
        if transparent {
            if zoom_prompts(cmd) && self.tabs[i].active_cmd.is_some() {
                self.tabs[i].suspended_cmd = self.tabs[i].active_cmd.take();
                self.tabs[i].transparent_resume = true;
                self.tabs[i].scene.clear_preview_wire();
            }
            let task = self.dispatch_families(cmd, i).unwrap_or_else(Task::none);
            // A one-shot zoom is over already — resume straight away.
            self.resume_transparent_parent(i);
            return Some(task);
        }
        // A walk is only a temporary hide, and these commands take ownership of
        // layer visibility from here on: starting one puts the drawing back
        // first, so the override never swallows the result it is hiding.
        if self.tabs[i].layer_walk.is_some() && Self::layer_walk_conflicts_with(cmd) {
            self.abandon_layer_walk_on(i);
        }
        // A new command abandons any grip edit and its numeric input.
        let had_pending_grip_input = self.grip_pending.take().is_some();
        let had_active_grip = self.cancel_active_grip_edit();
        if had_pending_grip_input || had_active_grip {
            self.command_line.input.clear();
        }
        // Starting a command closes any open ribbon dropdown (e.g. a style
        // combo left open) so it does not stay stuck behind the new tool.
        self.ribbon.close_dropdown();
        // Selection keywords last only for the round that asked for them: a
        // Remove or a fixed Window sense must not quietly still be in force
        // when the next command asks for objects. (#596)
        self.select_remove_mode = false;
        {
            let mut selection = self.tabs[i].scene.selection.borrow_mut();
            // Cancel the active selection gesture before the new command starts.
            selection.clear_left_selection_gesture();
        }
        // Cancel any running command before starting a new one.
        if self.tabs[i].active_cmd.is_some() {
            self.tabs[i].pending_parameter_set_follow_up = None;
            self.tabs[i].pending_xy_box_set = None;
            self.tabs[i].pending_action_pair = None;
            self.tabs[i].scene.clear_preview_wire();
            self.tabs[i].active_cmd = None;
            // Interrupting an ADDSELECTED draw with another command reverts its
            // template-property override too (#239).
            self.restore_add_selected_defaults();
        }
        self.reset_command_start_state(i);

        if let Some(path_str) = cmd.strip_prefix("OPEN_RECENT:") {
            let path = PathBuf::from(path_str);
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            return Some(Task::done(Message::OpenPathPicked(Some((path, size)))));
        }

        // The Start (welcome) tab has no drawing to act on, so a drawing
        // command would silently do nothing. Allow only the commands that
        // make sense there (create / open a document, or quit) and tell the
        // user otherwise instead of running a no-op. See #96.
        // Anything that acts on the application rather than a drawing belongs
        // here: document lifecycle, the links, and the app-wide configuration
        // editors (shortcuts, aliases) — none of them read the scene. This is
        // the single place that decides; `on_ribbon_tool_click` defers to it
        // rather than keeping a second, blunter copy (#388, #389).
        if self.tabs[i].is_start && !start_allowed(strip_command_prefixes(cmd).unwrap_or(cmd)) {
            self.command_line.push_info(
                crate::t!("No drawing open. Use NEW or OPEN to start a drawing.").as_ref(),
            );
            return Some(Task::none());
        }

        // Light the ribbon button of the command now starting, however it was
        // started (typed, alias, shortcut, Repeat), so the user sees where it
        // lives. A transparent command returned above and leaves the running
        // command's highlight alone.
        self.ribbon
            .show_command(strip_command_prefixes(cmd).unwrap_or(cmd));

        if !self.suppress_plugin_dispatch && crate::plugin::try_dispatch(self, i, cmd) {
            // try_dispatch returns true for both finished commands and interactive
            // commands that it just installed. If no command is now active, the
            // tool was a one-shot and we must turn the ribbon highlight off here —
            // normally apply_cmd_result does that, but plugin dispatch can return
            // without producing a CmdResult.
            self.tabs[i].last_cmd = Some(cmd.to_string());
            self.command_line.record_recent(cmd);
            if self.tabs[i].active_cmd.is_none() {
                self.ribbon.deactivate_tool();
            }
            return Some(self.finish_dispatch(cmd));
        }

        // Command families are dispatched in source order (see
        // `dispatch_families`); the first whose `match` arm matches handles it.
        if let Some(t) = self.dispatch_families(cmd, i) {
            // A command resolved — record it for the right-click Repeat menu so
            // commands from every source (command line, ribbon, context menu,
            // shortcuts) appear there, not only typed ones. Recorded after
            // resolution so a partial verb completed via the suggestion
            // fallback (`BAC`) stores the real command (`BACKGROUND`).
            self.command_line.record_recent(cmd);
            return Some(t);
        }

        // Scripts and typed input may carry the international (`_`) and
        // built-in (`.`) name prefixes (`_LINE`, `._LINE`). Ribbon tools
        // dispatch internal names that start with `_` themselves
        // (`_PCCROPPOLY`), so a name is tried as written first and loses its
        // prefixes only when nothing handles it.
        if let Some(bare) = strip_command_prefixes(cmd) {
            return self.try_dispatch_command(bare, allow_suggest);
        }

        // No family matched. From the interactive command line, run the
        // closest autocomplete suggestion instead of erroring, so a partial
        // command completes on Enter (`BAC` → `BACKGROUND`). The verb's own
        // input was already cleared, so rank against `cmd` directly.
        if allow_suggest {
            if let Some(top) = crate::ui::command_line::ranked_matches(
                cmd,
                &self.command_line.dynamic_commands,
                &self.command_line.command_aliases,
            )
            .into_iter()
            .next()
            {
                if !top.eq_ignore_ascii_case(cmd) {
                    return self.try_dispatch_command(&top, false);
                }
            }
        }
        None
    }

    /// Try each command family in source order, returning the first that
    /// handles `cmd`, or `None` when none match. Each family returns
    /// `Some(task)` for an arm it owns (early-returning or falling through to
    /// `finish_dispatch`), or `None` to defer to the next — equivalent to one
    /// sequential `match` over all arms.
    fn dispatch_families(&mut self, cmd: &str, i: usize) -> Option<Task<Message>> {
        if let Some(t) = self.dispatch_text_layer(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_dim_layer(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_dim_cmd(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_dim_inspect(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_mleader_layer(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_dash(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_ucsfollow(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_fileops(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_layers(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_xref_attach(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_import(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_model_import(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_bvisibility(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_block_cmdline(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_blocks(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_pdf_underlay(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_pc_colormap(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_pc_extract(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_attdef(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_ole(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_sheet_set(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_field(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_hyperlink(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_text_edit(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_spell(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_count(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_draw(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_dim(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_refedit(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_inquiry(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_view(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_layerprops(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_plotvars(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_blockvars(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_styleprops(cmd, i) {
            return Some(t);
        }
        if let Some(t) = self.dispatch_display(cmd, i) {
            return Some(t);
        }
        None
    }

    /// Shared tail run after a `dispatch_*` family handler whose matched arm
    /// did not early-return. Focuses the command line whenever a command just
    /// became active.
    fn finish_dispatch(&mut self, cmd: &str) -> Task<Message> {
        let i = self.active_tab;
        if self.tabs[i].active_cmd.is_some() {
            self.tabs[i].last_cmd = Some(cmd.to_string());
            self.sync_dyn_fields();
            self.focus_cmd_input()
        } else {
            Task::none()
        }
    }
}

/// Commands that toggle a drafting aid and nothing else.
///
/// They are reachable from a function key, and a function key gets pressed
/// mid-command — that is the point of it. Running them through the ordinary
/// path cancelled the active command, cleared its preview and dropped its base
/// point, so F8 during a MOVE both ended the move and made the dragged ghost
/// vanish. Nothing here starts a command, opens a document or reads geometry,
/// so there is nothing for the teardown to protect. (#677)
/// Commands that may run transparently (`'PAN`, `'ZOOM …`): the navigation
/// set, which never edits the drawing.
fn is_transparent_capable(cmd: &str) -> bool {
    cmd == "PAN"
        || cmd == "ZOOM"
        || cmd.starts_with("ZOOM ")
        || matches!(
            cmd,
            "ZW" | "ZE" | "ZA" | "ZP" | "ZI" | "ZO" | "ZD" | "ZEA" | "ZOBJ"
        )
}

/// ZOOM forms that install an interactive prompt (window corners, object
/// pick) and so need the running command parked while they last.
fn zoom_prompts(cmd: &str) -> bool {
    matches!(
        cmd,
        "ZOOM" | "ZOOM WINDOW" | "ZOOM W" | "ZW" | "ZOOM OBJECT" | "ZOOM O" | "ZOBJ"
    )
}

pub fn is_transparent(cmd: &str) -> bool {
    is_spacemouse_command(cmd)
        || matches!(
            cmd,
            "ORTHO"
                | "GRID"
                | "SNAP"
                | "POLAR"
                | "OSNAP"
                | "DSETTINGS"
                | "BAUTHORPALETTE"
                | "BAUTHORPALETTECLOSE"
                | "PARAMETERSCLOSE"
        )
}

fn is_spacemouse_command(cmd: &str) -> bool {
    matches!(
        cmd,
        "SPACEMOUSE"
            | "SPACEMOUSEPAUSE"
            | "SPACEMOUSEPAN"
            | "SPACEMOUSEPANZOOM"
            | "SPACEMOUSEAUTO"
            | "SPACEMOUSE3D"
    )
}

/// Whether `cmd` makes sense on the Start (welcome) tab — document lifecycle,
/// links, and app-wide configuration; nothing that reads the scene. Single
/// source of truth: the dispatch gate refuses everything else, and the ribbon
/// dims the tools this rejects.
pub fn start_allowed(cmd: &str) -> bool {
    // A batch script starts from no drawing: it sets FILEDIA / SDI and
    // opens the drawings it works on by path, and EXIT answers its save
    // questions from any tab.
    let verb = cmd.split_whitespace().next().unwrap_or("");
    is_spacemouse_command(cmd)
        || matches!(verb, "OPEN" | "SCRIPT" | "SCR" | "FILEDIA" | "SDI" | "EXIT")
        || matches!(
            cmd,
            "NEW"
            | "QNEW"
            | "OPEN"
            | "EXIT"
            | "QUIT"
            | "REPORT"
            | "CHANGELOG"
            | "ABOUT"
            | "PLUGINS"
            | "PLUGINMANAGER"
            | "DONATE"
            | "WEBVERSION"
            | "HELP"
            | "PERF"
            | "CUI"
            | "ALIASEDIT"
            | "CUILOAD"
            | "CUIIMPORT"
            // The start page already offers Options as a button, so the
            // command that opens the same dialog belongs here too.
            | "OPTIONS"
            | "OP"
        )
}

// ── Autocomplete registry — one-shot commands ──────────────────────────────
// These commands dispatch a single action (file ops, view, layer/style
// managers, undo/redo, …) rather than installing an interactive `CadCommand`,
// so they have no module of their own to register from. They are surfaced for
// command-line autocomplete here. Internal dispatch tokens that the user never
// types (REFEDIT_BEGIN, REFCLOSE_SAVE, REFCLOSE_DISCARD) are intentionally
// excluded.
inventory::submit!(crate::command::CommandRegistration {
    names: &[
        // Drafting-aid / display toggles + customization entry points wired in the
        // dispatch families (no interactive command module of their own).
        "ALIASEDIT",
        // Implemented command-line block forms must also validate in ALIASEDIT.
        "-BLOCK",
        "-BMAKE",
        "-WBLOCK",
        "-WB",
        "-ATTEDIT",
        // Script forms of the table commands.
        "-LAYER",
        "-PURGE",
        "-OVERKILL",
        "-STYLE",
        "-RENAME",
        "-DIMSTYLE",
        "-LAYOUT",
        "-PLOT",
        "CLEANSCREEN",
        "CUI",
        "DSETTINGS",
        "PARAMETERS",
        "PARAMETERSCLOSE",
        "BTABLE",
        "GRID",
        "GRAPHICATTRIBUTES",
        "ISODRAFT",
        "ISOPLANE",
        "OSNAP",
        "POLAR",
        "QUICKPROPERTIES",
        "SNAP",
        "OPTIONS",
        "OP",
        "SYNCPVIEWPORTS",
        "VPSYNC",
        // Viewport-arrangement shortcuts (delegate to VPORTS configurations).
        "CASCADE",
        "HORIZONTAL",
        "VERTICAL",
        "VPJOIN",
        // Standard aliases for existing commands.
        "BMAKE",
        "EXPORTPDF",
        "EXPORTSVG",
        "PRINTALL",
        "PUBLISH",
        "DDIM",
        // Inquiry: list the whole drawing database.
        "DBLIST",
        // Layer management.
        "LAYDEL",
        "LAYTRANS",
        "DWGUNITS",
        "LAYMRG",
        "LAYERSTATE",
        "LAS",
        "LMAN",
        // 3D move (same operation as MOVE, which already works in 3D).
        "3DMOVE",
        // Quick leader (same as LEADER).
        "QLEADER",
        "QL",
        // Restore the last-erased objects.
        "OOPS",
        // Plan (top) view.
        "PICKADD",
        "PICKDRAG",
        "PLAN",
        // Current object colour (CECOLOR).
        "COLOR",
        "COLOUR",
        "CECOLOR",
        "CETRANSPARENCY",
        "DDCOLOR",
        "BYLAYER",
        // Synchronise block attributes.
        "ATTSYNC",
        // Drawing units picker.
        "UNITS",
        "UN",
        "DDUNITS",
        // Visual styles (mapped to the wireframe / shaded view).
        "VSCURRENT",
        "SHADEMODE",
        // Raster image brightness / contrast / fade.
        "ADJUST",
        // Block list + block-attribute list (command-line forms).
        "BLOCKPALETTE",
        "BLOCKSPALETTE",
        "BLOCKSPALETTECLOSE",
        "-INSERT",
        "BLOCKMRULIST",
        "BLOCKREDEFINEMODE",
        "BLOCKNAVIGATE",
        "BLOCKSTATE",
        "EXPLMODE",
        "INSNAME",
        "ATTMAN",
        "BATTMAN",
        "-BEDIT",
        "INSBASE",
        // Drawing-content overview.
        "ADCENTER",
        "CONTENTBROWSER",
        "ADC",
        // Annotation scale.
        "ANNOSCALE",
        "CANNOSCALE",
        "ANNOALLVISIBLE",
        "ANNOAUTOSCALE",
        "ANNOUPDATE",
        "SCALELISTEDIT",
        "OBJECTSCALE",
        "ANNORESET",
        // Import CSV into a table + LandXML survey points.
        "DATALINK",
        "DATALINKUPDATE",
        "LANDXMLIMPORT",
        // Keyboard-shortcut (CUI) export / import.
        "CUIEXPORT",
        "CUIIMPORT",
        "CUILOAD",
        // Save every open drawing.
        "SAVEALL",
        // Draw-order: all text/dims to front or back; hatches to back.
        "TEXTTOFRONT",
        "TEXTTOBACK",
        "HATCHTOBACK",
        "HB",
        // Criteria-based selection (same as QSELECT) + copy with picked base.
        "FILTER",
        "FI",
        "COPYBASE",
        // Change justification of selected text/mtext.
        "JUSTIFYTEXT",
        // Command-line arithmetic calculator.
        "CAL",
        // Text tools: case, mask, width fit, sequential numbering, arc layout.
        "TCASE",
        "TEXTMASK",
        "TEXTFIT",
        "TEXTFILL",
        "TCOUNT",
        "ARCTEXT",
        // Jogged radius dimension.
        "DIMJOGGED",
        "DJO",
        "DIMJOG",
        // Mark objects annotative.
        "OBJECTSCALE",
        // Copy nested objects out of a block.
        "NCOPY",
        "NCOPYALL",
        // Close tab, hidden-line / visual styles, calculator, hyperlink.
        "CLOSE",
        "HIDE",
        "HI",
        "VISUALSTYLES",
        "QUICKCALC",
        "QC",
        "HYPERLINK",
        "XOPEN",
        "REGION",
        "REG",
        // Redraw / regenerate the display caches.
        "REDRAW",
        "REDRAWALL",
        "REGEN",
        "REGENALL",
        "ARCHIVE",
        "ETRANSMIT",
        "FLATSHOT",
        "CONVTOSURFACE",
        // Slice/section + interference + press-pull/thicken + 3D transforms + wall/pyramid.
        "SLICE",
        "SL",
        "INTERFERE",
        "INF",
        "SECTIONCULL",
        "PRESSPULL",
        "THICKEN",
        "3DROTATE",
        "ROTATE3D",
        "POLYSOLID",
        "3DMIRROR",
        "MIRROR3D",
        "3DALIGN",
        "ALIGN3D",
        "SECTION",
        "SECTIONPLANE",
        "PYRAMID",
        "PYR",
        "SPLINEFIT",
        "FITSPLINE",
        // System variables (typeable directly).
        "MIRRTEXT",
        "PERSPECTIVE",
        "GRIDMODE",
        "GRIDUNIT",
        "QPMODE",
        "ZOOMWHEEL",
        "ZOOMFACTOR",
        "SHORTCUTMENU",
        "SHORTCUTMENUDURATION",
        "CURSORSIZE",
        "PICKBOX",
        "APERTURE",
        "CURSORTYPE",
        "SNAPANG",
        "ATTREQ",
        "ATTDIA",
        "DIMASSOC",
        "ANGBASE",
        "ANGDIR",
        "REGENMODE",
        "BLIPMODE",
        "SPLFRAME",
        "CAMERADISPLAY",
        "DELOBJ",
        "PLINEGEN",
        "PSLTSCALE",
        // Plot preferences and the current-layout variables (commands/plotvars.rs).
        "PLOTOFFSET",
        "PAPERUPDATE",
        "PLOTROTMODE",
        "PLOTTRANSPARENCYOVERRIDE",
        "BACKGROUNDPLOT",
        "CTAB",
        "TILEMODE",
        "PSETUPIN",
        "-PSETUPIN",
        "DISPSILH",
        "WORLDVIEW",
        "LIMCHECK",
        "DRAGMODE",
        "LUNITS",
        "LUPREC",
        "AUNITS",
        "AUPREC",
        "THICKNESS",
        "ELEVATION",
        "INSUNITS",
        "SPLINETYPE",
        "ISOLINES",
        "DIMASO",
        "DIMSHO",
        "QTEXTMODE",
        "PLIMCHECK",
        "VISRETAIN",
        "USRTIMER",
        "ATTMODE",
        "COORDS",
        "OSMODE",
        "PICKSTYLE",
        "SPLINESEGS",
        "SURFU",
        "SURFV",
        "SURFTYPE",
        "SHADEDGE",
        "MAXACTVP",
        "CMLJUST",
        "CMLSCALE",
        "CMLSTYLE",
        "TEXTQLTY",
        "SORTENTS",
        "FRAME",
        "IMAGEFRAME",
        "PDFFRAME",
        "DWFFRAME",
        "DGNFRAME",
        "PDFOSNAP",
        "UOSNAP",
        "FIELDDISPLAY",
        "FILEDIA",
        "SDI",
        "TRIMEXTENDMODE",
        "PELLIPSE",
        "SELECTIONCYCLING",
        "UCSDETECT",
        "SURFACEASSOCIATIVITY",
        "DGNIMPORTMODE",
        "LAYOUTCREATEVIEWPORT",
        "TXT2MTXTCOMBINEMTEXT",
        "TXT2MTXTTEXTORDER",
        "TXT2MTXTWORDWRAP",
        "TXT2MTXTLINESPACING",
        "FIELDEVAL",
        "PDFIMPORTMODE",
        "PDFIMPORTFILTER",
        "PDFIMPORTLAYERS",
        "PDFIMPORTIMAGEPATH",
        "PDFSHXLAYER",
        "PDFSHXBESTFONT",
        "PDFSHXTEXTSETTINGS",
        "XDWGFADECTL",
        "POINTCLOUDCLIPFRAME",
        "POINTCLOUDDENSITY",
        "POINTCLOUDPOINTSIZE",
        "POINTCLOUDLOCK",
        "POINTCLOUDAUTOUPDATE",
        "POINTCLOUDBOUNDARY",
        "POINTCLOUDRTDENSITY",
        "POINTCLOUDLOD",
        "POINTCLOUDPOINTMAX",
        "POINTCLOUDVISRETAIN",
        "POINTCLOUDSHADING",
        "POINTCLOUDCACHESIZE",
        "POINTCLOUD2DVSDISPLAY",
        "POINTCLOUDLIGHTING",
        "POINTCLOUDLIGHTSOURCE",
        "POINTCLOUDPOINTMAXLEGACY",
        "XCLIPFRAME",
        "WIPEOUTFRAME",
        "ATTMODE0",
        "ATTMODE1",
        "ATTMODE2",
        "FRAMES0",
        "FRAMES1",
        "FRAMES3",
        "FRAMES2",
        "UOSNAP0",
        "UOSNAP1",
        "HALOGAP",
        "TRACEWID",
        "SKETCHINC",
        "SKPOLY",
        "SKTOLERANCE",
        "CLIPROMPTLINES",
        "COMMANDLINEFADETIME",
        "COMMANDLINEOPACITY",
        "MENUBAR",
        "RIBBON",
        "RIBBONCLOSE",
        "COLORTHEME",
        "SELECTIONAREA",
        "SELECTIONAREAOPACITY",
        "SELECTIONEFFECT",
        "SELECTIONEFFECTCOLOR",
        "WINDOWSAREACOLOR",
        "WINDOWAREACOLOR",
        "CROSSINGAREACOLOR",
        "SELECTIONPREVIEW",
        "GRIPSIZE",
        "GRIPCOLOR",
        "GRIPHOT",
        "GRIPHOVER",
        "GRIPOBJLIMIT",
        // Dispatched all along, but absent from the registry, so command-line
        // completion never offered them.
        "ISAVEBAK",
        "SAVETIME",
        "FILEASSOC",
        // Reset selected entities' overrides to follow their layer.
        "SETBYLAYER",
        // Remove duplicate objects; set drawing base point; audit integrity;
        // read/write system variables.
        "OVERKILL",
        "BASE",
        "AUDIT",
        "SETVAR",
        "SCRIPT",
        "SCR",
        "FINDNONPURGEABLE",
        "3DORBIT",
        "3O",
        "ABOUT",
        "ATTACH",
        "ATTDISP",
        "ATTEXT",
        "BACKGROUND",
        "CDIMSTY",
        "CELTSCALE",
        "CHANGELOG",
        "CHPROP",
        "CLAYER",
        "CLEAR",
        "CLR",
        "COLORSCHEME",
        "COUNT",
        "DATAEXTRACTION",
        "-DATAEXTRACTION",
        "DE",
        "DESELALL",
        "DESELECT",
        "DIMSTYLE",
        "DONATE",
        "DRAWORDER",
        "DRAWORDER_FRONT",
        "DRAWORDER_BACK",
        "DRAWORDER_ABOVE",
        "DRAWORDER_UNDER",
        "DWGPROP",
        "DWGPROPS",
        "EATTEXT",
        "EXIT",
        "EXPORT",
        "EXPORTSTEP",
        "EXPORTSTL",
        "EXTERNALREFERENCES",
        "EXTERNALREFERENCESCLOSE",
        "ERSTATE",
        "POINTCLOUDMANAGER",
        "POINTCLOUDMANAGERCLOSE",
        "EXTRIM",
        "FILETAB",
        "FIND",
        "FLATTEN",
        "HELP",
        "HIDEOBJECTS",
        "IM",
        "IMAGE",
        "IMAGEATTACH",
        "TRANSPARENCY",
        "IMAGEEMBED",
        "IMPORTOBJ",
        "ISOLATEOBJECTS",
        "LA",
        "LAYER",
        "LAYERS",
        "LAYISO",
        "LAYON",
        "LAYOUTMANAGER",
        "LAYOUTPANEL",
        "LAYOUTTAB",
        "LAYTHW",
        "LAYUNISO",
        "LAYWALK",
        "LI",
        "LINETYPE",
        "LIST",
        "LTSCALE",
        "LWDISPLAY",
        "MASSPROP",
        "MLEADERSTYLE",
        "MLSTYLE",
        "MS",
        "MSPACE",
        "CHSPACE",
        "NAVVCUBE",
        "NEW",
        "QNEW",
        "OBJIMPORT",
        "IMPORT",
        "ACISIN",
        "WMFIN",
        "DGNIMPORT",
        "-DGNIMPORT",
        "OPEN",
        "ORTHO",
        "P",
        "PAN",
        "PAGESETUP",
        "PERF",
        "PERSP",
        "PLOT",
        "PRINTERS",
        "PLOTSTYLE",
        "PLOTSTYLEEDITOR",
        "PLOTSTYLEPANEL",
        "PR",
        "PRINT",
        "PROPERTIES",
        "PROPS",
        "PSPACE",
        "PURGE",
        "QP",
        "QUICKPRINT",
        "QS",
        "QSAVE",
        "QSELECT",
        "QUIT",
        "REDO",
        "RENAME",
        "REPORT",
        "SA",
        "SAVE",
        "SAVEAS",
        "SCALETEXT",
        "SELECTALL",
        "SELECTSIMILAR",
        "SELSIM",
        // Draw a new object of the same type as the selected one. (#239)
        "ADDSELECTED",
        // Outline of origin planes, open sketch and solid bodies.
        "BROWSER",
        // Open and close a sketch: a drawing plane, a square-on view and
        // snapping, held together until the profile is done.
        "CREATESKETCH",
        "FINISHSKETCH",
        "SHEETSET",
        "SHEETSETHIDE",
        "NEWSHEETSET",
        "OPENSHEETSET",
        "SSMAUTOOPEN",
        "SSLOCATE",
        "SSMPOLLTIME",
        "SSMSHEETSTATUS",
        "SSMSTATE",
        "SSFOUND",
        "SHORTCUTS",
        "SSM",
        "STEPOUT",
        "FBXEXPORT",
        "STLOUT",
        "STPOUT",
        "STYLE",
        "STYLESMANAGER",
        "TABLESTYLE",
        "BAUTHORPALETTE",
        "BAUTHORPALETTECLOSE",
        "TOOLPALETTES",
        "TP",
        "TS",
        "U",
        "UCS",
        "UCSICON",
        "UNDERLAY",
        "UNDO",
        "UNISOLATEOBJECTS",
        "USERI",
        "USERR",
        "VIEW",
        "-VIEW",
        "VPORTS",
        "VS",
        "VW",
        "WB",
        "WBLOCK",
        "WEBVERSION",
        "XA",
        "XATTACH",
        "XDATA",
        "XR",
        "XREF",
        "-XREF",
        "-ATTACH",
        "XRELOAD",
        "ZOOM",
        "ZS",
    ]
});

#[cfg(test)]
mod marquee_cancel_tests {
    use crate::app::{GripPendingValue, Message, OpenCADStudio};
    use crate::scene::model::object::GripMenuAction;
    use crate::scene::pick::grip::GripEdit;
    use codec::Handle;
    use iced::time::Instant;

    fn fresh() -> OpenCADStudio {
        let mut app = OpenCADStudio::new_for_test();
        app.automation_op(r#"{"op":"new"}"#);
        app
    }

    /// Arm a held box drag.
    fn arm_marquee(app: &mut OpenCADStudio) {
        let i = app.active_tab;
        let mut sel = app.tabs[i].scene.selection.borrow_mut();
        sel.input.left_down = true;
        sel.input.left_press_pos = Some(iced::Point::new(10.0, 10.0));
        sel.input.left_press_time = Some(Instant::now());
        sel.input.left_dragging = true;
        sel.gesture.box_anchor = Some(iced::Point::new(10.0, 10.0));
        sel.gesture.box_anchor_world = Some(glam::DVec3::new(1.0, 2.0, 0.0));
        sel.gesture.box_current = Some(iced::Point::new(40.0, 40.0));
        sel.gesture.box_crossing = true;
        sel.gesture.box_crossing_locked = true;
    }

    /// Arm a held lasso drag.
    fn arm_lasso(app: &mut OpenCADStudio) {
        let i = app.active_tab;
        let mut sel = app.tabs[i].scene.selection.borrow_mut();
        sel.input.left_down = true;
        sel.input.left_press_pos = Some(iced::Point::new(10.0, 10.0));
        sel.input.left_press_time = Some(Instant::now());
        sel.input.left_dragging = true;
        sel.gesture.poly_active = true;
        sel.gesture.poly_points = vec![iced::Point::new(10.0, 10.0), iced::Point::new(20.0, 30.0)];
        sel.gesture.poly_crossing = true;
    }

    #[test]
    fn typing_a_command_cancels_a_half_drawn_marquee() {
        let mut app = fresh();
        arm_marquee(&mut app);
        let i = app.active_tab;

        let _ = app.dispatch_command("LINE");

        let sel = app.tabs[i].scene.selection.borrow();
        assert!(!sel.input.left_down);
        assert!(sel.input.left_press_pos.is_none());
        assert!(sel.input.left_press_time.is_none());
        assert!(!sel.input.left_dragging);
        assert!(sel.gesture.box_anchor.is_none());
        assert!(sel.gesture.box_anchor_world.is_none());
        assert!(sel.gesture.box_current.is_none());
        assert!(!sel.gesture.box_crossing);
        assert!(!sel.gesture.box_crossing_locked);
    }

    #[test]
    fn typing_a_command_cancels_a_held_lasso() {
        let mut app = fresh();
        arm_lasso(&mut app);
        let i = app.active_tab;

        let _ = app.dispatch_command("LINE");

        let sel = app.tabs[i].scene.selection.borrow();
        assert!(!sel.input.left_down);
        assert!(sel.input.left_press_pos.is_none());
        assert!(sel.input.left_press_time.is_none());
        assert!(!sel.input.left_dragging);
        assert!(!sel.gesture.poly_active);
        assert!(sel.gesture.poly_points.is_empty());
        assert!(!sel.gesture.poly_crossing);
    }

    #[test]
    fn an_aliased_command_cancels_it_as_well() {
        let mut app = fresh();
        arm_marquee(&mut app);
        let i = app.active_tab;

        let _ = app.dispatch_command("L");

        assert_eq!(
            app.tabs[i].active_cmd.as_deref().map(|cmd| cmd.name()),
            Some("LINE")
        );
        let sel = app.tabs[i].scene.selection.borrow();
        assert!(sel.gesture.box_anchor.is_none());
        assert!(sel.gesture.box_anchor_world.is_none());
    }

    #[test]
    fn a_transparent_command_leaves_the_marquee_alone() {
        let mut app = fresh();
        arm_marquee(&mut app);
        let i = app.active_tab;

        let _ = app.dispatch_command("ORTHO");

        let sel = app.tabs[i].scene.selection.borrow();
        assert!(sel.input.left_down);
        assert!(sel.input.left_press_pos.is_some());
        assert!(sel.input.left_press_time.is_some());
        assert!(sel.input.left_dragging);
        assert!(sel.gesture.box_anchor.is_some());
        assert!(sel.gesture.box_anchor_world.is_some());
        assert!(sel.gesture.box_current.is_some());
        assert!(sel.gesture.box_crossing);
        assert!(sel.gesture.box_crossing_locked);
    }

    #[test]
    fn starting_a_command_cancels_a_grip_value_edit() {
        let mut app = fresh();
        let i = app.active_tab;
        let handle = Handle::new(42);
        app.tabs[i].active_grip =
            Some(GripEdit::radius(handle, 1, glam::DVec3::new(2.0, 0.0, 0.0)));
        app.grip_pending = Some(GripPendingValue {
            handle,
            grip_id: 1,
            action: GripMenuAction::Radius,
            label: "New radius",
        });
        app.command_line.input = "5".to_string();

        let _ = app.dispatch_command("LINE");

        assert!(app.tabs[i].active_grip.is_none());
        assert!(app.grip_pending.is_none());
        assert!(app.command_line.input.is_empty());
        assert_eq!(
            app.tabs[i].active_cmd.as_deref().map(|cmd| cmd.name()),
            Some("LINE")
        );
    }

    #[test]
    fn command_finalize_repeats_last_cmd() {
        let mut app = fresh();
        let i = app.active_tab;
        assert_eq!(app.tabs[i].last_cmd, None);

        // Run LINE command
        let _ = app.dispatch_command("LINE");
        assert_eq!(app.tabs[i].last_cmd.as_deref(), Some("LINE"));

        // Cancel LINE with Escape
        let _ = app.update(Message::CommandEscape);
        assert!(app.tabs[i].active_cmd.is_none());
        assert_eq!(app.tabs[i].last_cmd.as_deref(), Some("LINE"));

        // Press Enter (CommandFinalize) on empty command line repeats LINE
        let _ = app.update(Message::CommandFinalize);
        assert_eq!(
            app.tabs[i].active_cmd.as_deref().map(|cmd| cmd.name()),
            Some("LINE")
        );
    }
}

/// `cmd` without its leading `_` / `.` name prefixes (`_LINE`, `.LINE`,
/// `._LINE`), or `None` when it carries none.
fn strip_command_prefixes(cmd: &str) -> Option<&str> {
    let bare = cmd.trim_start_matches(['_', '.']);
    (bare.len() != cmd.len() && !bare.is_empty()).then_some(bare)
}

#[cfg(test)]
mod ribbon_highlight_tests {
    use crate::app::OpenCADStudio;

    fn fresh() -> OpenCADStudio {
        let mut app = OpenCADStudio::new_for_test();
        app.automation_op(r#"{"op":"new"}"#);
        app
    }

    #[test]
    fn a_typed_alias_lights_the_button_of_the_command_it_starts() {
        let mut app = fresh();
        let _ = app.dispatch_command_or_suggest("O");
        assert!(app.tabs[app.active_tab].active_cmd.is_some());
        assert_eq!(app.ribbon.active_tool(), Some("OFFSET"));
        let _ = app.dispatch_command_or_suggest("L");
        assert_eq!(app.ribbon.active_tool(), Some("LINE"));
    }

    #[test]
    fn a_transparent_command_keeps_the_running_command_lit() {
        let mut app = fresh();
        let _ = app.dispatch_command_or_suggest("LINE");
        let _ = app.dispatch_command_or_suggest("'ZOOM E");
        assert!(app.tabs[app.active_tab].active_cmd.is_some());
        assert_eq!(app.ribbon.active_tool(), Some("LINE"));
    }

    #[test]
    fn a_one_shot_command_leaves_nothing_lit() {
        // REGEN cancels the running LINE and finishes at once, so neither
        // its own button nor LINE's may stay lit.
        let mut app = fresh();
        let _ = app.dispatch_command_or_suggest("LINE");
        let _ = app.dispatch_command_or_suggest("REGEN");
        assert!(app.tabs[app.active_tab].active_cmd.is_none());
        assert_eq!(app.ribbon.active_tool(), None);
    }
}
