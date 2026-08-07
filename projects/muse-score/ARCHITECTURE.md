# Muse score engraving architecture

The cloned references in `tmp/reference-verovio`, `tmp/reference-vexflow`,
`tmp/reference-abcjs`, and `tmp/reference-lilypond` are study material only.
They are not dependencies and are never invoked at build or run time.

Muse should follow the common architecture visible across those projects:

1. **Semantic score**: normalize `Score` into measures, voices, rhythmic
   durations, pitch spelling, rests, tuplets, and spanners. No coordinates.
2. **Measure model**: create `MeasureEvent` and `VoiceEvent` streams, resolve
   simultaneous notes into note columns, and preserve ties/slurs/dynamics.
3. **Horizontal spacing**: calculate columns from rhythmic duration and glyph
   collision widths, then justify each measure and system to a target width.
4. **Vertical spacing**: place staves and use skyline bounds for noteheads,
   stems, beams, text, dynamics, and spanners. Layout must be deterministic.
5. **System/page breaking**: cast measures into systems, then systems into
   pages. The break result is independent of the drawing backend.
6. **Backend scene**: emit a backend-neutral `Scene` containing paths, text,
   glyph references, and lines. PDF, SVG, and HTML renderers consume this
   scene; none of them performs musical layout.

The current renderer is deliberately considered a transitional backend. The
next implementation should introduce these modules and tests before adding
more glyphs. This prevents another backend-specific coordinate generator from
becoming the score model.
