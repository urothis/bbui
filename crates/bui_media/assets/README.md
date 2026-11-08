# Media test fixtures

The `sample-h264*.mp4` files are two-frame synthetic `testsrc2` patterns encoded with
FFmpeg/libx264 for decoder tests. They contain no recorded or third-party media. The fixture
settings match the upstream oxideav-mp4 `tests/fixtures/params/gen.sh` generator: 32×18, 25 fps,
no audio or metadata, and `ultrafast`; the suffixes identify 8-bit 4:2:0, 8-bit 4:2:2, 10-bit
4:2:0, and 8-bit 4:4:4 pixel formats. The latter is an unsupported-format regression.

`sample-h264-long.mp4` is a 601-frame synthetic stream made by repeating the self-contained IDR
sample from `sample-h264.mp4` and rebuilding its MP4 sample tables. It checks streaming playback
beyond the eager sequence decoder's 600-frame utility limit without adding encoder tooling to CI.

From the workspace root, run `cargo run -p bui_media --example media_playback` to load the SVG,
GIF, VP8 WebM, and H.264 MP4 in Bevy UI. The H.264 sample is also used by the MP4/MOV sequence
loader and bounded frame-cache lifecycle tests.
