//! `cargo run -p tau-audio --release --example convert -- in.flac out.flac [bits]`: converts to what the
//! Pocket plays (rate folded to 44.1/48 kHz, at most stereo), for hand checks against other decoders.
use tau_audio::{
    decode::probe,
    flac_out,
    pipeline::{Pipeline, Target},
    pocket,
};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (input, output) = (std::path::Path::new(&a[1]), std::path::Path::new(&a[2]));
    let info = probe(input).expect("probe");
    let bits = a
        .get(3)
        .and_then(|b| b.parse().ok())
        .unwrap_or(info.bits.unwrap_or(16).clamp(16, 24));
    let target = Target {
        rate: pocket::auto_rate(info.rate),
        channels: info.channels.min(2),
    };
    let mut p = Pipeline::open(input, target).expect("open");
    let t = std::time::Instant::now();
    let frames = flac_out::encode(
        &mut p,
        bits,
        bits < info.bits.unwrap_or(0),
        &[],
        None,
        output,
        &mut |_| true,
    )
    .expect("encode");
    eprintln!(
        "{info:?} -> {} Hz {} ch {bits} bit, {} frames in {:?}",
        target.rate,
        target.channels,
        frames.frames,
        t.elapsed()
    );
}
