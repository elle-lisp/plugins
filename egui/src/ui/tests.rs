//! Side panels, laid out and clicked in a headless egui context.
//!
//! The counter-factual: the tree's only containers were layouts inside the
//! central panel, and a scroll area in an h-layout took one row's height, so
//! a list beside a detail pane never reached the bottom of the window.

use super::*;
use egui::panel::{PanelState, Side};
use egui::{pos2, vec2, Event, Id, Modifiers, Pos2, RawInput, Rect};

/// The window every test draws into.
const SCREEN: Rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(800.0, 600.0));

/// The central panel's inner rect: the screen inside its 8-point margin.
fn inner() -> Rect {
    SCREEN.shrink(8.0)
}

fn panel(id: &str, side: Side, width: f32, children: Vec<UiNode>) -> UiNode {
    UiNode::SidePanel { id: id.into(), side, width, children }
}

fn button(id: &str, text: &str) -> UiNode {
    UiNode::Button { id: id.into(), text: text.into() }
}

fn label(text: &str) -> UiNode {
    UiNode::Label { text: text.into() }
}

/// Draw one frame of NODES at TIME with EVENTS, and answer its interactions.
fn frame(ctx: &egui::Context, state: &mut WidgetState, nodes: &[UiNode], time: f64, events: Vec<Event>) -> Interactions {
    let input = RawInput { screen_rect: Some(SCREEN), time: Some(time), events, ..Default::default() };
    let mut ix = Interactions::default();
    let _ = ctx.run(input, |ctx| show(ctx, nodes, state, &mut ix));
    ix
}

fn press(at: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}

/// Lay NODES out, press at AT, release, and answer the release frame's interactions.
fn click(nodes: &[UiNode], at: Pos2) -> Interactions {
    let ctx = egui::Context::default();
    let mut state = WidgetState::new();
    frame(&ctx, &mut state, nodes, 0.0, vec![]);
    frame(&ctx, &mut state, nodes, 0.1, vec![Event::PointerMoved(at), press(at, true)]);
    frame(&ctx, &mut state, nodes, 0.2, vec![press(at, false)])
}

/// Draw NODES once, and answer the rect the panel ID was drawn in.
fn panel_rect(nodes: &[UiNode], id: &str) -> Rect {
    let ctx = egui::Context::default();
    let mut state = WidgetState::new();
    frame(&ctx, &mut state, nodes, 0.0, vec![]);
    PanelState::load(&ctx, Id::new(id)).expect("the panel stores its rect").rect
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1.0
}

#[test]
fn a_left_side_panel_runs_the_full_height_at_its_width() {
    let r = panel_rect(&[panel("tree", Side::Left, 240.0, vec![label("a row")])], "tree");
    assert!(near(r.left(), inner().left()), "on the left edge: {r:?}");
    assert!(near(r.top(), inner().top()) && near(r.bottom(), inner().bottom()), "the full height: {r:?}");
    assert!(near(r.width(), 240.0), "at its width: {r:?}");
}

#[test]
fn a_right_side_panel_meets_the_right_edge() {
    let r = panel_rect(&[panel("info", Side::Right, 180.0, vec![label("a fact")])], "info");
    assert!(near(r.right(), inner().right()), "on the right edge: {r:?}");
    assert!(near(r.bottom(), inner().bottom()), "the full height: {r:?}");
    assert!(near(r.width(), 180.0), "at its width: {r:?}");
}

#[test]
fn a_button_inside_a_side_panel_reports_its_click() {
    let nodes = [panel("tree", Side::Left, 240.0, vec![button("row", "a row")])];
    let ix = click(&nodes, inner().min + vec2(14.0, 8.0));
    assert!(ix.clicked.contains("row"), "the panel draws its children: {:?}", ix.clicked);
}

#[test]
fn a_node_after_a_side_panel_sits_beside_it() {
    // Drawn below the panel instead, the button would start at the bottom
    // of the window, and a click at the top beside the panel would miss.
    let nodes = [panel("tree", Side::Left, 240.0, vec![label("a row")]), button("detail", "the detail")];
    let ix = click(&nodes, inner().min + vec2(240.0 + 14.0, 6.0));
    assert!(ix.clicked.contains("detail"), "the next node starts beside the panel, at the top: {:?}", ix.clicked);
}

#[test]
fn a_side_is_left_or_right_and_left_when_absent() {
    assert_eq!(panel_side(None), Ok(Side::Left));
    assert_eq!(panel_side(Some("left")), Ok(Side::Left));
    assert_eq!(panel_side(Some("right")), Ok(Side::Right));
    let refused = panel_side(Some("top")).expect_err("no third side");
    assert!(refused.contains("top"), "the error names the value: {refused}");
}
