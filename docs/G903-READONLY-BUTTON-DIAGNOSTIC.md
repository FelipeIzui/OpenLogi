# G903 HERO — read-only button identity exploration

This phase is **diagnosis only**, layered on the working Windows G903 macro MVP from PR #1. The G903's actual feature table advertises **0x8110** (feature index 9 on the tested firmware) and **0x8100**, but not **0x1B04**. Never assume those indices for a different device: the CLI resolves the feature index dynamically.

Independent reverse-engineering on a **G502 HERO**, not on the G903, found unsolicited 0x8110 long HID++ reports with a big-endian 16-bit **physical-button bitmap**. Source: https://github.com/libratbag/libratbag/issues/1831 . This report layout **must be confirmed on the user's G903** before relying on it for remapping.

## Procedure (Windows x64 test build, CLI in installation's \`bin\` directory)

With G HUB closed, leave a harmless foreground window (e.g., Notepad) open. Use the terminal in the OpenLogi installation directory:

~~~powershell
.\bin\openlogi.exe diag button-spy --device G903 --seconds 45
~~~

Press and release each of the **four physical side buttons**, in a known order, one at a time. Record the on-screen \`DOWN bit NN\` and \`UP bit NN\` lines and associate each with the physical control; no file is created and no receiver identifiers are printed. If all four physical controls produce different bit numbers, that is promising evidence for independent detection (not yet independent remapping). If only two, or none, report what happened; do **not** attempt alternate firmware settings to force results.

This diagnostic performs only normal HID++ feature enumeration and passive input listening. It does not send a 0x8110 write, alter filter masks, save onboard profiles, change bindings, or touch receiver firmware. Existing macros may still execute when their physical buttons are pressed, so use an empty Notepad window and avoid applications where an accidental shortcut would be harmful.

**Interpretation caveats:** the G502 report format may not apply to G903; reads may be unavailable while another tool owns the HID node. The Windows OS-hook implementation continues to provide only the logical Back/XBUTTON1 and Forward/XBUTTON2 controls. Until distinct button identifiers are confirmed, **do not** expose four independent UI targets or change the working production controls.
