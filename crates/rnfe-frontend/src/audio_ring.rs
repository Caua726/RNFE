//! Anel de áudio SPSC sem lock: o laço de emulação escreve, o callback de áudio lê.
//!
//! Sem `unsafe` e sem `Mutex`: cada slot é um `AtomicU32` com os bits do `f32`, e os índices
//! são `AtomicUsize` com ordem acquire/release. Um único produtor e um único consumidor.
//! Em underrun o consumidor repete a última amostra (menos estalo que silêncio).

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

pub struct AudioRing {
    slots: Box<[AtomicU32]>,
    mask: usize,
    head: AtomicUsize, // próximo a ler
    tail: AtomicUsize, // próximo a escrever
    last: AtomicU32,
    underruns: AtomicUsize,
}

impl AudioRing {
    /// `capacity` é arredondada para potência de 2 (≥ 2).
    pub fn new(capacity: usize) -> Arc<AudioRing> {
        let cap = capacity.max(2).next_power_of_two();
        let slots = (0..cap).map(|_| AtomicU32::new(0)).collect::<Vec<_>>().into_boxed_slice();
        Arc::new(AudioRing {
            slots,
            mask: cap - 1,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            last: AtomicU32::new(0),
            underruns: AtomicUsize::new(0),
        })
    }

    pub fn capacity(&self) -> usize {
        self.mask + 1
    }

    /// Amostras disponíveis para leitura.
    pub fn len(&self) -> usize {
        self.tail.load(Ordering::Acquire).wrapping_sub(self.head.load(Ordering::Acquire))
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Escreve o que couber; devolve quantas amostras entraram (o excesso é descartado —
    /// melhor perder som velho do que acumular latência).
    pub fn push(&self, samples: &[f32]) -> usize {
        let head = self.head.load(Ordering::Acquire);
        let mut tail = self.tail.load(Ordering::Relaxed);
        let free = self.capacity() - tail.wrapping_sub(head);
        let n = samples.len().min(free);
        for &s in &samples[..n] {
            self.slots[tail & self.mask].store(s.to_bits(), Ordering::Relaxed);
            tail = tail.wrapping_add(1);
        }
        self.tail.store(tail, Ordering::Release);
        n
    }

    /// Preenche `out`; em underrun repete a última amostra e conta.
    pub fn pop(&self, out: &mut [f32]) {
        let tail = self.tail.load(Ordering::Acquire);
        let mut head = self.head.load(Ordering::Relaxed);
        let avail = tail.wrapping_sub(head);
        let n = out.len().min(avail);
        let mut last = f32::from_bits(self.last.load(Ordering::Relaxed));
        for o in &mut out[..n] {
            last = f32::from_bits(self.slots[head & self.mask].load(Ordering::Relaxed));
            *o = last;
            head = head.wrapping_add(1);
        }
        self.head.store(head, Ordering::Release);
        self.last.store(last.to_bits(), Ordering::Relaxed);
        if n < out.len() {
            self.underruns.fetch_add(1, Ordering::Relaxed);
            for o in &mut out[n..] {
                *o = last;
            }
        }
    }

    /// Descarta tudo (troca de ROM, pausa).
    pub fn clear(&self) {
        self.head.store(self.tail.load(Ordering::Acquire), Ordering::Release);
    }

    /// Descarta amostras antigas até sobrar `keep` (controle de latência).
    pub fn trim_to(&self, keep: usize) {
        let len = self.len();
        if len > keep {
            let head = self.head.load(Ordering::Relaxed);
            self.head.store(head.wrapping_add(len - keep), Ordering::Release);
        }
    }

    pub fn underruns(&self) -> usize {
        self.underruns.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifo_order_and_capacity() {
        let r = AudioRing::new(6);
        assert_eq!(r.capacity(), 8);
        assert_eq!(r.push(&[1.0, 2.0, 3.0]), 3);
        let mut out = [0.0; 2];
        r.pop(&mut out);
        assert_eq!(out, [1.0, 2.0]);
        assert_eq!(r.push(&[4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0]), 7, "só cabem 7 com 1 dentro");
        assert_eq!(r.len(), 8);
        let mut out = [0.0; 8];
        r.pop(&mut out);
        assert_eq!(out, [3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]);
        assert!(r.is_empty());
    }

    #[test]
    fn underrun_repeats_last_sample() {
        let r = AudioRing::new(4);
        r.push(&[0.5]);
        let mut out = [9.0; 3];
        r.pop(&mut out);
        assert_eq!(out, [0.5, 0.5, 0.5]);
        assert_eq!(r.underruns(), 1);
    }

    #[test]
    fn trim_and_clear() {
        let r = AudioRing::new(16);
        r.push(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        r.trim_to(2);
        let mut out = [0.0; 2];
        r.pop(&mut out);
        assert_eq!(out, [4.0, 5.0]);
        r.push(&[7.0]);
        r.clear();
        assert!(r.is_empty());
    }

    #[test]
    fn wraps_around_many_times() {
        let r = AudioRing::new(8);
        let mut expect = 0.0f32;
        for i in 0..1000 {
            let v = [i as f32, i as f32 + 0.5];
            assert_eq!(r.push(&v), 2);
            let mut out = [0.0; 2];
            r.pop(&mut out);
            assert_eq!(out[0], expect);
            expect += 1.0;
        }
    }
}
