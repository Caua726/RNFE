//! Cadência de frames com acumulador: decide quantos frames emular a cada tick do laço.

use std::time::Duration;

/// Mantém a emulação em `fps` quadros por segundo independentemente da frequência do laço
/// (vsync de 60/120/144 Hz, `requestAnimationFrame`, terminal…).
#[derive(Debug, Clone)]
pub struct FramePacer {
    period: Duration,
    next: Duration,
    max_catchup: u32,
    speed: f64,
}

impl FramePacer {
    pub fn new(fps: f64) -> Self {
        Self { period: Duration::from_secs_f64(1.0 / fps), next: Duration::ZERO, max_catchup: 3, speed: 1.0 }
    }

    /// Multiplicador de velocidade (1.0 = tempo real, 2.0 = turbo).
    pub fn set_speed(&mut self, speed: f64) {
        self.speed = speed.max(0.01);
    }

    /// Quantos frames devem ser emulados agora. Depois de uma pausa longa, no máximo
    /// `max_catchup` (o resto é descartado, para não travar tentando recuperar).
    pub fn frames_due(&mut self, now: Duration) -> u32 {
        let period = self.period.div_f64(self.speed);
        if self.next == Duration::ZERO {
            self.next = now + period;
            return 1;
        }
        let mut due = 0;
        while now >= self.next && due < self.max_catchup {
            self.next += period;
            due += 1;
        }
        if now >= self.next {
            // ainda atrasado além do limite: realinha
            self.next = now + period;
        }
        due
    }

    /// Instante em que o próximo frame vence (para `WaitUntil`/`sleep`).
    pub fn next_deadline(&self) -> Duration {
        self.next
    }

    /// Realinha após pausa/perda de foco, sem catch-up.
    pub fn resync(&mut self, now: Duration) {
        self.next = now + self.period.div_f64(self.speed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(x: u64) -> Duration {
        Duration::from_millis(x)
    }

    #[test]
    fn one_frame_per_period() {
        let mut p = FramePacer::new(60.0);
        assert_eq!(p.frames_due(ms(0)), 1);
        assert_eq!(p.frames_due(ms(10)), 0);
        assert_eq!(p.frames_due(ms(17)), 1);
        assert_eq!(p.frames_due(ms(34)), 1);
    }

    #[test]
    fn catch_up_is_capped() {
        let mut p = FramePacer::new(60.0);
        p.frames_due(ms(0));
        assert_eq!(p.frames_due(ms(1000)), 3);
        // realinhado: o próximo tick logo depois não deve pedir mais 3
        assert_eq!(p.frames_due(ms(1001)), 0);
    }

    #[test]
    fn speed_doubles_frames() {
        let mut p = FramePacer::new(60.0);
        p.set_speed(2.0);
        p.frames_due(ms(0));
        assert_eq!(p.frames_due(ms(17)), 2);
    }

    #[test]
    fn resync_drops_backlog() {
        let mut p = FramePacer::new(60.0);
        p.frames_due(ms(0));
        p.resync(ms(5000));
        assert_eq!(p.frames_due(ms(5001)), 0);
        assert_eq!(p.frames_due(ms(5017)), 1);
    }
}
