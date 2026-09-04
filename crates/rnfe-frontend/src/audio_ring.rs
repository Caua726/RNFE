//! Anel de áudio SPSC sem lock: o laço de emulação escreve, o callback de áudio lê.
//!
//! Sem `unsafe` e sem `Mutex`: cada slot é um `AtomicU32` com os bits do `f32`, e os índices
//! são `AtomicUsize` com ordem acquire/release. Um único produtor e um único consumidor.
//! Em underrun o consumidor repete a última amostra (menos estalo que silêncio).
//!
//! `head` só é escrito pelo consumidor: descartar amostras (limpar ou limitar a latência) é um
//! **pedido** do produtor (`flush`/`trim`) aplicado pelo consumidor no `pop` seguinte. Escrever
//! `head` das duas pontas dava estalo quando a limpeza caía no meio de um `pop`.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

pub struct AudioRing {
    slots: Box<[AtomicU32]>,
    mask: usize,
    head: AtomicUsize, // próximo a ler (só o consumidor escreve)
    tail: AtomicUsize, // próximo a escrever
    last: AtomicU32,
    underruns: AtomicUsize,
    /// Geração de "descarte tudo" pedida pelo produtor; o consumidor confirma em `flush_ack`.
    flush: AtomicUsize,
    flush_ack: AtomicUsize,
    /// Posição do `tail` quando o descarte foi pedido (o que vier depois é mantido).
    flush_at: AtomicUsize,
    /// Teto de amostras acumuladas: acima disso o consumidor pula as mais antigas.
    trim: AtomicUsize,
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
            flush: AtomicUsize::new(0),
            flush_ack: AtomicUsize::new(0),
            flush_at: AtomicUsize::new(0),
            trim: AtomicUsize::new(usize::MAX),
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

    /// Escreve só o que couber abaixo de `max_len` amostras na fila (o excesso — o mais novo —
    /// é descartado). Só o produtor toca nos índices dele: mantém o SPSC.
    /// Escreve tudo que couber e limita a latência **descartando o passado** (o consumidor
    /// aplica): jogar fora as amostras novas abriria um buraco no meio da onda.
    pub fn push_capped(&self, samples: &[f32], max_len: usize) -> usize {
        let n = self.push(samples);
        self.trim_to(max_len);
        n
    }

    /// Enche com silêncio até `n` amostras: partida/retomada absorvem o jitter do laço sem
    /// underrun (a latência-alvo vira piso, não só teto).
    pub fn prime(&self, n: usize) {
        // Se há um `clear()` pendente, o consumidor vai pular tudo que veio antes: o que conta
        // é o que existe **depois** do ponto do descarte, senão o prime não empurra nada e a
        // fila zera no próximo `pop`.
        let len = self.len_after_flush();
        if len < n {
            let zeros = vec![0.0f32; n - len];
            self.push(&zeros);
        }
        self.last.store(0, Ordering::Relaxed);
    }

    /// Preenche `out`; em underrun decai a última amostra até o silêncio (sem DC preso nem
    /// degrau na volta) e conta.
    pub fn pop(&self, out: &mut [f32]) {
        let mut head = self.head.load(Ordering::Relaxed);
        // pedidos do produtor, aplicados aqui (só esta ponta escreve `head`). `tail` é lido por
        // último: lê-lo antes perdia um `clear()` que chegasse no meio.
        let flush = self.flush.load(Ordering::Acquire);
        let at = self.flush_at.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        if flush != self.flush_ack.load(Ordering::Relaxed) {
            self.flush_ack.store(flush, Ordering::Relaxed);
            // só o que existia quando o descarte foi pedido: o silêncio primado logo depois
            // (prime_audio) precisa sobreviver, senão a fila volta a zero e estala
            if tail.wrapping_sub(at) <= tail.wrapping_sub(head) {
                head = at;
            }
        }
        let trim = self.trim.load(Ordering::Relaxed);
        let ahead = tail.wrapping_sub(head);
        if ahead > trim {
            head = head.wrapping_add(ahead - trim); // joga fora o passado, não o presente
        }
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
                last *= 0.98; // ~5 ms até o silêncio
                // multiplicar subnormal para sempre custa caro; `is_finite` também mata NaN
                if !last.is_finite() || last.abs() <= 1e-8 {
                    last = 0.0;
                }
                *o = last;
            }
            self.last.store(last.to_bits(), Ordering::Relaxed);
        }
    }

    /// Pede ao consumidor que descarte tudo (troca de ROM, pausa). Vale no próximo `pop`.
    pub fn clear(&self) {
        self.flush_at.store(self.tail.load(Ordering::Acquire), Ordering::Release);
        self.flush.fetch_add(1, Ordering::Release);
    }

    /// Teto de latência: o consumidor pula as amostras antigas acima de `keep`.
    pub fn trim_to(&self, keep: usize) {
        self.trim.store(keep, Ordering::Relaxed);
    }

    /// Amostras que vão sobrar depois de o consumidor aplicar um descarte pendente.
    fn len_after_flush(&self) -> usize {
        let tail = self.tail.load(Ordering::Acquire);
        if self.flush.load(Ordering::Acquire) != self.flush_ack.load(Ordering::Acquire) {
            return tail.wrapping_sub(self.flush_at.load(Ordering::Acquire));
        }
        tail.wrapping_sub(self.head.load(Ordering::Acquire))
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
    fn underrun_decays_last_sample() {
        let r = AudioRing::new(4);
        r.push(&[0.5]);
        let mut out = [9.0; 3];
        r.pop(&mut out);
        assert_eq!(out[0], 0.5);
        assert!(out[1] < 0.5 && out[2] < out[1] && out[2] > 0.0, "decai: {out:?}");
        assert_eq!(r.underruns(), 1);
        let mut more = [9.0; 400];
        r.pop(&mut more);
        assert!(more[399].abs() < 1e-3, "silêncio depois de ~5 ms");
    }

    #[test]
    fn prime_and_capped_push() {
        let r = AudioRing::new(64);
        r.prime(10);
        assert_eq!(r.len(), 10);
        r.prime(4);
        assert_eq!(r.len(), 10, "prime nunca descarta");
        // o teto vale ao ler: entra tudo, o consumidor pula o passado
        r.push_capped(&[1.0; 20], 16);
        let mut out = [0.0; 16];
        r.pop(&mut out);
        assert_eq!(out, [1.0; 16], "sobra o áudio mais novo, não o mais velho");
        assert!(r.is_empty());
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
        let mut out = [9.0; 1];
        r.pop(&mut out); // o descarte pedido é aplicado aqui, do lado do consumidor
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

    /// `clear()` seguido de `prime()` precisa deixar a fila cheia de silêncio: antes o `prime`
    /// olhava o tamanho atual (que ainda contava o áudio a ser descartado) e não empurrava nada.
    #[test]
    fn clear_e_prime_deixam_a_fila_no_alvo() {
        let r = AudioRing::new(64);
        r.push(&[0.5; 40]);
        r.clear();
        r.prime(20);
        let mut out = [9.0; 20];
        r.pop(&mut out);
        assert_eq!(out, [0.0; 20], "só silêncio novo");
        assert_eq!(r.underruns(), 0, "não pode faltar amostra");
    }

    /// O decaimento de underrun tem que chegar a zero (senão o callback fica multiplicando
    /// subnormais para sempre).
    #[test]
    fn decaimento_chega_a_zero() {
        let r = AudioRing::new(8);
        r.push(&[0.5]);
        let mut out = [0.0; 2000];
        r.pop(&mut out);
        assert_eq!(out[1999], 0.0, "chegou em {}", out[1999]);
    }
}
