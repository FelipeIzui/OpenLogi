//! Inline editors for the parameterised power-user actions, shown inside the
//! config panel (the side inspector) once one is selected from the list.
//!
//! Each editor reuses the shared [`compact_panel`] surface. Draft state lives on
//! the [`FunctionRowView`] so it survives re-rendering. Closing the editor
//! returns to the action list; the panel itself closes when the key is
//! deselected.
//!
//! [`compact_panel`]: crate::features::binding_editor::compact_panel

#![expect(
    clippy::needless_pass_by_value,
    clippy::redundant_closure_for_method_calls,
    reason = "GPUI builders take owned Copy palette values; entity.update wants closures"
)]

use super::function_row::{FunctionRowView, KeyTarget, commit_key_action};
use crate::features::binding_editor::{compact_panel, divider, editor_scroll_list, title};
use crate::state::AppState;
use crate::ui::components::{MenuRow, control_input};
use crate::ui::theme::{self, Palette, Typography as _};
use gpui::{
    App, Entity, FontWeight, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px, svg,
};
use gpui_component::{
    Sizable as _,
    button::{Button, ButtonVariants},
    h_flex,
    input::InputState,
    v_flex,
};
use openlogi_core::binding::{Action, KeyCombo, WorkflowStep};

/// Which power-user editor is showing for the selected key.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PowerUserKind {
    TypeText,
    RunAppleScript,
    RunShellCommand,
    Workflow,
}

impl PowerUserKind {
    fn heading_key(self) -> &'static str {
        match self {
            Self::TypeText => "actions.type_text_heading",
            Self::RunAppleScript => "actions.run_applescript_heading",
            Self::RunShellCommand => "actions.run_shell_command_heading",
            Self::Workflow => "actions.workflow_heading",
        }
    }
}

/// Editable step kinds supported by the workflow composer.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum WorkflowInputKind {
    #[default]
    Text,
    Shortcut,
    Delay,
}

impl WorkflowInputKind {
    pub(crate) const ALL: [Self; 3] = [Self::Text, Self::Shortcut, Self::Delay];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Text => "Text",
            Self::Shortcut => "Shortcut",
            Self::Delay => "Delay",
        }
    }

    pub(crate) fn placeholder(self) -> &'static str {
        match self {
            Self::Text => "Text to type",
            Self::Shortcut => "Ctrl+Shift+P",
            Self::Delay => "Milliseconds (1-60000)",
        }
    }
}

/// Local editor draft. Only committed actions are written to the config.
pub(crate) struct WorkflowEditorState {
    pub steps: Vec<WorkflowStep>,
    pub input: Option<Entity<InputState>>,
    pub kind: WorkflowInputKind,
    pub editing: Option<usize>,
    pub error: Option<&'static str>,
}

pub(crate) fn parse_workflow_input(
    kind: WorkflowInputKind,
    value: &str,
) -> Result<WorkflowStep, &'static str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("Enter a value for this step");
    }
    match kind {
        WorkflowInputKind::Text => Ok(WorkflowStep::TypeText(value.to_string())),
        WorkflowInputKind::Shortcut => trimmed
            .parse::<KeyCombo>()
            .map(WorkflowStep::PressKey)
            .map_err(|_| "Enter a valid shortcut, e.g. Ctrl+Shift+P"),
        WorkflowInputKind::Delay => trimmed
            .parse::<u64>()
            .ok()
            .filter(|millis| (1..=60_000).contains(millis))
            .map(|millis| WorkflowStep::Delay { millis })
            .ok_or("Delay must be between 1 and 60000 ms"),
    }
}

pub(crate) fn workflow_input_seed(step: &WorkflowStep) -> Option<(WorkflowInputKind, String)> {
    match step {
        WorkflowStep::TypeText(text) => Some((WorkflowInputKind::Text, text.clone())),
        WorkflowStep::PressKey(combo) => {
            Some((WorkflowInputKind::Shortcut, combo.rendered_label()))
        }
        WorkflowStep::Delay { millis } => Some((WorkflowInputKind::Delay, millis.to_string())),
        WorkflowStep::RunAppleScript(_) | WorkflowStep::RunShellCommand(_) => None,
    }
}

