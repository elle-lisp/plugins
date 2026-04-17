//! UiNode tree, Elle<->Rust conversion, egui rendering, and interaction tracking.

use elle_plugin::{ElleResult, ElleValue};
use std::collections::{HashMap, HashSet};

use crate::api;

// ── UiNode ──────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum UiNode {
    Label { text: String },
    Heading { text: String },
    ProgressBar { fraction: f64, text: Option<String> },
    Separator,
    Spacer { size: f32 },
    Button { id: String, text: String },
    TextInput { id: String, hint: Option<String> },
    TextEdit { id: String, desired_rows: usize },
    Checkbox { id: String, text: String },
    Slider { id: String, min: f64, max: f64 },
    ComboBox { id: String, options: Vec<String> },
    VLayout { children: Vec<UiNode> },
    HLayout { children: Vec<UiNode> },
    Centered { children: Vec<UiNode> },
    CenteredJustified { children: Vec<UiNode> },
    ScrollArea { id: String, children: Vec<UiNode> },
    Collapsing { id: String, title: String, children: Vec<UiNode> },
    Group { children: Vec<UiNode> },
    Grid { id: String, columns: usize, children: Vec<UiNode> },
}

// ── Interactions ─────────────────────────────────────────────────────

#[derive(Default)]
pub struct Interactions {
    pub clicked: HashSet<String>,
    pub text_values: HashMap<String, String>,
    pub check_values: HashMap<String, bool>,
    pub slider_values: HashMap<String, f64>,
    pub combo_values: HashMap<String, String>,
    pub collapsed: HashMap<String, bool>,
    pub closed: bool,
    pub width: f32,
    pub height: f32,
}

// ── Value -> UiNode conversion ────────────────────────────────────────

fn get_prop_str(props: ElleValue, key: &str) -> Option<String> {
    let a = api();
    let v = a.get_struct_field(props, key);
    a.get_string(v).map(|s| s.to_string())
}

fn get_prop_keyword(props: ElleValue, key: &str) -> Option<String> {
    let a = api();
    let v = a.get_struct_field(props, key);
    a.get_keyword_name(v).map(|s| s.to_string())
}

fn get_prop_id(props: ElleValue) -> Option<String> {
    get_prop_keyword(props, "id").or_else(|| get_prop_str(props, "id"))
}

fn get_prop_float(props: ElleValue, key: &str) -> Option<f64> {
    let a = api();
    let v = a.get_struct_field(props, key);
    a.get_float(v).or_else(|| a.get_int(v).map(|i| i as f64))
}

fn get_prop_int(props: ElleValue, key: &str) -> Option<i64> {
    let a = api();
    let v = a.get_struct_field(props, key);
    a.get_int(v)
}

