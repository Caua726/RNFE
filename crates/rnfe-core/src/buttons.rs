/// Estado dos 8 botões de um controle, no bit layout que o registrador `$4016` desloca.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug, Hash)]
pub struct Buttons(pub u8);

impl Buttons {
    pub const NONE: Buttons = Buttons(0);
    pub const A: Buttons = Buttons(0x80);
    pub const B: Buttons = Buttons(0x40);
    pub const SELECT: Buttons = Buttons(0x20);
    pub const START: Buttons = Buttons(0x10);
    pub const UP: Buttons = Buttons(0x08);
    pub const DOWN: Buttons = Buttons(0x04);
    pub const LEFT: Buttons = Buttons(0x02);
    pub const RIGHT: Buttons = Buttons(0x01);

    pub fn contains(self, other: Buttons) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn with(self, other: Buttons, pressed: bool) -> Buttons {
        if pressed { self | other } else { self & !other }
    }
}

impl std::ops::BitOr for Buttons {
    type Output = Buttons;
    fn bitor(self, rhs: Buttons) -> Buttons {
        Buttons(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for Buttons {
    fn bitor_assign(&mut self, rhs: Buttons) {
        self.0 |= rhs.0;
    }
}

impl std::ops::BitAnd for Buttons {
    type Output = Buttons;
    fn bitand(self, rhs: Buttons) -> Buttons {
        Buttons(self.0 & rhs.0)
    }
}

impl std::ops::Not for Buttons {
    type Output = Buttons;
    fn not(self) -> Buttons {
        Buttons(!self.0)
    }
}
