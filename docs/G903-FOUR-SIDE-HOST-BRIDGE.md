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
- With the previously-read onboard mappings, the four entries still resolve
  to two Windows mouse-button identities. A G HUB software-profile test has
  now produced four distinct host keyboard usages (see evidence below).
  Whether F13-F16 can be **persisted in G903 onboard memory** without G HUB
  running remains unverified.

## Windows G903 host-event test — 17 October 2026

A user performed the single consolidated four-button test in Windows after
configuring F13-F16 assignments through Logitech G HUB. A local browser
keydown tester recorded the following (user-provided output):

| Host key | Browser `code` | `keyCode` | Events |
| --- | --- | --- | ---: |
| F13 | F13 | 124 | 14 |
| F14 | F14 | 125 | 7 |
| F15 | F15 | 126 | 7 |
| F16 | F16 | 127 | 7 |

All four Windows-visible keyboard identities were observed, showing that the
G HUB-managed assignments can distinguish four triggers at the host. The log
does not explicitly label each mouse-side physical switch, nor record separate
down/up edges or the G HUB onboard-mode state. F13 registered twice as many
events as each other key; this alone cannot establish accidental double firing
versus additional presses or auto-repeat.

**Validated:** four distinct F13-F16 host key events in the G HUB-assisted test.
**Not validated:** onboard persistence without G HUB; routing all four events
through a running OpenLogi Windows agent and saved macro workflows; G903
firmware/profile writes or recovery.

This is evidence supplied by the hardware user, not a CI hardware test.
No new firmware modifications were performed by OpenLogi.

## Safety and testing

The tab adds no device I/O; it reuses host keyboard bindings and workflow
actions. Existing mouse bindings remain untouched. Tests check all four F-key
identities, their uniqueness, and the absence of HID++ control targets.

No new MSI requested during implementation. Before doing any firmware writes:
active-profile verification, immutable backup, proven rollback, and explicit
user approval are mandatory. This PR remains Draft until CI and real-device
end-to-end validation.
