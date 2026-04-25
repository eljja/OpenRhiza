# OpenRhiza Boot Autorun

Commands in the fenced block below run automatically after the boot sequence reaches `input>`.

```openrhiza
/skill-download skill_display_console_mode_v1
@wait skill-cached skill_display_console_mode_v1
/skill-load skill_display_console_mode_v1
@wait skill-stage skill_display_console_mode_v1 testing
/skill-run skill_display_console_mode_v1
```
