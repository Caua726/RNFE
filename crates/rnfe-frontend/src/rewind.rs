//! Rewind: anel de save states gravados a cada N frames, limitado por memória.
//!
//! `record` uma vez por frame; segurar a tecla de rewind chama `step_back` por frame, que
//! restaura o state mais recente e o descarta (volta `EVERY` frames por chamada).

use rnfe_core::Nes;
use std::collections::VecDeque;

pub struct Rewind {
    ring: VecDeque<Vec<u8>>,
    bytes: usize,
    cap_bytes: usize,
    every: u32,
    counter: u32,
}

impl Rewind {
    /// Frames entre snapshots.
    pub const EVERY: u32 = 5;
    /// Limite padrão de memória (32 MB ≈ 1 500 states ≈ 2 min de jogo).
    pub const DEFAULT_CAP: usize = 32 << 20;

    pub fn new(cap_bytes: usize) -> Self {
        Rewind { ring: VecDeque::new(), bytes: 0, cap_bytes, every: Self::EVERY, counter: 0 }
    }

    pub fn len(&self) -> usize {
        self.ring.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// Descarta o histórico (troca de ROM, reset).
    pub fn clear(&mut self) {
        self.ring.clear();
        self.bytes = 0;
        self.counter = 0;
    }

    /// Chame uma vez por frame emulado. Grava um state a cada `EVERY` frames.
    pub fn record(&mut self, nes: &Nes) {
        self.counter += 1;
        if self.counter < self.every {
            return;
        }
        self.counter = 0;
        let st = nes.save_state();
        self.bytes += st.len();
        self.ring.push_back(st);
        while self.bytes > self.cap_bytes && self.ring.len() > 1 {
            if let Some(old) = self.ring.pop_front() {
                self.bytes -= old.len();
            }
        }
    }

    /// Volta ao state mais recente e o remove. `false` se não há histórico.
    pub fn step_back(&mut self, nes: &mut Nes) -> bool {
        let Some(st) = self.ring.pop_back() else { return false };
        self.bytes -= st.len();
        self.counter = 0;
        nes.load_state(&st).is_ok()
    }
}