pub(crate) fn text_editor_placeholder(kind: PowerUserKind) -> gpui::SharedString {
    match kind {
        PowerUserKind::TypeText => tr!("actions.type_text_placeholder"),
        PowerUserKind::RunAppleScript => "display dialog \"Hello\"".into(),
        PowerUserKind::RunShellCommand => "echo hello".into(),
        PowerUserKind::Workflow => "".into(),
    }
}

pub(crate) fn text_editor_seed(action: Option<&Action>, kind: PowerUserKind) -> String {
    match (action, kind) {
        (Some(Action::TypeText(text)), PowerUserKind::TypeText)
        | (Some(Action::RunAppleScript(text)), PowerUserKind::RunAppleScript)
        | (Some(Action::RunShellCommand(text)), PowerUserKind::RunShellCommand) => text.clone(),
        _ => String::new(),
    }
}

pub(crate) fn workflow_editor_seed(action: Option<&Action>) -> Vec<WorkflowStep> {
    match action {
        Some(Action::Workflow(steps)) => steps.clone(),
        _ => Vec::new(),
    }
}

/// Render the editor card for `kind`, replacing the panel's action list.
pub fn editor_card(
    target: KeyTarget,
    kind: PowerUserKind,
    text_state: Option<Entity<InputState>>,
    workflow: WorkflowEditorState,
    view: &Entity<FunctionRowView>,
    pal: Palette,
) -> gpui::Div {
    match kind {
        PowerUserKind::Workflow => workflow_editor_card(target, workflow, view, pal),
        _ => match text_state {
            Some(state) => text_editor_card(target, kind, state, view, pal),
            None => compact_panel(pal)
                .w(px(300.))
                .child(title(tr!("keyboard.editor_unavailable"), pal)),
        },
    }
}

/// The TypeText / RunAppleScript / RunShellCommand editors share a single text
/// field; only the commit wrapping differs.
fn text_editor_card(
    target: KeyTarget,
    kind: PowerUserKind,
    text_state: Entity<InputState>,
    view: &Entity<FunctionRowView>,
    pal: Palette,
) -> gpui::Div {
    let heading = tr!(kind.heading_key());
    let key_name = target.label();

    compact_panel(pal)
        .w(px(300.))
        .child(title(
            tr!("actions.action_key_summary", action => heading, key => key_name),
            pal,
        ))
        .child(divider(pal))
        .child(
            v_flex()
                .p_2()
                .gap_2()
                .child(div().child(control_input(&text_state).cleanable(true)))
                .child(editor_action_row(target, kind, view)),
        )
}

/// Cancel (back to list) + Save (commit the drafted text).
fn editor_action_row(
    target: KeyTarget,
    kind: PowerUserKind,
    view: &Entity<FunctionRowView>,
) -> impl IntoElement {
    let view_save = view.clone();
    let view_cancel = view.clone();

    h_flex()
        .gap_2()
        .justify_end()
        .child(
            Button::new("editor-cancel")
                .ghost()
                .label(tr!("common.cancel"))
                .on_click(move |_e, _window, cx| {
                    view_cancel.update(cx, |v, vcx| v.close_editor(vcx));
                }),
        )
        .child(
            Button::new("editor-save")
                .primary()
                .label(tr!("common.save"))
                .on_click(move |_e, _window, cx| {
                    let text = view_save
                        .read(cx)
                        .text_state()
                        .map(|s| s.read(cx).value().to_string())
                        .unwrap_or_default();
                    let action = match kind {
                        PowerUserKind::TypeText => Action::TypeText(text),
                        PowerUserKind::RunAppleScript => Action::RunAppleScript(text),
                        PowerUserKind::RunShellCommand => Action::RunShellCommand(text),
                        PowerUserKind::Workflow => return,
                    };
                    AppState::apply(cx, |state| commit_key_action(state, &target, Some(action)));
                    view_save.update(cx, |v, vcx| v.close_editor(vcx));
                }),
        )
}

