mod network_simulator;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SampleRate};
use network_simulator::NetworkSimulator;
use opus::{Application, Decoder, Encoder};
use std::fs::File;
use std::io::BufWriter;
use std::sync::{Arc, Mutex};

fn main() -> Result<(), anyhow::Error> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .expect("no output device available");

    // Set up input WAV file
    let input_file = hound::WavReader::open("input.wav")?;
    let input_spec = input_file.spec();
    let duration_seconds = input_file.duration() as f32 / input_spec.sample_rate as f32;
    println!("Input WAV spec: {:?}", input_spec);

    // Set the desired sample rate
    let desired_sample_rate = SampleRate(48000);

    // Get supported config from device
    let supported_config = device
        .supported_output_configs()?
        .find(|config| {
            config.channels() == 2
                && config.min_sample_rate() <= desired_sample_rate
                && config.max_sample_rate() >= desired_sample_rate
        })
        .expect("no supported config")
        .with_sample_rate(desired_sample_rate);

    // Prepare the output wav file
    const PATH: &str = "output_recording.wav";
    let spec = wav_file_spec_from_config(&supported_config);
    let writer = hound::WavWriter::create(PATH, spec)?;
    let writer = Arc::new(Mutex::new(Some(writer)));
    let writer_clone = writer.clone();

    // Initialize Opus encoder and decoder
    let mut encoder = Encoder::new(48000, opus::Channels::Stereo, Application::Voip)?;
    let mut decoder = Decoder::new(48000, opus::Channels::Stereo)?;

    // Set up network simulator
    let network = NetworkSimulator::new(0.5, 10, 5);

    println!("Begin processing...");

    let err_fn = move |err| {
        eprintln!("an error occurred on stream: {}", err);
    };

    // Read samples from input WAV and create stream based on format
    let stream = match supported_config.sample_format() {
        SampleFormat::F32 => {
            let samples: Vec<f32> = input_file
                .into_samples::<f32>()
                .filter_map(Result::ok)
                .collect();
            let samples_clone = samples.clone();
            let mut sample_idx = 0;

            device.build_output_stream(
                &supported_config.into(),
                move |data: &mut [f32], _: &_| {
                    for sample_out in data.iter_mut() {
                        if sample_idx < samples_clone.len() {
                            *sample_out = samples_clone[sample_idx];
                            sample_idx += 1;
                        } else {
                            *sample_out = 0.0;
                        }
                    }
                    write_input_data::<f32, f32>(
                        data,
                        &writer_clone,
                        &mut encoder,
                        &mut decoder,
                        &network,
                    );
                },
                err_fn,
                None,
            )?
        }
        SampleFormat::I16 => {
            let samples: Vec<i16> = input_file
                .into_samples::<i16>()
                .filter_map(Result::ok)
                .collect();
            let samples_clone = samples.clone();
            let mut sample_idx = 0;

            device.build_output_stream(
                &supported_config.into(),
                move |data: &mut [i16], _: &_| {
                    for sample_out in data.iter_mut() {
                        if sample_idx < samples_clone.len() {
                            *sample_out = samples_clone[sample_idx];
                            sample_idx += 1;
                        } else {
                            *sample_out = 0;
                        }
                    }
                    write_input_data::<i16, i16>(
                        data,
                        &writer_clone,
                        &mut encoder,
                        &mut decoder,
                        &network,
                    );
                },
                err_fn,
                None,
            )?
        }
        format => {
            return Err(anyhow::Error::msg(format!(
                "Unsupported sample format '{format}'"
            )))
        }
    };

    // Start processing
    stream.play()?;

    // Process for the duration of the input file plus a small buffer
    let duration = duration_seconds as u64;
    std::thread::sleep(std::time::Duration::from_secs(duration + 1));

    // Clean up and finalize the recording
    drop(stream);
    writer.lock().unwrap().take().unwrap().finalize()?;
    println!("Processing {} complete!", PATH);
    Ok(())
}

type WavWriterHandle = Arc<Mutex<Option<hound::WavWriter<BufWriter<File>>>>>;

fn write_input_data<T, U>(
    input: &[T],
    writer: &WavWriterHandle,
    encoder: &mut Encoder,
    decoder: &mut Decoder,
    network: &NetworkSimulator,
) where
    T: Sample + FromSample<f32>,
    U: Sample + hound::Sample + FromSample<T>,
    f32: FromSample<T>,
{
    if let Ok(mut guard) = writer.try_lock() {
        if let Some(writer) = guard.as_mut() {
            // Convert samples to f32 for Opus
            let float_samples: Vec<f32> = input.iter().map(|&s| f32::from_sample(s)).collect();

            // De-interleave stereo samples
            let mut left_channel: Vec<f32> = Vec::with_capacity(float_samples.len() / 2);
            let mut right_channel: Vec<f32> = Vec::with_capacity(float_samples.len() / 2);
            for chunk in float_samples.chunks(2) {
                left_channel.push(chunk[0]);
                right_channel.push(chunk[1]);
            }

            let mut deinterleaved = Vec::with_capacity(float_samples.len());
            deinterleaved.extend(&left_channel);
            deinterleaved.extend(&right_channel);

            // Encode with Opus
            const FRAME_SIZE: usize = 960;
            let mut frame = vec![0.0; FRAME_SIZE];
            let copy_size = std::cmp::min(deinterleaved.len(), FRAME_SIZE);
            frame[..copy_size].copy_from_slice(&deinterleaved[..copy_size]);
            let mut encoded = vec![0u8; 1275]; // Max opus packet size
            let encoded_len = encoder
                .encode_float(&frame, &mut encoded)
                .expect("Failed to encode");

            // Simulate network conditions
            if let Some(received_packet) = network.simulate_network(encoded) {
                // Decode with Opus
                let mut decoded = vec![0f32; 960]; // Frame size
                let decoded_len = decoder
                    .decode_float(&received_packet, &mut decoded, false)
                    .expect("Failed to decode");

                // Write decoded samples to both channels
                for sample in decoded[..decoded_len].iter() {
                    let sample: U = U::from_sample(Sample::from_sample(*sample));
                    writer.write_sample(sample).ok(); // Left channel
                    writer.write_sample(sample).ok(); // Right channel
                }
            }
        }
    }
}

fn wav_file_spec_from_config(config: &cpal::SupportedStreamConfig) -> hound::WavSpec {
    hound::WavSpec {
        channels: 2,
        sample_rate: config.sample_rate().0 as _,
        bits_per_sample: (config.sample_format().sample_size() * 8) as _,
        sample_format: sample_format_converter(config.sample_format()),
    }
}

fn sample_format_converter(format: cpal::SampleFormat) -> hound::SampleFormat {
    if format.is_float() {
        hound::SampleFormat::Float
    } else {
        hound::SampleFormat::Int
    }
}
