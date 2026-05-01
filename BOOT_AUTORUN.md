# OpenRhiza Boot Autorun

Commands in the fenced block below run automatically after the boot sequence reaches `input>`.

This file exists to bootstrap capability bring-up during development.
It is not the final user-facing interaction model.
The final goal is that the OpenRhiza intelligence itself decides which skills, workflows, and GUI mutations to run from inside the machine.

```openrhiza
@wait ticks 1000
/api-skill
@wait ticks 1500
/skill-load skill_qemu_driver_pack_v1
@wait skill-stage skill_qemu_driver_pack_v1 testing
/skill-run skill_qemu_driver_pack_v1
/driver-bindings
/driver-host-status
@wait ticks 500
/skill-load skill_display_console_mode_v1
@wait skill-stage skill_display_console_mode_v1 testing
/skill-run skill_display_console_mode_v1
@wait ticks 1000
/skill-load skill_gui_modern_shell_v1
@wait skill-stage skill_gui_modern_shell_v1 testing
/skill-run skill_gui_modern_shell_v1
@wait ticks 2500
/gui-focus composer
/display-status
/gui-scene
```
