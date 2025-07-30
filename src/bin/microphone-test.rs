mod network_simulator;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SampleRate};
use network_simulator::NetworkSimulator;
use opus::{Application, Decoder, Encoder};
use std::fs::File;
use std::io::BufWriter;
use std::iter::zip;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

fn main() -> Result<(), anyhow::Error> {
    // Set the audio API to use
    // On macOS, this should be coreaudio....
    let host = cpal::default_host();

    // Call init_airpods function to initialize the airpods devices
    let (airpods_output, airpods_input) = init_airpods(&host);

    // Set the desired sample rate and call the airpods_config function
    let desired_sample_rate = SampleRate(48000);
    let (_supported_output_stream_config, supported_input_stream_config) =
        audio_device_stream_config(&airpods_output, &airpods_input, desired_sample_rate);

    println!("Input config set: {:?}", supported_input_stream_config);

    // Prepare the wav file we'll record input stream to
    const PATH: &str = "input_stream_recording.wav";
    let spec = wav_file_spec_from_config(&supported_input_stream_config);
    let writer = hound::WavWriter::create(PATH, spec)?;
    let writer = Arc::new(Mutex::new(Some(writer)));

    println!("Begin recording...");

    // Run the input stream on sep thread
    let writer_clone = writer.clone();

    let err_fn = move |err| {
        eprintln!("an error occurred on stream: {}", err);
    };

    // Set up the input stream
    let stream = match supported_input_stream_config.sample_format() {
        SampleFormat::I8 => airpods_input.build_input_stream(
            &supported_input_stream_config.into(),
            move |data, _: &_| write_input_data::<i8, i8>(data, &writer_clone),
            err_fn,
            None,
        )?,
        SampleFormat::I16 => airpods_input.build_input_stream(
            &supported_input_stream_config.into(),
            move |data, _: &_| write_input_data::<i16, i16>(data, &writer_clone),
            err_fn,
            None,
        )?,
        SampleFormat::I32 => airpods_input.build_input_stream(
            &supported_input_stream_config.into(),
            move |data, _: &_| write_input_data::<i32, i32>(data, &writer_clone),
            err_fn,
            None,
        )?,
        SampleFormat::F32 => airpods_input.build_input_stream(
            &supported_input_stream_config.into(),
            move |data, _: &_| write_input_data::<f32, f32>(data, &writer_clone),
            err_fn,
            None,
        )?,
        sample_format => {
            return Err(anyhow::Error::msg(format!(
                "Unsupported sample format '{sample_format}'"
            )))
        }
    };

    // Start recording stream
    stream.play()?;

    // Record for 10 seconds
    std::thread::sleep(std::time::Duration::from_secs(10));

    // Clean and finalize the recording
    drop(stream);
    writer.lock().unwrap().take().unwrap().finalize()?;
    println!("Recording {} complete!", PATH);
    Ok(())
}

type WavWriterHandle = Arc<Mutex<Option<hound::WavWriter<BufWriter<File>>>>>;

fn write_input_data<T, U>(input: &[T], writer: &WavWriterHandle)
where
    T: Sample,
    U: Sample + hound::Sample + FromSample<T>,
{
    if let Ok(mut guard) = writer.try_lock() {
        if let Some(writer) = guard.as_mut() {
            // For each input sample, write it to both channels
            for &sample in input.iter() {
                let sample: U = U::from_sample(sample);
                writer.write_sample(sample).ok(); // Left channel
                writer.write_sample(sample).ok(); // Right channel
            }
        }
    }
}

fn init_airpods(host: &cpal::Host) -> (cpal::Device, cpal::Device) {
    // Return all available input and output devices with the _device methods
    let output_devices = host.output_devices().unwrap();
    let input_devices = host.input_devices().unwrap();

    // Set AirPods Pro input/output devices // todo: make selectable!
    let (airpods_output, airpods_input) = zip(output_devices, input_devices)
        .find(|(out_dev, in_dev)| {
            out_dev
                .name()
                .map(|name| name.contains("AirPods Pro"))
                .unwrap_or(false)
                && in_dev
                    .name()
                    .map(|name| name.contains("AirPods Pro"))
                    .unwrap_or(false)
        })
        .expect("Could not find AirPods Pro.");
    println!("Airpods output and input devices confirmed:");
    println!(
        "Output: {}\nInput: {}",
        airpods_output.name().unwrap(),
        airpods_input.name().unwrap()
    );
    (airpods_output, airpods_input)
}

fn audio_device_stream_config(
    audio_output: &cpal::Device,
    audio_input: &cpal::Device,
    desired_sample_rate: SampleRate,
) -> (cpal::SupportedStreamConfig, cpal::SupportedStreamConfig) {
    let output_config_range = audio_output
        .supported_output_configs()
        .expect("Could not get supported output configurations")
        .find(|config| {
            config.sample_format() == SampleFormat::F32
                && config.channels() == 2
                && config.min_sample_rate() <= desired_sample_rate
                && config.max_sample_rate() >= desired_sample_rate
        })
        .expect("Could not find supported output configuration");
    let supported_output_stream_config = output_config_range
        .try_with_sample_rate(desired_sample_rate)
        .expect("48000 Hz is not supported");

    let input_config_range = audio_input
        .supported_input_configs()
        .expect("Could not get supported input configurations")
        .find(|config| {
            // Just check if it's within the sample rate range
            config.min_sample_rate() <= config.max_sample_rate()
        })
        .expect("Could not find supported input configuration");

    let supported_input_stream_config = input_config_range.with_max_sample_rate();
    (
        supported_output_stream_config,
        supported_input_stream_config,
    )
}

// Converts cpal::SampleFormat to hound::SampleFormat
fn sample_format_converter(format: cpal::SampleFormat) -> hound::SampleFormat {
    if format.is_float() {
        hound::SampleFormat::Float
    } else {
        hound::SampleFormat::Int
    }
}

fn wav_file_spec_from_config(config: &cpal::SupportedStreamConfig) -> hound::WavSpec {
    hound::WavSpec {
        channels: 2, // Force stereo for the wav
        sample_rate: config.sample_rate().0 as _,
        bits_per_sample: (config.sample_format().sample_size() * 8) as _,
        sample_format: sample_format_converter(config.sample_format()),
    }
}
