//! Audio device workers. The output callback owns the renderer and the plugin
//! rack; everything else talks to it through bounded lock-free queues.

use crate::{
    audio::AudioBuffer,
    plugin::{Processor, Rack, MAX_BLOCK},
    render::Renderer,
    Result,
};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    SampleFormat, Stream,
};
use rtrb::{Consumer, Producer, RingBuffer};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread::JoinHandle,
};

pub const RACK_SLOTS: usize = 1024;

#[derive(Default)]
pub struct Telemetry {
    pub position: AtomicU64,
    pub playing: AtomicBool,
    pub device_failed: AtomicBool,
    pub input_overflow: AtomicBool,
    /// True while the renderer is clicking a count-in before the transport starts.
    pub counting_in: AtomicBool,
    /// Peak of the microphone input since the last read, 0-1 as f32 bits.
    pub input_peak: AtomicU32,
    pub cpu: AtomicU32,
    pub late_callbacks: AtomicU64,
    pub voice_overflows: AtomicU64,
    pub peaks: [AtomicU32; 4],
    pub track_peaks: [AtomicU32; crate::render::METER_TRACKS],
    /// Frames in the most recent input and output device callbacks, and the input's rate.
    pub input_frames: AtomicU32,
    pub output_frames: AtomicU32,
    pub input_rate: AtomicU32,
    /// True while the output callback holds a live monitor tap.
    pub monitoring: AtomicBool,
    /// Input frames waiting in the monitor ring when the output last read it.
    pub monitor_fill: AtomicU32,
    /// Monitor frames thrown away: the ring was full, or a backlog was skipped to catch up.
    pub monitor_drops: AtomicU64,
    /// Times the output wanted monitor input and the ring was empty.
    pub monitor_underruns: AtomicU64,
}
impl Telemetry {
    /// Input buffer + frames waiting in the ring + output buffer, in milliseconds. Converter
    /// and driver latency inside the interface comes on top and is not visible from here.
    pub fn monitor_latency_ms(&self, output_rate: u32) -> Option<f64> {
        let input_rate = self.input_rate.load(Ordering::Relaxed);
        if !self.monitoring.load(Ordering::Relaxed) || input_rate == 0 || output_rate == 0 {
            return None;
        }
        let input = (self.input_frames.load(Ordering::Relaxed)
            + self.monitor_fill.load(Ordering::Relaxed)) as f64
            / input_rate as f64;
        let output = self.output_frames.load(Ordering::Relaxed) as f64 / output_rate as f64;
        Some((input + output) * 1000.0)
    }
    pub fn beats(&self) -> f64 {
        f64::from_bits(self.position.load(Ordering::Relaxed))
    }
    pub fn load(&self) -> f32 {
        f32::from_bits(self.cpu.load(Ordering::Relaxed))
    }
    /// The loudest input sample since the last call; reading resets it.
    pub fn take_input_peak(&self) -> f32 {
        f32::from_bits(self.input_peak.swap(0, Ordering::Relaxed))
    }
    pub fn peaks(&self) -> [f32; 4] {
        std::array::from_fn(|i| f32::from_bits(self.peaks[i].load(Ordering::Relaxed)))
    }
    pub fn track_peaks(&self) -> [f32; crate::render::METER_TRACKS] {
        std::array::from_fn(|i| f32::from_bits(self.track_peaks[i].load(Ordering::Relaxed)))
    }
}
pub enum Message {
    Replace(Box<Renderer>),
    Start(f64),
    Stop,
    Locate(f64),
    Preview(usize, u8, u8),
    Note {
        track: usize,
        on: bool,
        pitch: u8,
        velocity: u8,
    },
    RoutedNote {
        route: usize,
        on: bool,
        pitch: u8,
        velocity: u8,
    },
    Select(Option<usize>),
    Mount(u32, Box<dyn Processor>),
    Unmount(u32),
    UnmountAll,
    SetParam(u32, u32, f64),
    ResetSlot(u32),
    SetRecording(bool),
    /// Park at the first position, click for the second number of beats, then play.
    CountIn(f64, f64),
    /// Live input for monitored tracks; replaces any earlier tap.
    Monitor(Box<MonitorTap>),
}
/// Objects the callback no longer needs; the main thread reclaims them.
pub enum Retired {
    Renderer(Box<Renderer>),
    Processor(u32, Box<dyn Processor>),
    Monitor(Box<MonitorTap>),
}

/// Frames the monitor ring holds: a third of a second at 48 kHz, room for four 4096-frame
/// device buffers. It is a latency bound as much as a capacity.
const MONITOR_RING_FRAMES: usize = 16_384;

