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
        let device = host.default_output_device()?;
        let config = device.default_output_config().ok()?;
        let sample_rate = config.sample_rate();
        let channels = config.channels() as usize;
        let ring = AudioRing::new(sample_rate as usize / 4); // 250 ms de capacidade
        let reader = ring.clone();
        let dead = Arc::new(AtomicBool::new(false));
        let dead_cb = dead.clone();
        let mut mono: Vec<f32> = Vec::new();
        let stream = device
            .build_output_stream(
                &config.into(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let frames = data.len() / channels.max(1);
                    mono.resize(frames, 0.0);
                    reader.pop(&mut mono);
                    for (frame, &s) in data.chunks_mut(channels.max(1)).zip(mono.iter()) {
                        frame.fill(s);
                    }
                },
                move |err| {
                    log::error!("áudio: {err}");
                    dead_cb.store(true, Ordering::Relaxed);
                },
                None,
            )
            .ok()?;
        stream.play().ok()?;
        log::info!("áudio: {sample_rate} Hz, {channels} canais");
        Some(AudioOut { _stream: stream, ring, sample_rate, channels, dead })
    }

    pub fn is_dead(&self) -> bool {
        self.dead.load(Ordering::Relaxed)
    }
}
