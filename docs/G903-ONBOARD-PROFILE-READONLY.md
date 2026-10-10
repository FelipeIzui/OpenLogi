# G903 HERO — reading the 0x8100 onboard-profile descriptor

This is a deliberately **read-only descriptor probe**. It does **not** read,
decode, switch or write onboard profiles, adjust the 0x8110 mouse-button filter,
or remap device keys.

## Why

The G903 HERO's four physical lateral switches currently reach Windows as only
two XBUTTON identities. The user's 45-second `0x8110` passive diagnostic
reported no unsolicited notifications (even after retry); a separate 45-second
Windows Raw Input capture found `Mouse#1`, XBUTTON1 and XBUTTON2, with no extra
`ulRawButtons` bits. These observations cannot independently identify four
physical controls.

On a G903 (WPID `4087`), the libratbag read-only hardware snapshot reports
12 *firmware button entries*, with multiple distinct entries mapped to the
same logical XBUTTON1/XBUTTON2. This makes **onboard profile mappings** a
promising lead, not permission to write memory. Historically, G903 HERO
firmware used profile format 4, and unsafe/unsupported tooling has caused
profile corruption.

References:
- https://github.com/libratbag/libratbag/issues/1464
- https://github.com/libratbag/libratbag/issues/774

## Command after a validated Windows build

From the OpenLogi install directory in PowerShell:

~~~powershell
.\bin\openlogi.exe diag profile-info --device G903
~~~

The command sends exactly one protocol-level request to `0x8100`:
`GetProfilesDescription` (function ID 0). It prints metadata only:
memory model, profile format, macro format, profile counts, button-entry
count, sector count and size, mechanical-layout code and flags. It never
sends SetOnboardMode, SetActiveProfile, ReadProfileSector, WriteProfileSector,
or a command to `0x8110`.

If firmware reports an unfamiliar profile format, **stop at descriptor
discovery**. No profile byte decoding, settings modifications, or experimental
writes until the exact firmware's layout and complete backup/rollback
strategy are known and the user explicitly approves.

A firmware entry count of 12 does **not** mean Windows can see 12 independent
events. Four individual bindings would require the device to report
distinguishable host events, possibly by first assigning physically distinct
slots to spare keyboard usages in a future *opt-in, backed-up* operation.
