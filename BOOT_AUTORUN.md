# OpenRhiza Boot Autorun

Commands in the fenced block below run automatically after the boot sequence reaches `input>`.

This file exists to bootstrap capability bring-up during development.
It is not the final user-facing interaction model.
The final goal is that the OpenRhiza intelligence itself decides which skills, workflows, and GUI mutations to run from inside the machine.
The public-facing boot default is now command-first: the recovery/command shell stays active after bootstrap, and GUI or desktop shells are launched explicitly with `/gui` or `/desktop`.

```openrhiza
@wait ticks 1000
/api-skill
@wait ticks 1500
/skill-load skill_qemu_driver_pack_v1
@wait skill-stage skill_qemu_driver_pack_v1 testing
/skill-run skill_qemu_driver_pack_v1
/skill-load skill_e1000_sandbox_probe_v1
@wait skill-stage skill_e1000_sandbox_probe_v1 testing
/skill-run skill_e1000_sandbox_probe_v1
/net-driver-status
/driver-bindings
/driver-host-status
/skill-load skill_piix_ide_sandbox_probe_v1
@wait skill-stage skill_piix_ide_sandbox_probe_v1 testing
/skill-run skill_piix_ide_sandbox_probe_v1
/skill-load skill_xhci_sandbox_probe_v1
@wait skill-stage skill_xhci_sandbox_probe_v1 testing
/skill-run skill_xhci_sandbox_probe_v1
/driver-host-status
/input-routing-status
/sandbox-keyboard-load
@wait ticks 500
/sandbox-mouse-load
@wait ticks 500
/input-routing-status
/fs-bridge-status
/skill-load skill_storage_host_probe_v1
@wait skill-stage skill_storage_host_probe_v1 testing
/skill-run skill_storage_host_probe_v1
/fs-bridge-status
@wait ticks 500
/skill-load skill_display_console_mode_v1
@wait skill-stage skill_display_console_mode_v1 testing
/skill-run skill_display_console_mode_v1
@wait ticks 1000
/display-status
/desktop-status
/gui-scene
```
