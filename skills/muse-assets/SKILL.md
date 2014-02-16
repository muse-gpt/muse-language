---
name: muse-assets
description: Import, inspect, and maintain Muse project assets and metadata. Use when adding images, audio, MIDI, MusicXML, or other media to a Muse project, checking hashes, or diagnosing unsupported codecs.
---

# Muse Assets

Use `muse asset import <file>` so the project records a SHA-256 hash and a Git-tracked `.meta` file. Inspect entries with `muse asset info <id>`. Keep source assets under the project asset directory and never silently relabel an unsupported codec.

For music, prefer WAV/FLAC/MP3 audio and MIDI or MusicXML scores. Rendering is pure Rust; do not add FFmpeg commands or sys bindings. Report unsupported video codecs with an actionable replacement format.
