use crate::{audio::AudioBuffer, render::Renderer, Result};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    SampleFormat, Stream,
};
use rtrb::{Consumer, Producer, RingBuffer};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Arc,
    },
    thread::JoinHandle,
};

#[derive(Default)]
pub struct Telemetry {
    pub position: AtomicU64,
    pub playing: AtomicBool,
    pub device_failed: AtomicBool,
    pub cpu: AtomicU32,
    pub late_callbacks: AtomicU64,
    pub voice_overflows: AtomicU64,
    pub peaks: [AtomicU32; 4],
}
impl Telemetry {
    pub fn beats(&self) -> f64 {
        f64::from_bits(self.position.load(Ordering::Relaxed))
    }
    pub fn load(&self) -> f32 {
        f32::from_bits(self.cpu.load(Ordering::Relaxed))
    }
    pub fn peaks(&self) -> [f32; 4] {
        std::array::from_fn(|i| f32::from_bits(self.peaks[i].load(Ordering::Relaxed)))
    }
}
pub enum Message {
    Replace(Box<Renderer>),
    Start(f64),
    Stop,
    Locate(f64),
    Preview(usize, u8, u8),
    Select(Option<usize>),
}
pub struct DeviceEngine {
    _lifetime: std::sync::mpsc::Sender<()>,
    commands: Producer<Message>,
    retired: Consumer<Box<Renderer>>,
    pub telemetry: Arc<Telemetry>,
    pub sample_rate: u32,
    pub device_name: String,
}
impl DeviceEngine {
    pub fn open(renderer: impl FnOnce(u32) -> Result<Renderer> + Send + 'static) -> Result<Self> {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || match Self::open_local(renderer) {
            Ok((handle, stream, lifetime)) => {
                if tx.send(Ok(handle)).is_ok() {
                    let _ = lifetime.recv();
                }
                drop(stream);
            }
            Err(error) => {
                let _ = tx.send(Err(error));
            }
        });
        rx.recv()
            .map_err(|_| "Audio device worker stopped".to_string())?
    }
    fn open_local(
        renderer: impl FnOnce(u32) -> Result<Renderer>,
    ) -> Result<(Self, Stream, std::sync::mpsc::Receiver<()>)> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("No output device. Connect one and choose Audio > Reconnect.")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let config = supported.config();
        let sample_rate = config.sample_rate.0;
        let name = device.name().unwrap_or_else(|_| "Default output".into());
        let renderer = Box::new(renderer(sample_rate)?);
        let (commands, input) = RingBuffer::new(64);
        let (garbage, retired) = RingBuffer::new(64);
        let telemetry = Arc::new(Telemetry::default());
        let rt = Callback {
            renderer,
            fade: None,
            fade_remaining: 0,
            input,
            garbage,
            telemetry: telemetry.clone(),
        };
        let stream = match supported.sample_format() {
            SampleFormat::F32 => output::<f32>(&device, &config, rt),
            SampleFormat::I16 => output::<i16>(&device, &config, rt),
            SampleFormat::U16 => output::<u16>(&device, &config, rt),
            SampleFormat::I32 => output::<i32>(&device, &config, rt),
            SampleFormat::F64 => output::<f64>(&device, &config, rt),
            other => Err(format!("Unsupported device sample format: {other}")),
        }?;
        stream.play().map_err(|e| e.to_string())?;
        let (lifetime, closed) = std::sync::mpsc::channel();
        Ok((
            Self {
                _lifetime: lifetime,
                commands,
                retired,
                telemetry,
                sample_rate,
                device_name: name,
            },
            stream,
            closed,
        ))
    }
    pub fn collect(&mut self) {
        while let Ok(old) = self.retired.pop() {
            drop(old);
        }
    }
    pub fn send(&mut self, msg: Message) -> Result<()> {
        self.collect();
        self.commands
            .push(msg)
            .map_err(|_| "Audio command queue is full; wait and retry".to_string())
    }
}
struct Callback {
    renderer: Box<Renderer>,
    fade: Option<Box<Renderer>>,
    fade_remaining: u32,
    input: Consumer<Message>,
    garbage: Producer<Box<Renderer>>,
    telemetry: Arc<Telemetry>,
}
impl Callback {
    fn commands(&mut self) {
        if self.fade.is_some() {
            return;
        }
        // Bound work per callback; never destroy an old graph on this thread.
        for _ in 0..32 {
            if self.garbage.is_full() {
                break;
            }
            let Ok(message) = self.input.pop() else {
                break;
            };
            match message {
                Message::Replace(mut new) => {
                    new.locate(self.renderer.position());
                    new.playing = self.renderer.playing;
                    std::mem::swap(&mut self.renderer, &mut new);
                    self.fade_remaining = (self.renderer.rate() / 200).max(1);
                    self.fade = Some(new);
                    break;
                }
                Message::Start(beats) => {
                    self.renderer.locate(beats);
                    self.renderer.playing = true;
                }
                Message::Stop => self.renderer.stop(),
                Message::Locate(beats) => self.renderer.locate(beats),
                Message::Preview(track, pitch, velocity) => {
                    self.renderer.preview(track, pitch, velocity)
                }
                Message::Select(track) => self.renderer.set_selected(track),
            }
        }
    }
}
fn output<T: cpal::SizedSample + cpal::FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut rt: Callback,
) -> Result<Stream> {
    let channels = config.channels as usize;
    if channels == 0 {
        return Err("Output device has no channels".into());
    }
    let rate = config.sample_rate.0 as f64;
    let failure = rt.telemetry.clone();
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                let started = std::time::Instant::now();
                rt.commands();
                rt.renderer.begin_block();
                for frame in data.chunks_mut(channels) {
                    let mut stereo = rt.renderer.next_frame();
                    if let Some(old) = rt.fade.as_mut() {
                        let previous = old.next_frame();
                        let blend =
                            rt.fade_remaining as f32 / (rt.renderer.rate() / 200).max(1) as f32;
                        for c in 0..2 {
                            stereo[c] = stereo[c] * (1.0 - blend) + previous[c] * blend;
                        }
                        rt.fade_remaining = rt.fade_remaining.saturating_sub(1);
                        if rt.fade_remaining == 0 {
                            // commands() reserves this slot before starting a crossfade.
                            if let Some(old) = rt.fade.take() {
                                let _ = rt.garbage.push(old);
                            }
                        }
                    }
                    for (c, sample) in frame.iter_mut().enumerate() {
                        let value = if channels == 1 {
                            (stereo[0] + stereo[1]) * 0.5
                        } else if c < 2 {
                            stereo[c]
                        } else {
                            0.0
                        };
                        *sample = T::from_sample(value);
                    }
                }
                let budget = data.len() as f64 / channels as f64 / rate;
                let load = (started.elapsed().as_secs_f64() / budget.max(1e-6)) as f32;
                rt.telemetry.cpu.store(load.to_bits(), Ordering::Relaxed);
                if load > 1.0 {
                    rt.telemetry.late_callbacks.fetch_add(1, Ordering::Relaxed);
                }
                rt.telemetry
                    .position
                    .store(rt.renderer.position().to_bits(), Ordering::Relaxed);
                rt.telemetry
                    .playing
                    .store(rt.renderer.playing, Ordering::Relaxed);
                rt.telemetry
                    .voice_overflows
                    .store(rt.renderer.voice_overflows, Ordering::Relaxed);
                let peaks = [
                    rt.renderer.peak[0],
                    rt.renderer.peak[1],
                    rt.renderer.channel_peak[0],
                    rt.renderer.channel_peak[1],
                ];
                for (target, value) in rt.telemetry.peaks.iter().zip(peaks) {
                    target.store(value.to_bits(), Ordering::Relaxed);
                }
            },
            move |_| {
                failure.device_failed.store(true, Ordering::Relaxed);
            },
            None,
        )
        .map_err(|e| e.to_string())
}