/// Input-callback end of the monitor ring.
pub struct MonitorFeed(Producer<[f32; 2]>);
impl MonitorFeed {
    /// Never blocks: a full ring drops the frame and counts it.
    pub fn push(&mut self, frame: [f32; 2], telemetry: &Telemetry) -> bool {
        if self.0.push(frame).is_ok() {
            return true;
        }
        telemetry.monitor_drops.fetch_add(1, Ordering::Relaxed);
        false
    }
    /// The output side closed or replaced its tap.
    pub fn is_abandoned(&self) -> bool {
        self.0.is_abandoned()
    }
}
/// Output-callback end: resamples the input to the output rate and keeps the ring short.
pub struct MonitorTap {
    consumer: Consumer<[f32; 2]>,
    /// Input frames per output frame.
    step: f64,
    phase: f64,
    previous: [f32; 2],
    current: [f32; 2],
    running: bool,
}
/// A bounded ring from an input stream to the output callback. Different rates are bridged by
/// linear interpolation in the tap; rates no device should report are refused.
pub fn monitor_ring(input_rate: u32, output_rate: u32) -> Result<(MonitorFeed, MonitorTap)> {
    for (name, rate) in [("Input", input_rate), ("Output", output_rate)] {
        if !(8000..=192_000).contains(&rate) {
            return Err(format!(
                "{name} sample rate {rate} Hz cannot be monitored; choose a rate between 8 and 192 kHz"
            ));
        }
    }
    let (producer, consumer) = RingBuffer::new(MONITOR_RING_FRAMES);
    Ok((
        MonitorFeed(producer),
        MonitorTap {
            consumer,
            step: input_rate as f64 / output_rate as f64,
            phase: 1.0,
            previous: [0.0; 2],
            current: [0.0; 2],
            running: false,
        },
    ))
}
impl MonitorTap {
    pub fn buffered(&self) -> usize {
        self.consumer.slots()
    }
    /// The input stream closed.
    pub fn is_finished(&self) -> bool {
        self.consumer.is_abandoned() && self.consumer.is_empty()
    }
    /// Fill one output block. Waits for one input buffer before starting (and again after an
    /// underrun), and skips a backlog rather than let the delay grow. No allocation or locks.
    pub fn fill(&mut self, out: &mut [[f32; 2]], telemetry: &Telemetry) {
        let block = match telemetry.input_frames.load(Ordering::Relaxed) as usize {
            0 => 512,
            frames => frames.min(MONITOR_RING_FRAMES / 8),
        };
        let needed = (out.len() as f64 * self.step).ceil() as usize;
        let mut buffered = self.consumer.slots();
        if buffered > block * 4 + needed {
            let skip = buffered - block;
            if let Ok(chunk) = self.consumer.read_chunk(skip) {
                chunk.commit_all();
                telemetry
                    .monitor_drops
                    .fetch_add(skip as u64, Ordering::Relaxed);
                buffered -= skip;
            }
        }
        telemetry
            .monitor_fill
            .store(buffered as u32, Ordering::Relaxed);
        if !self.running {
            out.fill([0.0; 2]);
            if buffered >= block {
                // Start on the next callback: that spacing is the slack against a late input.
                self.running = true;
                self.phase = 1.0;
                self.previous = [0.0; 2];
                self.current = [0.0; 2];
            }
            return;
        }
        for (index, frame) in out.iter_mut().enumerate() {
            while self.phase >= 1.0 {
                match self.consumer.pop() {
                    Ok(next) => {
                        self.previous = self.current;
                        self.current = next;
                        self.phase -= 1.0;
                    }
                    Err(_) => {
                        telemetry.monitor_underruns.fetch_add(1, Ordering::Relaxed);
                        self.running = false;
                        out[index..].fill([0.0; 2]);
                        return;
                    }
                }
            }
            let t = self.phase as f32;
            *frame = [
                self.previous[0] + (self.current[0] - self.previous[0]) * t,
                self.previous[1] + (self.current[1] - self.previous[1]) * t,
            ];
            self.phase += self.step;
        }
    }
}
/// A clonable handle for pushing messages from other threads (MIDI input).
#[derive(Clone)]
pub struct Sender(Arc<Mutex<Producer<Message>>>);
impl Sender {
    pub fn try_send(&self, message: Message) -> std::result::Result<(), Message> {
        match self.0.lock() {
            Ok(mut queue) => queue
                .push(message)
                .map_err(|rtrb::PushError::Full(message)| message),
            Err(_) => Err(message),
        }
    }
    pub fn send(&self, message: Message) -> Result<()> {
        self.try_send(message)
            .map_err(|_| "Audio command queue is full or unavailable; wait and retry".to_string())
    }
}
/// The requested frames per buffer, held to what the device accepts.
fn buffer_size(supported: &cpal::SupportedBufferSize, wanted: Option<u32>) -> cpal::BufferSize {
    match (supported, wanted) {
        (cpal::SupportedBufferSize::Range { min, max }, Some(frames)) if min <= max => {
            cpal::BufferSize::Fixed(frames.clamp(*min, *max))
        }
        _ => cpal::BufferSize::Default,
    }
}
pub struct DeviceEngine {
    lifetime: Option<std::sync::mpsc::Sender<()>>,
    worker: Option<JoinHandle<()>>,
    commands: Sender,
    retired: Consumer<Retired>,
    pub telemetry: Arc<Telemetry>,
    pub sample_rate: u32,
    pub device_name: String,
}
/// Output and input device names known to the system backend.
pub fn output_devices() -> Vec<String> {
    cpal::default_host()
        .output_devices()
        .map(|d| d.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default()
}
pub fn input_devices() -> Vec<String> {
    cpal::default_host()
        .input_devices()
        .map(|d| d.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default()
}
impl DeviceEngine {
    /// Open the named output (or the system default) on a dedicated worker.
    pub fn open(
        device: Option<String>,
        buffer_frames: Option<u32>,
        renderer: impl FnOnce(u32) -> Result<Renderer> + Send + 'static,
    ) -> Result<Self> {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let worker =
            std::thread::spawn(
                move || match Self::open_local(device, buffer_frames, renderer) {
                    Ok((handle, stream, lifetime)) => {
                        if tx.send(Ok(handle)).is_ok() {
                            let _ = lifetime.recv();
                        }
                        drop(stream);
                    }
                    Err(error) => {
                        let _ = tx.send(Err(error));
                    }
                },
            );
        let mut engine = rx
            .recv()
            .map_err(|_| "Audio device worker stopped".to_string())??;
        engine.worker = Some(worker);
        Ok(engine)
    }
    fn open_local(
        name: Option<String>,
        buffer_frames: Option<u32>,
        renderer: impl FnOnce(u32) -> Result<Renderer>,
    ) -> Result<(Self, Stream, std::sync::mpsc::Receiver<()>)> {
        let host = cpal::default_host();
        let device = match &name {
            Some(wanted) => host
                .output_devices()
                .map_err(|e| e.to_string())?
                .find(|d| d.name().is_ok_and(|n| &n == wanted))
                .ok_or_else(|| format!("Output device not found: {wanted}"))?,
            None => host
                .default_output_device()
                .ok_or("No output device. Connect one and choose Audio > Reconnect.")?,
        };
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let mut config = supported.config();
        config.buffer_size = buffer_size(supported.buffer_size(), buffer_frames);
        let sample_rate = config.sample_rate.0;
        let name = device.name().unwrap_or_else(|_| "Default output".into());
        let renderer = Box::new(renderer(sample_rate)?);
        let (commands, input) = RingBuffer::new(256);
        // Leave room for every live processor during device teardown, plus
        // objects already retired by queued graph changes.
        let (garbage, retired) = RingBuffer::new(RACK_SLOTS * 2 + 256);
        let telemetry = Arc::new(Telemetry::default());
        let rt = Callback {
            renderer,
            rack: Rack::new(RACK_SLOTS),
            input,
            garbage,
            telemetry: telemetry.clone(),
            scratch: vec![[0.0; 2]; MAX_BLOCK],
            counting: false,
            monitor: None,
            live: vec![[0.0; 2]; MAX_BLOCK],
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
                lifetime: Some(lifetime),
                worker: None,
                commands: Sender(Arc::new(Mutex::new(commands))),
                retired,
                telemetry,
                sample_rate,
                device_name: name,
            },
            stream,
            closed,
        ))
    }
    /// Reclaim retired graphs and processors on the caller's (main) thread.
    pub fn collect(&mut self) -> Vec<Retired> {
        let mut out = vec![];
        while let Ok(old) = self.retired.pop() {
            out.push(old);
        }
        out
    }
    pub fn sender(&self) -> Sender {
        self.commands.clone()
    }
    pub fn send(&mut self, msg: Message) -> Result<()> {
        self.commands.send(msg)
    }
    pub fn try_send(&mut self, msg: Message) -> std::result::Result<(), Message> {
        self.commands.try_send(msg)
    }
}
impl Drop for DeviceEngine {
    /// Stop the stream and wait for its worker so plugin processors it still
    /// holds are released before their editors are destroyed on this thread.
    fn drop(&mut self) {
        self.lifetime.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
struct Callback {
    renderer: Box<Renderer>,
    rack: Rack,
    input: Consumer<Message>,
    garbage: Producer<Retired>,
    telemetry: Arc<Telemetry>,
    scratch: Vec<[f32; 2]>,
    /// The renderer was counting in during the previous callback.
    counting: bool,
    monitor: Option<Box<MonitorTap>>,
    live: Vec<[f32; 2]>,
}
impl Callback {
    /// Retire a tap whose input stream has closed; the main thread frees it.
    fn reap_monitor(&mut self) {
        if self.monitor.as_ref().is_some_and(|tap| tap.is_finished()) && !self.garbage.is_full() {
            if let Some(tap) = self.monitor.take() {
                let _ = self.garbage.push(Retired::Monitor(tap));
            }
            self.telemetry.monitoring.store(false, Ordering::Relaxed);
            self.telemetry.monitor_fill.store(0, Ordering::Relaxed);
        }
    }
    /// Render `n` frames into `scratch`, with the live input when a track monitors it.
    fn render(&mut self, n: usize) {
        match self.monitor.as_mut() {
            Some(tap) => {
                tap.fill(&mut self.live[..n], &self.telemetry);
                if self.renderer.wants_input() {
                    self.renderer.render_monitored(
                        &mut self.rack,
                        &mut self.scratch[..n],
                        &self.live[..n],
                    );
                } else {
                    self.renderer.render(&mut self.rack, &mut self.scratch[..n]);
                }
            }
            None => self.renderer.render(&mut self.rack, &mut self.scratch[..n]),
        }
    }
    fn commands(&mut self) {
        let panic = self.telemetry.input_overflow.swap(false, Ordering::AcqRel);
        // Bound work per callback; never destroy an old graph or plugin on this thread.
        for _ in 0..if panic { 256 } else { 64 } {
            if self.garbage.slots() < 2 {
                break;
            }
            let Ok(message) = self.input.pop() else {
                break;
            };
            if panic
                && matches!(
                    message,
                    Message::Note { .. }
                        | Message::RoutedNote { .. }
                        | Message::Preview(..)
                        | Message::Start(..)
                        | Message::CountIn(..)
                )
            {
                continue;
            }
            match message {
                Message::Replace(mut new) => {
                    new.adopt(&self.renderer);
                    std::mem::swap(&mut self.renderer, &mut new);
                    let _ = self.garbage.push(Retired::Renderer(new));
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
                Message::Note {
                    track,
                    on,
                    pitch,
                    velocity,
                } => self.renderer.note(track, on, pitch, velocity),
                Message::RoutedNote {
                    route,
                    on,
                    pitch,
                    velocity,
                } => self.renderer.routed_note(route, on, pitch, velocity),
                Message::Select(track) => self.renderer.set_selected(track),
                Message::Mount(slot, processor) => {
                    if let Some(old) = self.rack.mount(slot, processor) {
                        let _ = self.garbage.push(Retired::Processor(slot, old));
                    }
                }
                Message::Unmount(slot) => {
                    if let Some(old) = self.rack.unmount(slot) {
                        let _ = self.garbage.push(Retired::Processor(slot, old));
                    }
                }
                Message::UnmountAll => {
                    for slot in 0..self.rack.capacity() as u32 {
                        if self.garbage.slots() == 0 {
                            break;
                        }
                        if let Some(old) = self.rack.unmount(slot) {
                            let _ = self.garbage.push(Retired::Processor(slot, old));
                        }
                    }
                }
                Message::SetParam(slot, id, value) => self.rack.set_param(slot, id, value),
                Message::ResetSlot(slot) => self.rack.reset(slot),
                Message::SetRecording(on) => self.renderer.recording = on,
                Message::CountIn(beats, count) => self.renderer.count_in(beats, count),
                Message::Monitor(tap) => {
                    if let Some(old) = self.monitor.replace(tap) {
                        let _ = self.garbage.push(Retired::Monitor(old));
                    }
                    self.telemetry.monitoring.store(true, Ordering::Relaxed);
                }
            }
        }
        if panic {
            self.renderer.stop();
            if !self.input.is_empty() {
                self.telemetry.input_overflow.store(true, Ordering::Release);
            }
        }
    }
}
impl Drop for Callback {
    fn drop(&mut self) {
        for processor in self.rack.drain() {
            let _ = self.garbage.push(Retired::Processor(u32::MAX, processor));
        }
        if let Some(tap) = self.monitor.take() {
            let _ = self.garbage.push(Retired::Monitor(tap));
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
                rt.reap_monitor();
                rt.renderer.begin_block();
                let frames = data.len() / channels;
                rt.telemetry
                    .output_frames
                    .store(frames as u32, Ordering::Relaxed);
                let mut done = 0;
                while done < frames {
                    let n = (frames - done).min(MAX_BLOCK);
                    rt.render(n);
                    for (i, frame) in data[done * channels..(done + n) * channels]
                        .chunks_mut(channels)
                        .enumerate()
                    {
                        let stereo = rt.scratch[i];
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
                    done += n;
                }
                let budget = frames as f64 / rate;
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
                // The window raises the flag when it asks for a count-in, so a recorder that
                // opens before this callback sees it. Only the end of the count clears it.
                let counting = rt.renderer.counting_in();
                if rt.counting && !counting {
                    rt.telemetry.counting_in.store(false, Ordering::Relaxed);
                }
                rt.counting = counting;
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
                for (target, value) in rt.telemetry.track_peaks.iter().zip(rt.renderer.track_peaks)
                {
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
#[derive(Debug)]
pub struct RecordedAudio {
    pub buffer: AudioBuffer,
    /// Successfully captured samples are retained when capture is interrupted.
    pub warning: Option<String>,
    pub recovery_path: Option<std::path::PathBuf>,
}
impl RecordedAudio {
    /// Preserve source audio independently of project placement or saving.
    pub fn preserve(&mut self, path: &std::path::Path) -> Result<()> {
        preserve_recording(&self.buffer, path)?;
        self.recovery_path = Some(path.into());
        Ok(())
    }
}
pub fn preserve_recording(buffer: &AudioBuffer, path: &std::path::Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    crate::document::atomic_write(path, |file| {
        let mut writer = hound::WavWriter::new(
            std::io::BufWriter::new(file),
            hound::WavSpec {
                channels: 2,
                sample_rate: buffer.sample_rate,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
        )
        .map_err(|e| e.to_string())?;
        for sample in buffer.frames.iter().flatten() {
            writer.write_sample(*sample).map_err(|e| e.to_string())?;
        }
        writer.finalize().map_err(|e| e.to_string())
    })
}
pub struct Recorder {
    stop: Option<std::sync::mpsc::Sender<()>>,
    result: std::sync::mpsc::Receiver<Result<RecordedAudio>>,
    pub failed: Arc<AtomicBool>,
    pub first_beat: Arc<AtomicU64>,
    pub monitoring: Monitoring,
}
impl Recorder {
    pub fn start(
        telemetry: Arc<Telemetry>,
        input: Option<String>,
        byte_limit: usize,
        monitor: Option<MonitorLink>,
        buffer_frames: Option<u32>,
    ) -> Result<Self> {
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            match LocalRecorder::start(telemetry, input, byte_limit, monitor, buffer_frames) {
                Ok(local) => {
                    let (stop_tx, stop_rx) = std::sync::mpsc::channel();
                    let (result_tx, result_rx) = std::sync::mpsc::sync_channel(1);
                    let handle = Self {
                        stop: Some(stop_tx),
                        result: result_rx,
                        failed: local.failed.clone(),
                        first_beat: local.first_beat.clone(),
                        monitoring: local.monitoring.clone(),
                    };
                    if ready_tx.send(Ok(handle)).is_ok() {
                        let _ = stop_rx.recv();
                    }
                    let _ = result_tx.send(local.finish());
                }
                Err(error) => {
                    let _ = ready_tx.send(Err(error));
                }
            }
        });
        ready_rx
            .recv()
            .map_err(|_| "Microphone worker stopped".to_string())?
    }
    pub fn finish(mut self) -> Result<RecordedAudio> {
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
    join: Option<JoinHandle<Result<RecordedAudio>>>,
    pub first_beat: Arc<AtomicU64>,
    monitoring: Monitoring,
}
impl LocalRecorder {
    pub fn start(
        telemetry: Arc<Telemetry>,
        input: Option<String>,
        byte_limit: usize,
        monitor: Option<MonitorLink>,
        buffer_frames: Option<u32>,
    ) -> Result<Self> {
        let device = find_input(&input)?;
        let supported = device.default_input_config().map_err(|e| e.to_string())?;
        let config = supported.config();
        let rate = config.sample_rate.0;
        let peak_chunk = (rate / 400).max(1) as usize;
        let maximum_frames =
            (byte_limit / (8 * peak_chunk + 4) * peak_chunk).min(crate::audio::MAX_AUDIO_BYTES / 8);
        if maximum_frames == 0 {
            return Err("No decoded-audio capacity remains for a recording".into());
        }
        let (producer, consumer) = RingBuffer::<[f32; 2]>::new(rate as usize * 2);
        let stop = Arc::new(AtomicBool::new(false));
        let failed = Arc::new(AtomicBool::new(false));
        let first_beat = Arc::new(AtomicU64::new(f64::NAN.to_bits()));
        telemetry.input_rate.store(rate, Ordering::Relaxed);
        let (feed, monitoring) = connect_monitor(monitor.as_ref(), &device, rate);
        let state = Capture {
            producer: Some(producer),
            telemetry,
            failed: failed.clone(),
            first_beat: first_beat.clone(),
            first: true,
            monitor: feed,
        };
        let stream = capture_stream(&device, &supported, buffer_frames, state)?;
        stream.play().map_err(|e| e.to_string())?;
        let worker_stop = stop.clone();
        let worker_failed = failed.clone();
        let join = std::thread::spawn(move || {
            collect_recording(consumer, &worker_stop, &worker_failed, rate, maximum_frames)
        });
        Ok(Self {
            stream: Some(stream),
            stop,
            failed,
            join: Some(join),
            first_beat,
            monitoring,
        })
    }
    pub fn finish(mut self) -> Result<RecordedAudio> {
        self.stream.take();
        self.stop.store(true, Ordering::Release);
        self.join
            .take()
            .ok_or("Recording already finished")?
            .join()
            .map_err(|_| "Recording worker failed")?
    }
}
fn collect_recording(
    mut consumer: Consumer<[f32; 2]>,
    stop: &AtomicBool,
    failed: &AtomicBool,
    rate: u32,
    maximum_frames: usize,
) -> Result<RecordedAudio> {
    let mut frames = Vec::new();
    let mut warning = None;
    'capture: loop {
        while let Ok(frame) = consumer.pop() {
            if frames.len() >= maximum_frames {
                failed.store(true, Ordering::Relaxed);
                warning = Some("Recording reached its memory limit. The partial take was recovered; save it before continuing.".into());
                break 'capture;
            }
            frames.push(frame);
        }
        if (stop.load(Ordering::Acquire) || failed.load(Ordering::Relaxed)) && consumer.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    if failed.load(Ordering::Relaxed) && warning.is_none() {
        warning = Some("Recording was interrupted by an input disconnect or buffer overrun. The partial take was recovered; save it before continuing.".into());
    }
    if frames.is_empty() {
        return Err("Recording ended before any audio was captured".into());
    }
    Ok(RecordedAudio {
        buffer: AudioBuffer::new(rate, frames)?,
        warning,
        recovery_path: None,
    })
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
/// What an input worker needs to feed monitored tracks: where the output callback listens and
/// what it plays on. The ring is made on the worker, once the input's own rate is known.
#[derive(Clone)]
pub struct MonitorLink {
    pub sender: Sender,
    pub output_rate: u32,
    pub output_name: String,
    /// The user accepted monitoring from the built-in microphone to the built-in speakers.
    pub allow_speakers: bool,
}
/// How an input stream answered a request to monitor.
#[derive(Clone, Debug, PartialEq)]
pub enum Monitoring {
    Off,
    On {
        input_name: String,
        input_rate: u32,
    },
    /// Built-in microphone into built-in speakers: it would howl. Nothing is routed.
    FeedbackRisk {
        input_name: String,
    },
    Failed(String),
}
/// A laptop's own microphone playing out of its own speakers feeds back within a second.
/// Headphones on the jack appear under another name ("External Headphones"), so names decide.
pub fn feedback_risk(input_name: &str, output_name: &str) -> bool {
    let input = input_name.to_lowercase();
    let output = output_name.to_lowercase();
    let internal = |name: &str| {
        [
            "macbook",
            "built-in",
            "builtin",
            "internal",
            "imac",
            "realtek",
            "microphone array",
        ]
        .iter()
        .any(|word| name.contains(word))
    };
    let microphone = internal(&input) && (input.contains("mic") || input.contains("input"));
    let speakers = internal(&output)
        && (output.contains("speaker") || output.contains("output"))
        && !output.contains("headphone");
    microphone && speakers
}
impl MonitorLink {
    /// Make the ring and hand its tap to the output callback, unless it would feed back.
    fn connect(&self, device: &cpal::Device, input_rate: u32) -> (Option<MonitorFeed>, Monitoring) {
        let input_name = device.name().unwrap_or_else(|_| "Default input".into());
        if !self.allow_speakers && feedback_risk(&input_name, &self.output_name) {
            return (None, Monitoring::FeedbackRisk { input_name });
        }
        match monitor_ring(input_rate, self.output_rate) {
            Ok((feed, tap)) => match self.sender.send(Message::Monitor(Box::new(tap))) {
                Ok(()) => (
                    Some(feed),
                    Monitoring::On {
                        input_name,
                        input_rate,
                    },
                ),
                Err(error) => (None, Monitoring::Failed(error)),
            },
            Err(error) => (None, Monitoring::Failed(error)),
        }
    }
}
fn connect_monitor(
    link: Option<&MonitorLink>,
    device: &cpal::Device,
    input_rate: u32,
) -> (Option<MonitorFeed>, Monitoring) {
    link.map_or((None, Monitoring::Off), |link| {
        link.connect(device, input_rate)
    })
}
/// Build the input stream for whatever sample format the device speaks.
fn capture_stream(
    device: &cpal::Device,
    supported: &cpal::SupportedStreamConfig,
    buffer_frames: Option<u32>,
    state: Capture,
) -> Result<Stream> {
    let mut config = supported.config();
    config.buffer_size = buffer_size(supported.buffer_size(), buffer_frames);
    match supported.sample_format() {
        SampleFormat::F32 => input_stream::<f32>(device, &config, state),
        SampleFormat::I16 => input_stream::<i16>(device, &config, state),
        SampleFormat::U16 => input_stream::<u16>(device, &config, state),
        SampleFormat::I32 => input_stream::<i32>(device, &config, state),
        SampleFormat::F64 => input_stream::<f64>(device, &config, state),
        other => Err(format!("Unsupported input format: {other}")),
    }
}
fn find_input(input: &Option<String>) -> Result<cpal::Device> {
    let host = cpal::default_host();
    match input {
        Some(wanted) => host
            .input_devices()
            .map_err(|e| e.to_string())?
            .find(|d| d.name().is_ok_and(|n| &n == wanted))
            .ok_or_else(|| format!("Input device not found: {wanted}")),
        None => host
            .default_input_device()
            .ok_or_else(|| "No input device available".to_string()),
    }
}

/// Keeps the input open only to feed [`Telemetry::input_peak`], so a singer can set their
/// level while a track is armed. Nothing is stored. The stream lives on its own worker and
/// closes when this handle is dropped.
pub struct InputMeter {
    stop: Option<std::sync::mpsc::Sender<()>>,
    pub monitoring: Monitoring,
}
impl InputMeter {
    pub fn start(
        telemetry: Arc<Telemetry>,
        input: Option<String>,
        monitor: Option<MonitorLink>,
        buffer_frames: Option<u32>,
    ) -> Result<Self> {
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
        let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
        std::thread::spawn(move || {
            let opened = (|| {
                let device = find_input(&input)?;
                let supported = device.default_input_config().map_err(|e| e.to_string())?;
                let rate = supported.sample_rate().0;
                telemetry.input_rate.store(rate, Ordering::Relaxed);
                let (feed, monitoring) = connect_monitor(monitor.as_ref(), &device, rate);
                let stream = capture_stream(
                    &device,
                    &supported,
                    buffer_frames,
                    Capture {
                        producer: None,
                        telemetry,
                        failed: Arc::new(AtomicBool::new(false)),
                        first_beat: Arc::new(AtomicU64::new(f64::NAN.to_bits())),
                        first: false,
                        monitor: feed,
                    },
                )?;
                stream.play().map_err(|e| e.to_string())?;
                Ok::<_, String>((stream, monitoring))
            })();
            match opened {
                Ok((stream, monitoring)) => {
                    if ready_tx.send(Ok(monitoring)).is_ok() {
                        let _ = stop_rx.recv();
                    }
                    drop(stream);
                }
                Err(error) => {
                    let _ = ready_tx.send(Err(error));
                }
            }
        });
        let monitoring = ready_rx
            .recv()
            .map_err(|_| "Input meter worker stopped".to_string())??;
        Ok(Self {
            stop: Some(stop_tx),
            monitoring,
        })
    }
}
impl Drop for InputMeter {
    fn drop(&mut self) {
        self.stop.take();
    }
}
struct Capture {
    /// `None` for a stream that only feeds the input meter.
    producer: Option<Producer<[f32; 2]>>,
    telemetry: Arc<Telemetry>,
    failed: Arc<AtomicBool>,
    first_beat: Arc<AtomicU64>,
    first: bool,
    /// Live copy for monitored tracks, fed whether or not a take is being kept.
    monitor: Option<MonitorFeed>,
}
fn input_stream<T: cpal::SizedSample>(
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
                // Once interrupted, retain one contiguous prefix instead of
                // concatenating audio across an unreported gap in the take.
                if capture.failed.load(Ordering::Relaxed) {
                    return;
                }
                capture
                    .telemetry
                    .input_frames
                    .store((data.len() / channels) as u32, Ordering::Relaxed);
                let mut peak = 0f32;
                for frame in data.chunks_exact(channels) {
                    for sample in &frame[..channels.min(2)] {
                        peak = peak.max(<f32 as cpal::FromSample<T>>::from_sample_(*sample).abs());
                    }
                }
                if let Some(feed) = capture.monitor.as_mut() {
                    for frame in data.chunks_exact(channels) {
                        let l = <f32 as cpal::FromSample<T>>::from_sample_(frame[0]);
                        let r = <f32 as cpal::FromSample<T>>::from_sample_(
                            frame[if channels > 1 { 1 } else { 0 }],
                        );
                        feed.push([l, r], &capture.telemetry);
                    }
                }
                let seen = f32::from_bits(capture.telemetry.input_peak.load(Ordering::Relaxed));
                if peak > seen {
                    capture
                        .telemetry
                        .input_peak
                        .store(peak.min(4.0).to_bits(), Ordering::Relaxed);
                }
                // A meter-only stream, or a count-in still clicking: nothing is kept yet, so
                // the take begins on the first beat of the song rather than on the click.
                let Some(producer) = capture.producer.as_mut() else {
                    return;
                };
                if capture.telemetry.counting_in.load(Ordering::Relaxed) {
                    return;
                }
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
                    if producer.push([l, r]).is_err() {
                        capture.failed.store(true, Ordering::Relaxed);
                        break;
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

#[cfg(test)]
mod recording_tests {
    use super::*;

    #[test]
    fn raw_recording_backup_roundtrips_and_write_failure_preserves_the_buffer() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("recordings/take.wav");
        let mut take = RecordedAudio {
            buffer: AudioBuffer::new(48000, vec![[0.125, -0.75]; 100]).unwrap(),
            warning: None,
            recovery_path: None,
        };
        take.preserve(&path).unwrap();
        let decoded = crate::audio::decode(std::fs::read(&path).unwrap(), Some("wav")).unwrap();
        assert_eq!(decoded.frames, take.buffer.frames);
        assert_eq!(take.recovery_path.as_ref(), Some(&path));
        assert!(take.preserve(&path.join("impossible.wav")).is_err());
        assert_eq!(take.buffer.frames, vec![[0.125, -0.75]; 100]);
    }

    #[test]
    fn only_a_built_in_microphone_into_built_in_speakers_is_a_feedback_risk() {
        assert!(feedback_risk(
            "MacBook Pro Microphone",
            "MacBook Pro Speakers"
        ));
        assert!(feedback_risk("Built-in Microphone", "Built-in Output"));
        assert!(feedback_risk(
            "Microphone Array (Realtek(R) Audio)",
            "Speakers (Realtek(R) Audio)"
        ));
        assert!(!feedback_risk(
            "MacBook Pro Microphone",
            "External Headphones"
        ));
        assert!(!feedback_risk("MacBook Pro Microphone", "AirPods Pro"));
        assert!(!feedback_risk("Scarlett 2i2 USB", "MacBook Pro Speakers"));
        assert!(!feedback_risk("Scarlett 2i2 USB", "Scarlett 2i2 USB"));
    }

    #[test]
    fn a_new_monitor_tap_retires_the_old_one_and_a_closed_input_retires_the_last() {
        let (mut producer, input) = RingBuffer::new(4);
        let (garbage, mut retired) = RingBuffer::new(8);
        let telemetry = Arc::new(Telemetry::default());
        let mut callback = Callback {
            renderer: Box::new(
                Renderer::new(
                    crate::store::empty(),
                    &crate::audio::Library::new(),
                    48000,
                    &std::collections::HashMap::new(),
                )
                .unwrap(),
            ),
            rack: Rack::new(1),
            input,
            garbage,
            telemetry: telemetry.clone(),
            scratch: vec![[0.0; 2]; MAX_BLOCK],
            counting: false,
            monitor: None,
            live: vec![[0.0; 2]; MAX_BLOCK],
        };
        let (first_feed, first) = monitor_ring(48000, 48000).unwrap();
        let (second_feed, second) = monitor_ring(44100, 48000).unwrap();
        producer
            .push(Message::Monitor(Box::new(first)))
            .ok()
            .unwrap();
        producer
            .push(Message::Monitor(Box::new(second)))
            .ok()
            .unwrap();
        callback.commands();
        assert!(matches!(retired.pop(), Ok(Retired::Monitor(_))));
        assert!(first_feed.is_abandoned());
        assert!(telemetry.monitoring.load(Ordering::Relaxed));
        callback.render(64);
        drop(second_feed);
        callback.reap_monitor();
        assert!(callback.monitor.is_none());
        assert!(matches!(retired.pop(), Ok(Retired::Monitor(_))));
        assert!(!telemetry.monitoring.load(Ordering::Relaxed));
    }

    #[test]
    fn input_queue_overflow_discards_pending_attacks_and_stops_transport() {
        let (mut producer, input) = RingBuffer::new(4);
        let (garbage, _retired) = RingBuffer::new(8);
        producer.push(Message::Start(0.0)).ok().unwrap();
        producer
            .push(Message::Note {
                track: 0,
                on: true,
                pitch: 60,
                velocity: 100,
            })
            .ok()
            .unwrap();
        let telemetry = Arc::new(Telemetry::default());
        telemetry.input_overflow.store(true, Ordering::Release);
        let mut callback = Callback {
            renderer: Box::new(
                Renderer::new(
                    crate::store::empty(),
                    &crate::audio::Library::new(),
                    48000,
                    &std::collections::HashMap::new(),
                )
                .unwrap(),
            ),
            rack: Rack::new(1),
            input,
            garbage,
            telemetry,
            scratch: vec![[0.0; 2]; MAX_BLOCK],
            counting: false,
            monitor: None,
            live: vec![[0.0; 2]; MAX_BLOCK],
        };
        callback.commands();
        assert!(!callback.renderer.playing);
        assert!(callback.input.is_empty());
        assert!(!callback.telemetry.input_overflow.load(Ordering::Relaxed));
    }

    #[test]
    fn interrupted_capture_preserves_every_completed_sample_and_reports_the_partial_take() {
        let (mut producer, consumer) = RingBuffer::new(16);
        let frames = [[0.1, -0.2], [0.3, -0.4], [0.5, -0.6]];
        for frame in frames {
            producer.push(frame).unwrap();
        }
        let captured = collect_recording(
            consumer,
            &AtomicBool::new(false),
            &AtomicBool::new(true),
            48000,
            16,
        )
        .unwrap();
        assert_eq!(captured.buffer.frames, frames);
        assert!(captured
            .warning
            .unwrap()
            .contains("partial take was recovered"));
    }

    #[test]
    fn memory_limit_preserves_the_contiguous_prefix_instead_of_discarding_the_take() {
        let (mut producer, consumer) = RingBuffer::new(16);
        for i in 0..8 {
            producer.push([i as f32; 2]).unwrap();
        }
        let failed = AtomicBool::new(false);
        let captured =
            collect_recording(consumer, &AtomicBool::new(true), &failed, 48000, 4).unwrap();
        assert_eq!(
            captured.buffer.frames,
            vec![[0.0; 2], [1.0; 2], [2.0; 2], [3.0; 2]]
        );
        assert!(failed.load(Ordering::Relaxed));
        assert!(captured.warning.unwrap().contains("memory limit"));
    }

    #[test]
    fn successful_capture_is_complete_and_empty_capture_reports_no_audio() {
        let (mut producer, consumer) = RingBuffer::new(16);
        producer.push([0.25; 2]).unwrap();
        let captured = collect_recording(
            consumer,
            &AtomicBool::new(true),
            &AtomicBool::new(false),
            48000,
            16,
        )
        .unwrap();
        assert_eq!(captured.buffer.frames, vec![[0.25; 2]]);
        assert!(captured.warning.is_none());
        let (_, consumer) = RingBuffer::new(16);
        assert!(collect_recording(
            consumer,
            &AtomicBool::new(true),
            &AtomicBool::new(true),
            48000,
            16
        )
        .unwrap_err()
        .contains("before any audio"));
    }
}
