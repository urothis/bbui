# Showcase media assets

These small synthetic assets are copied from `crates/bui_media/assets/` for the root
`assets/` path used by `cargo run -p bui --example showcase`. `loop.gif` is a generated
two-pixel red/blue animation. `sample.webm` is a VP8 fixture. `sample-h264.mp4` is a
two-frame 32×18 `testsrc2` pattern encoded with FFmpeg/libx264. They contain no
recorded or third-party artwork.

The media crate retains its longer and unsupported-format fixtures for decoder tests.