/// Sendable control handle; the platform stream never leaves its owning worker.
pub struct Recorder {
    stop: Option<std::sync::mpsc::Sender<()>>,
    result: std::sync::mpsc::Receiver<Result<AudioBuffer>>,
    pub failed: Arc<AtomicBool>,
    pub first_beat: Arc<AtomicU64>,
}
impl Recorder {
    pub fn start(telemetry: Arc<Telemetry>) -> Result<Self> {
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || match LocalRecorder::start(telemetry) {
            Ok(local) => {
                let (stop_tx, stop_rx) = std::sync::mpsc::channel();
                let (result_tx, result_rx) = std::sync::mpsc::sync_channel(1);
                let handle = Self {
                    stop: Some(stop_tx),
                    result: result_rx,
                    failed: local.failed.clone(),
                    first_beat: local.first_beat.clone(),
                };
                if ready_tx.send(Ok(handle)).is_ok() {
                    let _ = stop_rx.recv();
                }
                let _ = result_tx.send(local.finish());
            }
            Err(error) => {
                let _ = ready_tx.send(Err(error));
            }
        });
        ready_rx
            .recv()
            .map_err(|_| "Microphone worker stopped".to_string())?
    }
    pub fn finish(mut self) -> Result<AudioBuffer> {
        self.stop.take();
        self.result
            .recv_timeout(std::time::Duration::from_secs(10))
            .map_err(|_| "Microphone did not finish the take in time".to_string())?
    }
}
struct LocalRecorder {
    stream: Option<Stream>,
    stop: Arc<AtomicBool>,
    pub failed: Arc<AtomicBool>,
    join: Option<JoinHandle<Result<AudioBuffer>>>,
    pub first_beat: Arc<AtomicU64>,
}
impl LocalRecorder {
    pub fn start(telemetry: Arc<Telemetry>) -> Result<Self> {
        let device = cpal::default_host()
            .default_input_device()
            .ok_or("No input device available")?;
        let supported = device.default_input_config().map_err(|e| e.to_string())?;
        let config = supported.config();
        let rate = config.sample_rate.0;
        let (producer, mut consumer) = RingBuffer::<[f32; 2]>::new(rate as usize * 2);
        let stop = Arc::new(AtomicBool::new(false));
        let failed = Arc::new(AtomicBool::new(false));
        let first_beat = Arc::new(AtomicU64::new(f64::NAN.to_bits()));
        let state = Capture {
            producer,
            telemetry,
            failed: failed.clone(),
            first_beat: first_beat.clone(),
            first: true,
        };
        let stream = match supported.sample_format() {
            SampleFormat::F32 => input::<f32>(&device, &config, state),
            SampleFormat::I16 => input::<i16>(&device, &config, state),
            SampleFormat::U16 => input::<u16>(&device, &config, state),
            SampleFormat::I32 => input::<i32>(&device, &config, state),
            SampleFormat::F64 => input::<f64>(&device, &config, state),
            other => Err(format!("Unsupported input format: {other}")),
        }?;
        stream.play().map_err(|e| e.to_string())?;
        let worker_stop = stop.clone();
        let worker_failed = failed.clone();
        let join = std::thread::spawn(move || {
            let mut frames = Vec::new();
            loop {
                while let Ok(frame) = consumer.pop() {
                    if frames.len() >= crate::audio::MAX_AUDIO_BYTES / 8 {
                        worker_failed.store(true, Ordering::Relaxed);
                        return Err("Take reached the 512 MiB recording limit".into());
                    }
                    frames.push(frame);
                }
                if worker_stop.load(Ordering::Acquire) && consumer.is_empty() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            if worker_failed.load(Ordering::Relaxed) {
                return Err(
                    "Recording interrupted: input disconnected or recording buffer overrun".into(),
                );
            }
            AudioBuffer::new(rate, frames)
        });
        Ok(Self {
            stream: Some(stream),
            stop,
            failed,
            join: Some(join),
            first_beat,
        })
    }
    pub fn finish(mut self) -> Result<AudioBuffer> {
        self.stream.take();
        self.stop.store(true, Ordering::Release);
        self.join
            .take()
            .ok_or("Recording already finished")?
            .join()
            .map_err(|_| "Recording worker failed")?
    }
}
impl Drop for LocalRecorder {
    fn drop(&mut self) {
        self.stream.take();
        self.stop.store(true, Ordering::Release);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
struct Capture {
    producer: Producer<[f32; 2]>,
    telemetry: Arc<Telemetry>,
    failed: Arc<AtomicBool>,
    first_beat: Arc<AtomicU64>,
    first: bool,
}
fn input<T: cpal::SizedSample>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut capture: Capture,
) -> Result<Stream>
where
    f32: cpal::FromSample<T>,
{
    let channels = config.channels as usize;
    if channels == 0 {
        return Err("Input device has no channels".into());
    }
    let failure = capture.failed.clone();
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                if capture.first {
                    capture.first_beat.store(
                        capture.telemetry.position.load(Ordering::Relaxed),
                        Ordering::Relaxed,
                    );
                    capture.first = false;
                }
                for frame in data.chunks_exact(channels) {
                    let l = <f32 as cpal::FromSample<T>>::from_sample_(frame[0]);
                    let r = <f32 as cpal::FromSample<T>>::from_sample_(
                        frame[if channels > 1 { 1 } else { 0 }],
                    );
                    if capture.producer.push([l, r]).is_err() {
                        capture.failed.store(true, Ordering::Relaxed);
                    }
                }
            },
            move |_| {
                failure.store(true, Ordering::Relaxed);
            },
            None,
        )
        .map_err(|e| e.to_string())
}
