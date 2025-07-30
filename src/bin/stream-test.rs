use console::Term;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Data, FromSample, Sample, SampleFormat, SampleRate};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::{f32::consts::PI, thread};

fn main() {
    // Create the frequency as an Arc<AtomicU32> in main
    let frequency = Arc::new(AtomicU32::new(440.0f32.to_bits()));

    // Clone Arc for the keyboard thread
    let keyboard_frequency = frequency.clone();

    // Spawn keyboard handling in a separate thread with the frequency
    let keyboard_thread = thread::spawn(move || {
        handle_keyboard_input(keyboard_frequency);
    });

    let host = cpal::default_host();

    // Return all available host devices with the device method
    let devices = host.devices().unwrap();

    println!("All available devices:");
    for device in devices {
        println!("{}", device.name().unwrap());
    }

    // Return all available input devices using the input devices method
    let input_devices = host.input_devices().unwrap();

    println!("\nAll available input devices:");
    for device in input_devices {
        println!("{}", device.name().unwrap());
    }

    // Set our output device to the default
    let device = host
        .default_output_device()
        .expect("no output device available");

    // Print the output device's name....
    println!("\nDefault output device: {}", device.name().unwrap());

    // Get a fresh iterator and get the first config
    let mut supported_configs_range = device
        .supported_output_configs()
        .expect("error while querying configs");

    // Set desired sample rate and try to get the supported configration with that sample rate
    let desired_sample_rate = SampleRate(48000);
    let config_range = supported_configs_range
        .find(|range| {
            range.min_sample_rate() <= desired_sample_rate
                && range.max_sample_rate() >= desired_sample_rate
        })
        .expect(&format!(
            "no supported config found for sample rate {:?}Hz",
            desired_sample_rate,
        ));
    let supported_config = config_range
        .try_with_sample_rate(desired_sample_rate)
        .expect("48000 Hz is not supported");

    // Print supported configuration
    println!("\nSupported configuration: {:?}", supported_config);

    let err_fn = |err| eprintln!("an error occurred on the output audio stream: {}", err);
    let sample_format = supported_config.sample_format();
    let config = supported_config.into();

    // print sample format
    println!("\nSample format: {:?}", sample_format);

    // Match statement for different sample formats
    let write_frequency = frequency.clone();
    let stream = match sample_format {
        SampleFormat::F32 => device.build_output_stream(
            &config,
            move |data, info| write_sine::<f32>(data, info, &write_frequency),
            err_fn,
            None,
        ),
        SampleFormat::I16 => device.build_output_stream(
            &config,
            move |data, info| write_sine::<i16>(data, info, &write_frequency),
            err_fn,
            None,
        ),
        SampleFormat::U16 => device.build_output_stream(
            &config,
            move |data, info| write_sine::<u16>(data, info, &write_frequency),
            err_fn,
            None,
        ),
        sample_format => panic!("Unsupported sample format '{sample_format}'"),
    };

    thread::sleep(Duration::from_secs(5));
}

fn write_sine<T: Sample + FromSample<f32>>(
    data: &mut [T],
    _: &cpal::OutputCallbackInfo,
    frequency: &Arc<AtomicU32>,
) {
    static mut SAMPLE_CLOCK: f32 = 0.0;
    let current_freq = f32::from_bits(frequency.load(Ordering::Relaxed));
    let sample_rate = 48000.0;
    let volume = 0.5;

    for sample in data.iter_mut() {
        unsafe {
            let value = (SAMPLE_CLOCK * current_freq * 2.0 * PI / sample_rate).sin() * volume;
            *sample = Sample::from_sample(value);
            SAMPLE_CLOCK = (SAMPLE_CLOCK + 1.0) % sample_rate;
        }
    }
}

fn handle_keyboard_input(frequency: Arc<AtomicU32>) {
    let term = Term::stdout();
    println!("Press any key (q to quit, w/s keys to change frequency)");

    loop {
        if let Ok(character) = term.read_char() {
            match character {
                'q' => {
                    println!("Quitting...");
                    break;
                }
                // Add frequency control
                'w' => {
                    // Up arrow alternative
                    let current = f32::from_bits(frequency.load(Ordering::Relaxed));
                    let new_freq = current + 10.0;
                    frequency.store(new_freq.to_bits(), Ordering::Relaxed);
                    println!("Frequency: {:.1} Hz", new_freq);
                }
                's' => {
                    // Down arrow alternative
                    let current = f32::from_bits(frequency.load(Ordering::Relaxed));
                    let new_freq = (current - 10.0).max(20.0);
                    frequency.store(new_freq.to_bits(), Ordering::Relaxed);
                    println!("Frequency: {:.1} Hz", new_freq);
                }
                _ => println!("You pressed: {}", character),
            }
        }
    }
}
