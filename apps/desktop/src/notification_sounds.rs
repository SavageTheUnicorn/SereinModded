//! One bounded, lazy audio worker. Original synthesized cues never interrupt attachment playback.
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use model::notification_preferences::Sound;
use std::{
	sync::{
		Arc,
		atomic::{AtomicU8, AtomicU64, Ordering},
		mpsc::{self, SyncSender},
	},
	time::Duration,
};

#[derive(Default)]
pub struct Sounds {
	send: Option<SyncSender<(u64, Sound)>>,
	generation: Arc<AtomicU64>,
	status: Arc<AtomicU8>,
}
impl Sounds {
	pub fn status(&self) -> &'static str {
		match self.status.load(Ordering::Acquire) {
			1 => "Playing notification sound...",
			2 => "Audio output unavailable. Check your system sound settings.",
			_ => "",
		}
	}
	pub fn stop(&mut self) {
		self.generation.fetch_add(1, Ordering::AcqRel);
		self.status.store(0, Ordering::Release);
	}
	pub fn play(&mut self, sound: Sound, ctx: &eframe::egui::Context) {
		if self.send.is_none() {
			let (send, receive) = mpsc::sync_channel::<(u64, Sound)>(1);
			let generation = self.generation.clone();
			let status = self.status.clone();
			let context = ctx.clone();
			if std::thread::Builder::new()
				.name("serein-notification-audio".into())
				.spawn(move || {
					while let Ok((request, sound)) = receive.recv() {
						if generation.load(Ordering::Acquire) != request {
							continue;
						}
						match open(sound, generation.clone(), request, status.clone()) {
							Ok(stream) => {
								// Playback is under one second; cancellation silences the callback immediately.
								for _ in 0..50 {
									if generation.load(Ordering::Acquire) != request {
										break;
									}
									std::thread::sleep(Duration::from_millis(20));
								}
								drop(stream);
								if generation.load(Ordering::Acquire) == request {
									let _ = status.compare_exchange(
										1,
										0,
										Ordering::AcqRel,
										Ordering::Acquire,
									);
								}
							}
							Err(()) => {
								if generation.load(Ordering::Acquire) == request {
									status.store(2, Ordering::Release);
								}
							}
						}
						context.request_repaint();
					}
				})
				.is_err()
			{
				self.status.store(2, Ordering::Release);
				return;
			}
			self.send = Some(send);
		}
		let request = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
		self.status.store(1, Ordering::Release);
		if self
			.send
			.as_ref()
			.expect("worker started")
			.try_send((request, sound))
			.is_err()
		{
			self.status.store(0, Ordering::Release);
		}
	}
}
impl Drop for Sounds {
	fn drop(&mut self) {
		self.stop();
	}
}

fn samples(sound: Sound, rate: u32) -> Vec<f32> {
	let duration = if sound == Sound::IncomingRing {
		0.8
	} else {
		0.24
	};
	(0..(rate as f32 * duration) as usize)
		.map(|frame| {
			let time = frame as f32 / rate as f32;
			let (frequency, local, length) = match sound {
				Sound::IncomingRing => (660.0, time % 0.4, 0.26),
				Sound::CurrentChannel => (660.0, time, 0.24),
				Sound::Message => (if time < 0.12 { 784.0 } else { 1046.5 }, time % 0.12, 0.12),
			};
			let envelope = (local / 0.008).min(1.0) * ((length - local) / 0.035).clamp(0.0, 1.0);
			(time * frequency * std::f32::consts::TAU).sin() * envelope * 0.16
		})
		.collect()
}
fn open(
	sound: Sound,
	generation: Arc<AtomicU64>,
	request: u64,
	status: Arc<AtomicU8>,
) -> Result<cpal::Stream, ()> {
	let device = cpal::default_host().default_output_device().ok_or(())?;
	let supported = device.default_output_config().map_err(|_| ())?;
	let config = supported.config();
	if !(8000..=192000).contains(&config.sample_rate) || !(1..=8).contains(&config.channels) {
		return Err(());
	}
	let samples = samples(sound, config.sample_rate);
	let stream = match supported.sample_format() {
		cpal::SampleFormat::F32 => {
			output::<f32>(&device, config, samples, generation, request, status)
		}
		cpal::SampleFormat::I16 => {
			output::<i16>(&device, config, samples, generation, request, status)
		}
		cpal::SampleFormat::I32 => {
			output::<i32>(&device, config, samples, generation, request, status)
		}
		cpal::SampleFormat::U16 => {
			output::<u16>(&device, config, samples, generation, request, status)
		}
		_ => return Err(()),
	}
	.map_err(|_| ())?;
	stream.play().map_err(|_| ())?;
	Ok(stream)
}
fn output<T: cpal::SizedSample + cpal::FromSample<f32>>(
	device: &cpal::Device,
	config: cpal::StreamConfig,
	samples: Vec<f32>,
	generation: Arc<AtomicU64>,
	request: u64,
	status: Arc<AtomicU8>,
) -> Result<cpal::Stream, cpal::Error> {
	let mut position = 0;
	let errors = generation.clone();
	device.build_output_stream(
		config,
		move |data: &mut [T], _| {
			data.fill(T::from_sample(0.0));
			if generation.load(Ordering::Acquire) != request {
				return;
			}
			for frame in data.chunks_exact_mut(usize::from(config.channels)) {
				let Some(sample) = samples.get(position) else {
					break;
				};
				frame.fill(T::from_sample(*sample));
				position += 1;
			}
		},
		move |_| {
			if errors.load(Ordering::Acquire) == request {
				status.store(2, Ordering::Release);
			}
		},
		None,
	)
}
#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn cues_are_distinct_bounded_and_fade_to_silence() {
		let cues =
			[Sound::Message, Sound::CurrentChannel, Sound::IncomingRing].map(|s| samples(s, 48000));
		assert_ne!(cues[0], cues[1]);
		for cue in cues {
			assert!(cue.len() <= 38400);
			assert!(cue.iter().all(|s| s.is_finite() && s.abs() <= 0.16));
			assert!(cue.iter().any(|s| s.abs() > 0.1));
			assert!(cue.last().unwrap().abs() < 0.001);
		}
	}
}