pub fn value_to_node(val: ElleValue) -> Result<UiNode, String> {
    let a = api();
    let len = a.get_array_len(val)
        .ok_or_else(|| format!("ui node must be an array, got {}", a.type_name(val)))?;
    if len == 0 { return Err("ui node array must not be empty".into()); }

    let first = a.get_array_item(val, 0);
    let tag = a.get_keyword_name(first)
        .ok_or("first element of ui node must be a keyword")?
        .to_string();

    // Check if second element is a props struct
    let (props, rest_start) = if len > 1 {
        let second = a.get_array_item(val, 1);
        if a.check_struct(second) {
            (Some(second), 2)
        } else {
            (None, 1)
        }
    } else {
        (None, 1)
    };

    // Helper to get props or nil
    let props_v = props.unwrap_or_else(|| a.nil());

    // Collect rest elements
    let rest_count = len - rest_start;
    let rest_item = |i: usize| a.get_array_item(val, rest_start + i);

    match tag.as_str() {
        "label" => {
            let text = if rest_count > 0 { a.get_string(rest_item(0)).unwrap_or("").to_string() } else { String::new() };
            Ok(UiNode::Label { text })
        }
        "heading" => {
            let text = if rest_count > 0 { a.get_string(rest_item(0)).unwrap_or("").to_string() } else { String::new() };
            Ok(UiNode::Heading { text })
        }
        "progress-bar" => {
            let fraction = get_prop_float(props_v, "fraction").unwrap_or(0.0);
            let text = get_prop_str(props_v, "text");
            Ok(UiNode::ProgressBar { fraction, text })
        }
        "separator" => Ok(UiNode::Separator),
        "spacer" => { let size = get_prop_float(props_v, "size").unwrap_or(8.0) as f32; Ok(UiNode::Spacer { size }) }
        "button" => {
            let id = get_prop_id(props_v).ok_or("button requires :id")?;
            let text = if rest_count > 0 { a.get_string(rest_item(0)).unwrap_or("").to_string() } else { String::new() };
            Ok(UiNode::Button { id, text })
        }
        "text-input" => {
            let id = get_prop_id(props_v).ok_or("text-input requires :id")?;
            let hint = get_prop_str(props_v, "hint");
            Ok(UiNode::TextInput { id, hint })
        }
        "text-edit" => {
            let id = get_prop_id(props_v).ok_or("text-edit requires :id")?;
            let desired_rows = get_prop_int(props_v, "rows").unwrap_or(4) as usize;
            Ok(UiNode::TextEdit { id, desired_rows })
        }
        "checkbox" => {
            let id = get_prop_id(props_v).ok_or("checkbox requires :id")?;
            let text = if rest_count > 0 { a.get_string(rest_item(0)).unwrap_or("").to_string() } else { String::new() };
            Ok(UiNode::Checkbox { id, text })
        }
        "slider" => {
            let id = get_prop_id(props_v).ok_or("slider requires :id")?;
            let min = get_prop_float(props_v, "min").unwrap_or(0.0);
            let max = get_prop_float(props_v, "max").unwrap_or(100.0);
            Ok(UiNode::Slider { id, min, max })
        }
        "combo-box" => {
            let id = get_prop_id(props_v).ok_or("combo-box requires :id")?;
            let options: Vec<String> = if rest_count > 0 {
                let arr_v = rest_item(0);
                let arr_len = a.get_array_len(arr_v).unwrap_or(0);
                (0..arr_len).filter_map(|i| a.get_string(a.get_array_item(arr_v, i)).map(|s| s.to_string())).collect()
            } else { Vec::new() };
            Ok(UiNode::ComboBox { id, options })
        }
        "v-layout" => { let children: Result<Vec<_>, _> = (0..rest_count).map(|i| value_to_node(rest_item(i))).collect(); Ok(UiNode::VLayout { children: children? }) }
        "h-layout" => { let children: Result<Vec<_>, _> = (0..rest_count).map(|i| value_to_node(rest_item(i))).collect(); Ok(UiNode::HLayout { children: children? }) }
        "centered" => { let children: Result<Vec<_>, _> = (0..rest_count).map(|i| value_to_node(rest_item(i))).collect(); Ok(UiNode::Centered { children: children? }) }
        "centered-justified" => { let children: Result<Vec<_>, _> = (0..rest_count).map(|i| value_to_node(rest_item(i))).collect(); Ok(UiNode::CenteredJustified { children: children? }) }
        "scroll-area" => {
            let id = get_prop_id(props_v).ok_or("scroll-area requires :id")?;
            let children: Result<Vec<_>, _> = (0..rest_count).map(|i| value_to_node(rest_item(i))).collect();
            Ok(UiNode::ScrollArea { id, children: children? })
        }
        "collapsing" => {
            let id = get_prop_id(props_v).ok_or("collapsing requires :id")?;
            let title = if rest_count > 0 { a.get_string(rest_item(0)).unwrap_or("").to_string() } else { String::new() };
            let children: Result<Vec<_>, _> = (1..rest_count).map(|i| value_to_node(rest_item(i))).collect();
            Ok(UiNode::Collapsing { id, title, children: children? })
        }
        "group" => { let children: Result<Vec<_>, _> = (0..rest_count).map(|i| value_to_node(rest_item(i))).collect(); Ok(UiNode::Group { children: children? }) }
        "grid" => {
            let id = get_prop_id(props_v).ok_or("grid requires :id")?;
            let columns = get_prop_int(props_v, "columns").unwrap_or(2) as usize;
            let children: Result<Vec<_>, _> = (0..rest_count).map(|i| value_to_node(rest_item(i))).collect();
            Ok(UiNode::Grid { id, columns, children: children? })
        }
        other => Err(format!("unknown ui node type: :{}", other)),
    }
}

pub fn value_to_tree(val: ElleValue) -> Result<Vec<UiNode>, ElleResult> {
    let a = api();
    if let Some(len) = a.get_array_len(val) {
        if len > 0 && a.check_keyword(a.get_array_item(val, 0)) {
            let node = value_to_node(val).map_err(|e| a.err("ui-error", &e))?;
            return Ok(vec![node]);
        }
        (0..len).map(|i| value_to_node(a.get_array_item(val, i)).map_err(|e| a.err("ui-error", &e))).collect()
    } else {
        Err(a.err("type-error", "egui/frame: tree must be an array"))
    }
}

// ── Rendering ────────────────────────────────────────────────────────

pub struct WidgetState {
    pub text_buffers: HashMap<String, String>,
    pub check_states: HashMap<String, bool>,
    pub slider_states: HashMap<String, f64>,
    pub combo_states: HashMap<String, String>,
    pub collapsed_states: HashMap<String, bool>,
}

impl WidgetState {
    pub fn new() -> Self {
        Self { text_buffers: HashMap::new(), check_states: HashMap::new(), slider_states: HashMap::new(), combo_states: HashMap::new(), collapsed_states: HashMap::new() }
    }
}

pub fn render_tree(ui: &mut egui::Ui, nodes: &[UiNode], state: &mut WidgetState, ix: &mut Interactions) {
    for node in nodes { render_node(ui, node, state, ix); }
}

