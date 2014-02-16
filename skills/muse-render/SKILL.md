---
name: muse-render
description: Compile, preview, and render Muse projects through the pure-Rust media pipeline. Use when producing render plans, PNG/WAV previews, deterministic outputs, or supported audio and score exports.
---

# Muse Rendering

Run `muse check` before `muse compile`, then inspect the stable JSON render plan. Use `muse preview <script.muse>` for PNG frames, WAV audio, and a manifest. Use `muse render <script.muse> -o output.wav` for audio; score interchange is available through `muse export`.

Rendering must be deterministic for the same script, asset hashes, project configuration, and seed. Do not call FFmpeg or fabricate MP4/WebM output; fail clearly when a requested container or codec is not implemented.
