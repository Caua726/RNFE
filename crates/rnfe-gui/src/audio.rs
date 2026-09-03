//! Saída de áudio com cpal: o callback lê do [`AudioRing`] sem lock.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rnfe_frontend::AudioRing;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct AudioOut {
    _stream: cpal::Stream,
    pub ring: Arc<AudioRing>,
    pub sample_rate: u32,
    pub channels: usize,
    /// O stream morreu (fone desconectado, saída trocada): recriar no próximo gesto.
    dead: Arc<AtomicBool>,
}

impl AudioOut {
    /// Latência-alvo em amostras mantidas no anel: ~50 ms a 48 kHz no desktop; o dobro na
    /// web e no Android, onde o callback de áudio e o laço de eventos oscilam mais.
    pub const TARGET_QUEUE: usize =
        if cfg!(any(target_arch = "wasm32", target_os = "android")) { 4800 } else { 2400 };

    /// Abre o dispositivo padrão. `None` se não há áudio (a emulação segue muda).
    pub fn start() -> Option<AudioOut> {
        let host = cpal::default_host();
        let Some(device) = host.default_output_device() else {
            log::warn!("áudio: nenhum dispositivo de saída");
            return None;
        };
        let config = match device.default_output_config() {
            Ok(c) => c,
            Err(e) => {
                log::warn!("áudio: sem configuração padrão: {e}");
                return None;
            }
        };
        let sample_rate = config.sample_rate();
        let channels = config.channels().max(1) as usize;
        let ring = AudioRing::new(sample_rate as usize / 4); // 250 ms de capacidade
        let dead = Arc::new(AtomicBool::new(false));
        let format = config.sample_format();
        let stream_config: cpal::StreamConfig = config.into();
        let built = match format {
            cpal::SampleFormat::F32 => Self::build::<f32>(&device, &stream_config, channels, &ring, &dead),
            cpal::SampleFormat::I16 => Self::build::<i16>(&device, &stream_config, channels, &ring, &dead),
            cpal::SampleFormat::U16 => Self::build::<u16>(&device, &stream_config, channels, &ring, &dead),
            other => {
                log::warn!("áudio: formato {other:?} sem suporte");
                return None;
            }
        };
        let stream = match built {
            Ok(s) => s,
            Err(e) => {
                log::error!("áudio: {e}");
                return None;
            }
        };
        if let Err(e) = stream.play() {
            log::error!("áudio: play: {e}");
            return None;
        }
        log::info!("áudio: {sample_rate} Hz, {channels} canais, {format:?}");
        Some(AudioOut { _stream: stream, ring, sample_rate, channels, dead })
    }

    fn build<T: cpal::SizedSample + cpal::FromSample<f32>>(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        channels: usize,
        ring: &Arc<AudioRing>,
        dead: &Arc<AtomicBool>,
    ) -> Result<cpal::Stream, cpal::BuildStreamError> {
        let reader = ring.clone();
        let dead_cb = dead.clone();
        let mut mono: Vec<f32> = Vec::with_capacity(8192);
        device.build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                let frames = data.len() / channels;
                mono.resize(frames, 0.0);
                reader.pop(&mut mono);
                for (frame, &s) in data.chunks_mut(channels).zip(mono.iter()) {
                    let v = T::from_sample(s);
                    frame.fill(v);
                }
            },
            move |err| {
                log::error!("áudio: {err}");
                dead_cb.store(true, Ordering::Relaxed);
            },
            None,
        )
    }

    pub fn is_dead(&self) -> bool {
        self.dead.load(Ordering::Relaxed)
    }
}
