# Zoom Call Glitch Simulator

Like many, I've spent enough time on zoom calls to know that sometimes, when the connection is bad, they can sound pretty weird. But it got me thinking -- what possibilities would there be to hardness that bad connection for musical means? [Glitch](<https://en.wikipedia.org/wiki/Glitch_(music)>) is an entire musical genre, after all. So, I decided to build a simulator that could mimic packet loss, latency, and jitter in a networked audio stream, and see what happens when you apply these conditions to musical input.

I did some research and found that Zoom apparently uses the Opus codec for audio compression. This codec is typically used with C/C++ libraries, and those languages are great, but I prefer Rust for its safety and concurrency features. Fortunately, Opus has a [Rust](https://crates.io/crates/opus) implementation.

## Implementation

**Phase 1: Audio Pipeline Setup**

First, I decided to capture audio from my AirPods and simply record it. This would be my first audio source for Opus experimentation. This meant using [CPAL](https://github.com/RustAudio/cpal) to build a proper audio device detection system:

```rust
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
```

**Phase 2: Opus Integration and Network Simulation**

Getting basic audio from a "microphone" input via AirPods was pretty simple. The hard part was actually building a network simulator that could drop packets, add latency, and introduce jitter in realistic ways, while still honoring Rust's ownership model. Ultimately, I whittled it down to this core function:

```rust
pub fn simulate_network(&self, packet: Vec<u8>) -> Option<Vec<u8>> {
    // Simulate packet loss
    if random::<f32>() < self.packet_loss_probability {
        return None;
    }

    // Simulate latency and jitter using microseconds
    let jitter = random::<u64>() % self.jitter_us;
    thread::sleep(Duration::from_micros(self.latency_us + jitter));

    Some(packet)
}
```

With which I could then encode and decode audio frames using the Opus codec:

```rust
// De-interleave stereo samples
let mut left_channel: Vec<f32> = Vec::with_capacity(float_samples.len() / 2);
let mut right_channel: Vec<f32> = Vec::with_capacity(float_samples.len() / 2);
for chunk in float_samples.chunks(2) {
    left_channel.push(chunk[0]);
    right_channel.push(chunk[1]);
}

// Encode with Opus, simulate network, then decode
let encoded_len = encoder.encode_float(&frame, &mut encoded)?;
if let Some(received_packet) = network.simulate_network(encoded) {
    let decoded_len = decoder.decode_float(&received_packet, &mut decoded, false)?;
    // Write back to output...
}
```

## Musical?

And ultimately, it worked! But it wasn't musical... It just made me think of someone playing a guitar or keyboard through a choppy Zoom call. And it didn't have the intentional feel that an audio effect should have. Ultimately, it was a useful technical exercise, but I wouldn't expect to make an album with it.
