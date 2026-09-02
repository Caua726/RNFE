//! Estado dos botões, com suporte a fontes que só sabem "apertou" (terminal) e a fontes
//! com pressionar/soltar (teclado de janela, toque).

use rnfe_core::Buttons;
use std::time::Duration;

/// Combina botões mantidos (teclado/toque) com botões "pulsados" que expiram sozinhos.
#[derive(Debug, Default, Clone)]
pub struct InputState {
    held: Buttons,
    /// Instante de expiração de cada bit (0..8), para fontes sem evento de soltar.
    pulses: [Option<Duration>; 8],
}

impl InputState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pressionar/soltar explícito (teclado de janela, gamepad, toque).
    pub fn set(&mut self, buttons: Buttons, pressed: bool) {
        self.held = self.held.with(buttons, pressed);
    }

    /// Pulso: o botão fica pressionado até `now + hold` (terminais não avisam quando a tecla solta).
    pub fn pulse(&mut self, buttons: Buttons, now: Duration, hold: Duration) {
        for bit in 0..8 {
            if buttons.0 & (1 << bit) != 0 {
                self.pulses[bit] = Some(now + hold);
            }
        }
    }

    /// Solta tudo (perda de foco, pausa).
    pub fn clear(&mut self) {
        self.held = Buttons::NONE;
        self.pulses = [None; 8];
    }

    /// Botões efetivamente pressionados em `now`.
    pub fn current(&mut self, now: Duration) -> Buttons {
        let mut b = self.held;
        for (bit, p) in self.pulses.iter_mut().enumerate() {
            if let Some(until) = *p {
                if now < until {
                    b.0 |= 1 << bit;
                } else {
                    *p = None;
                }
            }
        }
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(x: u64) -> Duration {
        Duration::from_millis(x)
    }

    #[test]
    fn held_buttons_stay_until_released() {
        let mut s = InputState::new();
        s.set(Buttons::A | Buttons::LEFT, true);
        assert_eq!(s.current(ms(0)), Buttons::A | Buttons::LEFT);
        s.set(Buttons::A, false);
        assert_eq!(s.current(ms(100)), Buttons::LEFT);
    }

    #[test]
    fn pulses_expire() {
        let mut s = InputState::new();
        s.pulse(Buttons::START, ms(0), ms(120));
        assert_eq!(s.current(ms(50)), Buttons::START);
        assert_eq!(s.current(ms(120)), Buttons::NONE);
        assert_eq!(s.current(ms(500)), Buttons::NONE);
    }

    #[test]
    fn pulse_and_held_combine() {
        let mut s = InputState::new();
        s.set(Buttons::B, true);
        s.pulse(Buttons::RIGHT, ms(0), ms(100));
        assert_eq!(s.current(ms(10)), Buttons::B | Buttons::RIGHT);
        s.clear();
        assert_eq!(s.current(ms(11)), Buttons::NONE);
    }
}