fn render_node(ui: &mut egui::Ui, node: &UiNode, state: &mut WidgetState, ix: &mut Interactions) {
    match node {
        UiNode::Label { text } => { ui.label(text.as_str()); }
        UiNode::Heading { text } => { ui.heading(text.as_str()); }
        UiNode::ProgressBar { fraction, text } => {
            let mut bar = egui::ProgressBar::new(*fraction as f32);
            if let Some(t) = text { bar = bar.text(t.as_str()); }
            ui.add(bar);
        }
        UiNode::Separator => { ui.separator(); }
        UiNode::Spacer { size } => { ui.add_space(*size); }
        UiNode::Button { id, text } => { if ui.button(text.as_str()).clicked() { ix.clicked.insert(id.clone()); } }
        UiNode::TextInput { id, hint } => {
            let buf = state.text_buffers.entry(id.clone()).or_default();
            let mut edit = egui::TextEdit::singleline(buf);
            if let Some(h) = hint { edit = edit.hint_text(h.as_str()); }
            ui.add(edit);
            ix.text_values.insert(id.clone(), buf.clone());
        }
        UiNode::TextEdit { id, desired_rows } => {
            let buf = state.text_buffers.entry(id.clone()).or_default();
            ui.add(egui::TextEdit::multiline(buf).desired_rows(*desired_rows));
            ix.text_values.insert(id.clone(), buf.clone());
        }
        UiNode::Checkbox { id, text } => {
            let checked = state.check_states.entry(id.clone()).or_insert(false);
            ui.checkbox(checked, text.as_str());
            ix.check_values.insert(id.clone(), *checked);
        }
        UiNode::Slider { id, min, max } => {
            let val = state.slider_states.entry(id.clone()).or_insert(*min);
            ui.add(egui::Slider::new(val, *min..=*max));
            ix.slider_values.insert(id.clone(), *val);
        }
        UiNode::ComboBox { id, options } => {
            let selected = state.combo_states.entry(id.clone()).or_insert_with(|| options.first().cloned().unwrap_or_default());
            egui::ComboBox::from_id_salt(id.as_str()).selected_text(selected.as_str()).show_ui(ui, |ui| {
                for opt in options { ui.selectable_value(selected, opt.clone(), opt.as_str()); }
            });
            ix.combo_values.insert(id.clone(), selected.clone());
        }
        UiNode::VLayout { children } => { ui.vertical(|ui| render_tree(ui, children, state, ix)); }
        UiNode::HLayout { children } => { ui.horizontal(|ui| render_tree(ui, children, state, ix)); }
        UiNode::Centered { children } => { ui.vertical_centered(|ui| render_tree(ui, children, state, ix)); }
        UiNode::CenteredJustified { children } => { ui.vertical_centered_justified(|ui| render_tree(ui, children, state, ix)); }
        UiNode::ScrollArea { id, children } => { egui::ScrollArea::vertical().id_salt(id.as_str()).show(ui, |ui| render_tree(ui, children, state, ix)); }
        UiNode::Collapsing { id, title, children } => {
            let default_open = *state.collapsed_states.entry(id.clone()).or_insert(true);
            let resp = egui::CollapsingHeader::new(title.as_str()).id_salt(id.as_str()).default_open(default_open).show(ui, |ui| render_tree(ui, children, state, ix));
            let is_open = resp.fully_open();
            state.collapsed_states.insert(id.clone(), is_open);
            ix.collapsed.insert(id.clone(), !is_open);
        }
        UiNode::Group { children } => { ui.group(|ui| render_tree(ui, children, state, ix)); }
        UiNode::Grid { id, columns, children } => {
            egui::Grid::new(id.as_str()).num_columns(*columns).show(ui, |ui| {
                for (i, child) in children.iter().enumerate() {
                    render_node(ui, child, state, ix);
                    if (i + 1) % columns == 0 { ui.end_row(); }
                }
            });
        }
    }
}

// ── Interactions -> Value ─────────────────────────────────────────────

pub fn interactions_to_value(ix: &Interactions) -> ElleValue {
    let a = api();

    let clicks: Vec<ElleValue> = ix.clicked.iter().map(|s| a.keyword(s)).collect();
    let clicks_set = a.set(&clicks);

    let text_fields: Vec<(&str, ElleValue)> = ix.text_values.iter().map(|(k, v)| (k.as_str(), a.string(v))).collect();
    let check_fields: Vec<(&str, ElleValue)> = ix.check_values.iter().map(|(k, v)| (k.as_str(), a.boolean(*v))).collect();
    let slider_fields: Vec<(&str, ElleValue)> = ix.slider_values.iter().map(|(k, v)| (k.as_str(), a.float(*v))).collect();
    let combo_fields: Vec<(&str, ElleValue)> = ix.combo_values.iter().map(|(k, v)| (k.as_str(), a.string(v))).collect();
    let collapsed_fields: Vec<(&str, ElleValue)> = ix.collapsed.iter().map(|(k, v)| (k.as_str(), a.boolean(*v))).collect();

    a.build_struct(&[
        ("clicks", clicks_set),
        ("text", a.build_struct(&text_fields)),
        ("checks", a.build_struct(&check_fields)),
        ("sliders", a.build_struct(&slider_fields)),
        ("combos", a.build_struct(&combo_fields)),
        ("collapsed", a.build_struct(&collapsed_fields)),
        ("closed", a.boolean(ix.closed)),
        ("size", a.array(&[a.int(ix.width as i64), a.int(ix.height as i64)])),
    ])
}
