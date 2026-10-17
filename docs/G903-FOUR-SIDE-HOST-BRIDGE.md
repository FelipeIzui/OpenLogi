# G903 HERO: four-side host-key macro workspace

This change exposes the **existing** fully-featured Keys remapping panel for
the G903 on Windows, with four explicit host-only shortcut slots
**F13, F14, F15 and F16**. No new hook, new firmware command, profile write,
new config schema, custom clipboard process, or kernel driver is needed.

## How it works

- When a G903 is selected on Windows, the existing Buttons panel is unchanged.
  Its Keys panel additionally appears and presents four F13-F16 triggers.
- Click a slot and use the regular action picker, shortcut editor, or workflow
  editor (multi-step text, keyboard shortcuts, pauses). All changes use
  `AppState::commit_keyboard_binding` and the pre-existing hook dispatch.
- Keyboard triggers are **global**. Another keyboard emitting F13 invokes
  the same macro; this isn't G903-specific device isolation.
- This GUI does **not** configure the physical G903 onboard profile. The
  previously verified read-only snapshot exposed four distinct firmware
  entries (4, 5, 6, 7), but with pairwise equal XBUTTON1 / XBUTTON2 assignments.
  The snapshot came from the first enabled profile, not necessarily the active.
- Therefore no four-button independence exists yet on an unchanged G903:
  physical switches must eventually be configured to emit four distinct host
  keyboard usages. Whether this G903 firmware and G HUB support storing
  F13-F16 onboard remains **unverified**; do not assert success prematurely.

## Safety and testing

The tab adds no device I/O; it reuses host keyboard bindings and workflow
actions. Existing mouse bindings remain untouched. Tests check all four F-key
identities, their uniqueness, and the absence of HID++ control targets.

No new MSI requested during implementation. Before doing any firmware writes:
active-profile verification, immutable backup, proven rollback, and explicit
user approval are mandatory. This PR remains Draft until CI and real-device
end-to-end validation.
