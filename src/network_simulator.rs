use rand::random;
use std::thread;
use std::time::Duration;

// Struct to simulate network conditions
// todo: update this as needed
pub struct NetworkSimulator {
    pub packet_loss_probability: f32,
    pub latency_us: u64,
    pub jitter_us: u64,
}

impl NetworkSimulator {
    pub fn new(packet_loss_probability: f32, latency_us: u64, jitter_us: u64) -> Self {
        Self {
            packet_loss_probability,
            latency_us,
            jitter_us,
        }
    }

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
}
