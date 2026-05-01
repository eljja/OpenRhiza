# Voice Input Plan

OpenRhiza should work naturally in keyboard-limited environments.
Voice input is therefore a first-class capability, but it must be implemented according to the OpenRhiza rule:

The core may capture or route minimal audio frames only when necessary. Speech recognition, voice activity detection, transcript correction, language policy, and multimodal LLM interaction should live in sandboxed skills and workflows.

## Goal

A user should be able to speak to OpenRhiza and have the OS treat speech as normal prompt input.

Expected behavior:

- wake or push-to-talk starts capture
- audio is converted to text by a voice skill or VL/multimodal LLM
- transcript appears in the composer before submission when possible
- user can confirm, edit, or cancel
- submitted voice prompts follow the same registry/workflow/driver policy as typed prompts

Keyboard input must remain available. Voice is an additional input route, not a replacement for recovery input.

## First Target: x86_64 QEMU / PC

The first implementation should target the existing x86_64 system before ARM or phones.

Preferred early path:

1. Host-side microphone capture for development.
2. Send bounded audio chunks to OpenRhiza through a narrow test channel.
3. Route audio to a sandbox voice skill.
4. Use Gemini or another multimodal LLM for transcription while local ASR is not ready.
5. Convert transcript into the same canonical prompt event used by typed input.

This avoids putting a full audio driver and ASR stack into the core too early.

## Long-Term Architecture

```text
microphone hardware
  -> audio driver capability
  -> audio frame host ABI
  -> skill_voice_vad_v1
  -> skill_voice_transcribe_v1
  -> skill_voice_intent_confirm_v1
  -> normal OpenRhiza prompt/workflow engine
```

Each stage should be a replaceable object:

- `input:microphone`
- `audio:capture-stream`
- `voice:vad`
- `voice:transcriber`
- `voice:language-router`
- `voice:confirmation-ui`
- `prompt:submission`

One failed voice component must not break keyboard, mouse, GUI, network, storage, or recovery shell.

## Core Responsibilities

Allowed in core:

- minimal audio device discovery metadata
- bounded audio frame buffer handles
- sandbox host ABI for reading captured frames
- activation and rollback gates
- emergency mute/disable command
- routing confirmed transcript into canonical input

Not allowed in core:

- speech recognition engine
- large language model client policy
- voice command policy
- natural language correction
- wake-word model
- GUI voice interface
- audio codec framework
- device-specific full audio drivers unless needed for survival

## Voice Host ABI Sketch

Future host calls should be handle-based, like the driver host ABI.

Candidate calls:

- `os_audio_list_devices(out_ptr, out_len)`
- `os_audio_claim_device(device_id, capability_id) -> handle`
- `os_audio_configure_stream(handle, sample_rate, channels, format)`
- `os_audio_read_frames(handle, dst_ptr, max_bytes) -> bytes_read`
- `os_audio_release(handle)`
- `os_audio_set_route(handle, route_flags)`

The host should enforce:

- max frame size
- max capture duration per cycle
- user-controlled enable/disable
- no background capture unless autonomy and voice capture are explicitly enabled
- visible status when recording

## Skill Set

Initial registry skills:

- `skill_voice_capture_bridge_v1`
- `skill_voice_vad_v1`
- `skill_voice_transcribe_gemini_v1`
- `skill_voice_prompt_confirm_v1`
- `workflow_voice_prompt_v1`
- `policy_voice_privacy_v1`

Later skills:

- `skill_voice_local_asr_v1`
- `skill_voice_noise_suppression_v1`
- `skill_voice_wake_phrase_v1`
- `skill_voice_speaker_profile_v1`
- `skill_voice_multilingual_router_v1`

## Privacy And Control

Voice input is sensitive.

Rules:

- default is off
- first boot may ask whether voice input should be enabled
- show a visible recording state
- allow `/voice off` at all times
- do not upload audio unless a voice workflow explicitly needs remote transcription
- prefer sending short bounded clips, not continuous streams
- keep transcript and audio retention separate
- never let autonomy enable voice capture by itself

## Commands

Planned commands:

- `/voice-status`
- `/voice on`
- `/voice off`
- `/voice push-to-talk`
- `/voice always-listen`
- `/voice-model <model>`
- `/voice-test`
- `/voice-clear-buffer`

`always-listen` must require explicit user approval and should not become the default.

Current implementation status:

- `/voice-status`
- `/voice <off|on|push-to-talk|always-listen>`
- `/voice-model <model>`
- `/voice-test`
- `/voice-clear-buffer`
- `skill_voice_capture_bridge_v1` seed skill
- `workflow_voice_prompt_v1` registry sync entry
- `policy_voice_privacy_v1` registry sync entry

The current voice bridge validates the sandbox capability path only.
It does not yet capture real microphone frames.

## Multimodal LLM Path

When using Gemini or another VL/multimodal model:

1. Capture a bounded clip.
2. Attach OS context only if needed.
3. Ask for transcript first, not direct action.
4. Display transcript.
5. Submit transcript only after confirmation, unless the user selected a trusted hands-free mode.

This prevents accidental action from background speech or misrecognition.

## Platform Expansion

### x86_64

Start with host-assisted capture or simple emulated audio.
Then move to sandboxed HDA/AC97/USB-audio drivers later.

### ARM64 QEMU

Use virtual audio only after serial/display/input are stable.
Voice is not required for first ARM boot.

### Android / Phone

Voice is strategically important on phones because keyboard may be absent.
However, phone audio capture depends on vendor audio stacks and permission/security models.
The phone path should reuse the same voice skill chain once an audio capture bridge exists.

## Validation

A voice feature is not considered usable until it proves:

- keyboard and GUI still work while voice is enabled
- voice can be disabled immediately
- failed transcription does not execute actions
- transcript is visible and editable
- network failure does not hang input
- audio buffers are bounded
- no persistent recording happens without explicit user choice
