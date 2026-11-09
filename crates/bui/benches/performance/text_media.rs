use bevy::{
    app::App,
    asset::Assets,
    text::{Font, FontCx, LayoutCx, TextPipeline},
};
use bui_core::history::History;
use bui_media::{
    decode_gif_thumbnail, decode_image, decode_image_thumbnail, decode_mp4_h264_thumbnail,
    decode_svg, decode_webm_vp8_thumbnail,
};
use bui_text::{TextBuffer, TextLayout, TextPlugin, expression, ui_default_text_font};
use bui_widgets::NumericField;
use criterion::{BatchSize, BenchmarkId, Criterion, Throughput};
use image::ImageEncoder;
use std::collections::HashMap;
use std::hint::black_box;

fn raster_fixture() -> Vec<u8> {
    const SIDE: u32 = 512;
    let mut rgba = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    for y in 0..SIDE {
        for x in 0..SIDE {
            rgba.extend_from_slice(&[(x / 2) as u8, (y / 2) as u8, ((x ^ y) & 0xff) as u8, 255]);
        }
    }
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(&rgba, SIDE, SIDE, image::ExtendedColorType::Rgba8)
        .expect("encode deterministic PNG benchmark fixture");
    png
}

pub fn bench(c: &mut Criterion) {
    let mut app = App::new();
    app.add_plugins(TextPlugin);
    let mut pipeline = app
        .world_mut()
        .remove_resource::<TextPipeline>()
        .expect("text pipeline resource");
    let fonts = app
        .world_mut()
        .remove_resource::<Assets<Font>>()
        .expect("font assets resource");
    let mut font_cx = app
        .world_mut()
        .remove_resource::<FontCx>()
        .expect("font collection resource");
    let mut layout_cx = app
        .world_mut()
        .remove_resource::<LayoutCx>()
        .expect("layout context resource");
    let font = ui_default_text_font(14.0);
    let paragraph =
        "NodeGraph · typed sockets · stable links. Drag to move, pan, or connect. ".repeat(12);
    let mut group = c.benchmark_group("text/bevy_shape");
    for (name, text, width) in [
        ("label", "NodeGraph · typed sockets · stable links", 600.0),
        ("wrapped_paragraph", paragraph.as_str(), 360.0),
    ] {
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(BenchmarkId::new("shape", name), &text, |b, text| {
            b.iter(|| {
                let shaped = TextLayout::shape_with_bevy(
                    black_box(text),
                    width,
                    &font,
                    &mut pipeline,
                    &fonts,
                    &mut font_cx,
                    &mut layout_cx,
                )
                .expect("bundled Inter font should shape");
                black_box(shaped.bounds());
            });
        });
    }
    group.finish();

    let vars = HashMap::from([("x".to_owned(), 1.25)]);
    let mut group = c.benchmark_group("editing/model");
    group.bench_function("expression_evaluate", |b| {
        b.iter(|| {
            black_box(
                expression::evaluate(black_box("2*pi + min(4, 3) + x"), &vars)
                    .expect("valid expression"),
            );
        });
    });
    let mut field = NumericField::float(1.25).expect("finite field");
    group.bench_function("numeric_field_commit", |b| {
        b.iter(|| {
            black_box(
                field
                    .evaluate_input(black_box("2*pi + min(4, 3)"), &vars)
                    .expect("valid numeric edit"),
            );
        });
    });
    group.bench_function("text_insert_undo_redo", |b| {
        b.iter_batched(
            || TextBuffer::new("BUI text field"),
            |mut buffer| {
                buffer.insert_text(black_box(" · edited"));
                buffer.undo();
                buffer.redo();
                black_box(buffer.as_str().len());
            },
            BatchSize::SmallInput,
        );
    });
    group.bench_function("history_transaction_undo_redo", |b| {
        b.iter_batched(
            History::<i64>::default,
            |mut history| {
                history.begin("number").expect("start transaction");
                history.record("field", 1, 2).expect("record edit");
                history.commit().expect("commit edit");
                black_box(history.undo().expect("undo edit"));
                black_box(history.redo().expect("redo edit"));
            },
            BatchSize::SmallInput,
        );
    });
    group.finish();

    let raster = raster_fixture();
    let svg = include_bytes!("../../../../crates/bui_media/assets/sample.svg");
    let gif = include_bytes!("../../../../crates/bui_media/assets/loop.gif");
    let webm = include_bytes!("../../../../crates/bui_media/assets/sample.webm");
    let mp4 = include_bytes!("../../../../crates/bui_media/assets/sample-h264.mp4");
    let mut group = c.benchmark_group("media/decode");
    for (name, bytes, decode) in [
        (
            "png_full",
            raster.as_slice(),
            decode_image as fn(&[u8]) -> Result<_, _>,
        ),
        ("svg_full", svg.as_slice(), decode_svg),
    ] {
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_with_input(BenchmarkId::new("frame", name), &bytes, |b, bytes| {
            b.iter(|| {
                let frame = decode(black_box(bytes)).expect("checked media fixture");
                black_box(frame.pixels.len());
            });
        });
    }
    for (name, bytes, decode) in [
        (
            "png_thumbnail",
            raster.as_slice(),
            decode_image_thumbnail as fn(&[u8], u32, u32) -> Result<_, _>,
        ),
        ("gif_thumbnail", gif.as_slice(), decode_gif_thumbnail),
        (
            "webm_vp8_thumbnail",
            webm.as_slice(),
            decode_webm_vp8_thumbnail,
        ),
    ] {
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_with_input(BenchmarkId::new("poster", name), &bytes, |b, bytes| {
            b.iter(|| {
                let frame = decode(black_box(bytes), 128, 128).expect("checked media fixture");
                black_box(frame.pixels.len());
            });
        });
    }
    group.throughput(Throughput::Bytes(mp4.len() as u64));
    group.bench_function("poster/mp4_h264_thumbnail", |b| {
        b.iter(|| {
            let frame = decode_mp4_h264_thumbnail(black_box(mp4.to_vec()), 128, 128)
                .expect("checked MP4 fixture");
            black_box(frame.pixels.len());
        });
    });
    group.finish();
}