/// The Workflow editor: compose text, shortcut and delay steps without TOML.
fn workflow_editor_card(
    target: KeyTarget,
    draft: WorkflowEditorState,
    view: &Entity<FunctionRowView>,
    pal: Palette,
) -> gpui::Div {
    let key_name = target.label();
    let WorkflowEditorState {
        steps,
        input,
        kind,
        editing,
        error,
    } = draft;
    let has_steps = !steps.is_empty();
    let rows = steps
        .into_iter()
        .enumerate()
        .map(|(idx, step)| WorkflowStepRow {
            idx,
            step,
            view: view.clone(),
        });

    let step_buttons = WorkflowInputKind::ALL.into_iter().map(|candidate| {
        let v = view.clone();
        Button::new(format!("wf-kind-{}", candidate.label()))
            .ghost()
            .small()
            .label(candidate.label())
            .on_click(move |_e, window, cx| {
                v.update(cx, |v, vcx| {
                    v.set_workflow_input_kind(candidate, window, vcx);
                });
            })
    });

    let view_add = view.clone();

    compact_panel(pal)
        .w(px(320.))
        .child(title(
            tr!("actions.workflow_key_summary", key => key_name),
            pal,
        ))
        .child(divider(pal))
        .child(editor_scroll_list("workflow-steps", rows))
        .child(
            v_flex()
                .p_2()
                .gap_2()
                .child(h_flex().gap_1().children(step_buttons))
                .child(
                    div()
                        .text_caption()
                        .text_color(pal.text_muted)
                        .child(format!(
                            "{} {}",
                            if editing.is_some() { "Edit" } else { "New" },
                            kind.label()
                        )),
                )
                .children(input.map(|input| control_input(&input).cleanable(true)))
                .children(error.map(|message| {
                    div()
                        .text_caption()
                        .text_color(pal.text_muted)
                        .child(message)
                }))
                .child(
                    Button::new("wf-add-step")
                        .ghost()
                        .small()
                        .label(if editing.is_some() {
                            "Update step"
                        } else {
                            "Add step"
                        })
                        .on_click(move |_e, window, cx| {
                            let input = view_add.read(cx).workflow_input();
                            if let Some(input) = input {
                                let value = input.read(cx).value().to_string();
                                let success = view_add
                                    .update(cx, |v, vcx| v.apply_workflow_input(&value, vcx));
                                if success {
                                    input.update(cx, |state, cx| {
                                        state.set_value(String::new(), window, cx);
                                    });
                                }
                            }
                        }),
                )
                .child(workflow_action_row(target, has_steps, view)),
        )
}

/// Cancel a workflow draft or save its validated steps.
fn workflow_action_row(
    target: KeyTarget,
    has_steps: bool,
    view: &Entity<FunctionRowView>,
) -> impl IntoElement {
    let view_cancel = view.clone();
    let view_save = view.clone();

    h_flex()
        .gap_2()
        .justify_end()
        .child(
            Button::new("wf-cancel")
                .ghost()
                .label(tr!("common.cancel"))
                .on_click(move |_e, _window, cx| {
                    view_cancel.update(cx, |v, vcx| v.close_editor(vcx));
                }),
        )
        .child(
            Button::new("wf-save")
                .primary()
                .label(tr!("actions.save_workflow"))
                .on_click(move |_e, _window, cx| {
                    if !has_steps {
                        return;
                    }
                    let steps = view_save.read(cx).workflow_draft().to_vec();
                    let action = Action::Workflow(steps);
                    AppState::apply(cx, |state| commit_key_action(state, &target, Some(action)));
                    view_save.update(cx, |v, vcx| v.close_editor(vcx));
                }),
        )
}

/// One Workflow step row: type chip + payload preview + remove button.
#[derive(IntoElement)]
struct WorkflowStepRow {
    idx: usize,
    step: WorkflowStep,
    view: Entity<FunctionRowView>,
}

