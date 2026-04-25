# OpenRhiza Boot Autorun

Commands in the fenced block below run automatically after the boot sequence reaches `input>`.

```openrhiza
@wait ticks 1000
/api-skill
@wait ticks 1500
/skill-load skill_display_console_mode_v1
@wait skill-stage skill_display_console_mode_v1 testing
/skill-run skill_display_console_mode_v1
@wait ticks 2500
/display-status
```
