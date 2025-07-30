use hound;
use std::f32::consts::PI;
use std::i16;

const WAVE_SPEC: hound::WavSpec = hound::WavSpec {
    channels: 2,
    sample_rate: 48000,
    bits_per_sample: 16,
    sample_format: hound::SampleFormat::Int,
};

fn main() {
    let mut writer = hound::WavWriter::create("sine.wav", WAVE_SPEC).unwrap();
    for t in (0..WAVE_SPEC.sample_rate).map(|x| x as f32 / WAVE_SPEC.sample_rate as f32) {
        let left_sample = (t * 440.0 * 2.0 * PI).sin();
        let right_sample = (t * 940.0 * 2.0 * PI).sin();
        let amplitude = i16::MAX as f32;
        writer
            .write_sample((left_sample * amplitude) as i16)
            .unwrap();
        writer
            .write_sample((right_sample * amplitude) as i16)
            .unwrap();
    }
}