impl RenderOnce for WorkflowStepRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (type_label, glyph): (&'static str, &'static str) = match &self.step {
            WorkflowStep::TypeText(_) => ("Type Text", "action-icons/keyboard.svg"),
            WorkflowStep::PressKey(_) => ("Press Key", "action-icons/keyboard.svg"),
            WorkflowStep::Delay { .. } => ("Delay", "action-icons/chevrons-right.svg"),
            WorkflowStep::RunAppleScript(_) => ("AppleScript", "action-icons/terminal.svg"),
            WorkflowStep::RunShellCommand(_) => ("Shell", "action-icons/terminal.svg"),
        };
        let pal = theme::palette(cx);
        let view_edit = self.view.clone();
        let view_remove = self.view;
        let idx = self.idx;

        h_flex()
            .w_full()
            .gap_1()
            .child(
                div().flex_1().min_w_0().child(
                    MenuRow::new(("wf-step", idx))
                        .child(
                            h_flex()
                                .w_full()
                                .items_center()
                                .gap_2()
                                .child(
                                    svg()
                                        .path(glyph)
                                        .size_4()
                                        .flex_none()
                                        .text_color(pal.text_muted),
                                )
                                .child(
                                    div()
                                        .text_caption()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(pal.text_muted)
                                        .child(type_label),
                                )
                                .child(div().flex_1().child(step_preview(&self.step, pal))),
                        )
                        .on_click(move |_e, window, cx| {
                            view_edit.update(cx, |v, vcx| {
                                v.edit_workflow_step(idx, window, vcx);
                            });
                        }),
                ),
            )
            .child(
                Button::new(("wf-remove", idx))
                    .ghost()
                    .small()
                    .label("Remove")
                    .on_click(move |_e, _window, cx| {
                        view_remove.update(cx, |v, vcx| v.remove_workflow_step(idx, vcx));
                    }),
            )
    }
}

fn step_preview(step: &WorkflowStep, pal: Palette) -> impl IntoElement {
    let text: String = match step {
        WorkflowStep::TypeText(s) => {
            if s.is_empty() {
                "…".to_string()
            } else {
                format!("“{s}”")
            }
        }
        WorkflowStep::PressKey(k) => key_combo_preview(k),
        WorkflowStep::Delay { millis } => format!("{millis} ms"),
        WorkflowStep::RunAppleScript(s) | WorkflowStep::RunShellCommand(s) => {
            if s.is_empty() {
                "…".to_string()
            } else {
                s.clone()
            }
        }
    };
    div()
        .text_caption()
        .text_color(pal.text_primary)
        .child(text)
}

fn key_combo_preview(combo: &KeyCombo) -> String {
    combo.rendered_label()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_editor_seed_only_uses_matching_power_user_action() {
        assert_eq!(
            text_editor_seed(
                Some(&Action::RunAppleScript(
                    "tell app \"Finder\" to activate".into()
                )),
                PowerUserKind::RunAppleScript,
            ),
            "tell app \"Finder\" to activate"
        );
        assert_eq!(
            text_editor_seed(
                Some(&Action::RunShellCommand("echo nope".into())),
                PowerUserKind::RunAppleScript,
            ),
            ""
        );
    }

    #[test]
    fn workflow_editor_seed_only_uses_workflow_action() {
        let steps = vec![WorkflowStep::TypeText("hello".into())];
        assert_eq!(
            workflow_editor_seed(Some(&Action::Workflow(steps.clone()))),
            steps
        );
        assert!(
            workflow_editor_seed(Some(&Action::RunAppleScript(
                "display dialog \"Hello\"".into()
            )))
            .is_empty()
        );
    }
    #[test]
    fn workflow_composer_parses_text_shortcuts_and_delays() {
        assert_eq!(
            parse_workflow_input(WorkflowInputKind::Text, "á 😊"),
            Ok(WorkflowStep::TypeText("á 😊".into()))
        );
        assert_eq!(
            parse_workflow_input(WorkflowInputKind::Shortcut, "Ctrl+Shift+P")
                .expect("valid shortcut"),
            WorkflowStep::PressKey("Ctrl+Shift+P".parse().expect("valid key combo"))
        );
        assert_eq!(
            parse_workflow_input(WorkflowInputKind::Delay, "250"),
            Ok(WorkflowStep::Delay { millis: 250 })
        );
        assert_eq!(
            parse_workflow_input(WorkflowInputKind::Delay, "60001"),
            Err("Delay must be between 1 and 60000 ms")
        );
        assert_eq!(
            parse_workflow_input(WorkflowInputKind::Shortcut, "???"),
            Err("Enter a valid shortcut, e.g. Ctrl+Shift+P")
        );
        assert_eq!(
            parse_workflow_input(WorkflowInputKind::Text, "   "),
            Err("Enter a value for this step")
        );
    }
}
