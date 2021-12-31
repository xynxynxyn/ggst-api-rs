use crate::error::{Error, Result};
use std::fmt;

#[derive(Hash, Debug, PartialEq, Eq, Clone, Copy)]
pub enum Character {
    Sol,
    Ky,
    May,
    Axl,
    Chipp,
    Potemkin,
    Faust,
    Millia,
    Zato,
    Ramlethal,
    Leo,
    Nagoriyuki,
    Giovanna,
    Anji,
    Ino,
    Goldlewis,
    Jacko,
    HappyChaos,
}

impl fmt::Display for Character {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Character::Sol => write!(f, "Sol Badguy"),
            Character::Ky => write!(f, "Ky Kiske"),
            Character::May => write!(f, "May"),
            Character::Axl => write!(f, "Axl Low"),
            Character::Leo => write!(f, "Leo Whitefang"),
            Character::Ino => write!(f, "I-no"),
            Character::Zato => write!(f, "Zato=1"),
            Character::Anji => write!(f, "Anji Mito"),
            Character::Chipp => write!(f, "Chipp Zanuff"),
            Character::Faust => write!(f, "Faust"),
            Character::Potemkin => write!(f, "Potemkin"),
            Character::Millia => write!(f, "Millia Rage"),
            Character::Ramlethal => write!(f, "Ramlethal Valentine"),
            Character::Giovanna => write!(f, "Giovanna"),
            Character::Nagoriyuki => write!(f, "Nagoriyuki"),
            Character::Goldlewis => write!(f, "Goldlewis Dickinson"),
            Character::Jacko => write!(f, "Jack-o"),
            Character::HappyChaos => write!(f, "Happy Chaos"),
        }
    }
}

impl Character {
    /// Convert a byte into a Character enum.
    /// 00: Sol 01: Ky 02: May 03: Axl 04: Chipp 05: Pot 06: Faust 07: Millia
    /// 08: Zato-1 09: Ram 0a: Leo 0b: Nago 0c: Gio 0d: Anji 0e: I-No 0f: Goldlewis 10: Jack-O
    ///
    /// See https://github.com/optix2000/totsugeki/issues/35#issuecomment-922516535
    pub fn from_u8(c: u8) -> Result<Self> {
        match c {
            0x00 => Ok(Character::Sol),
            0x01 => Ok(Character::Ky),
            0x02 => Ok(Character::May),
            0x03 => Ok(Character::Axl),
            0x04 => Ok(Character::Chipp),
            0x05 => Ok(Character::Potemkin),
            0x06 => Ok(Character::Faust),
            0x07 => Ok(Character::Millia),
            0x08 => Ok(Character::Zato),
            0x09 => Ok(Character::Ramlethal),
            0x0a => Ok(Character::Leo),
            0x0b => Ok(Character::Nagoriyuki),
            0x0c => Ok(Character::Giovanna),
            0x0d => Ok(Character::Anji),
            0x0e => Ok(Character::Ino),
            0x0f => Ok(Character::Goldlewis),
            0x10 => Ok(Character::Jacko),
            0x11 => Ok(Character::HappyChaos),
            _ => Err(Error::InvalidArguments(format!(
                "{:x} is not a valid character code",
                c
            ))),
        }
    }

    /// Convert a Character back to its u8 code
    /// 00: Sol 01: Ky 02: May 03: Axl 04: Chipp 05: Pot 06: Faust 07: Millia
    /// 08: Zato-1 09: Ram 0a: Leo 0b: Nago 0c: Gio 0d: Anji 0e: I-No 0f: Goldlewis 10: Jack-O
    ///
    /// See https://github.com/optix2000/totsugeki/issues/35#issuecomment-922516535
    pub fn to_u8(&self) -> u8 {
        match self {
            Character::Sol => 0x00,
            Character::Ky => 0x01,
            Character::May => 0x02,
            Character::Axl => 0x03,
            Character::Chipp => 0x04,
            Character::Potemkin => 0x05,
            Character::Faust => 0x06,
            Character::Millia => 0x07,
            Character::Zato => 0x08,
            Character::Ramlethal => 0x09,
            Character::Leo => 0x0a,
            Character::Nagoriyuki => 0x0b,
            Character::Giovanna => 0x0c,
            Character::Anji => 0x0d,
            Character::Ino => 0x0e,
            Character::Goldlewis => 0x0f,
            Character::Jacko => 0x10,
            Character::HappyChaos => 0x11,
        }
    }

    pub fn to_code(&self) -> &'static str {
        match self {
            Character::Sol => "SOL",
            Character::Ky => "KYK",
            Character::May => "MAY",
            Character::Axl => "AXL",
            Character::Leo => "LEO",
            Character::Ino => "INO",
            Character::Zato => "ZAT",
            Character::Anji => "ANJ",
            Character::Chipp => "CHP",
            Character::Faust => "FAU",
            Character::Potemkin => "POT",
            Character::Millia => "MLL",
            Character::Ramlethal => "RAM",
            Character::Giovanna => "GIO",
            Character::Nagoriyuki => "NAG",
            Character::Goldlewis => "GLD",
            Character::Jacko => "JKO",
            Character::HappyChaos => "COS",
        }
    }

    pub fn from_code(code: &str) -> Result<Character> {
        match code {
            "SOL" => Ok(Character::Sol),
            "KYK" => Ok(Character::Ky),
            "MAY" => Ok(Character::May),
            "AXL" => Ok(Character::Axl),
            "LEO" => Ok(Character::Leo),
            "INO" => Ok(Character::Ino),
            "ZAT" => Ok(Character::Zato),
            "ANJ" => Ok(Character::Anji),
            "CHP" => Ok(Character::Chipp),
            "FAU" => Ok(Character::Faust),
            "POT" => Ok(Character::Potemkin),
            "MLL" => Ok(Character::Millia),
            "RAM" => Ok(Character::Ramlethal),
            "GIO" => Ok(Character::Giovanna),
            "NAG" => Ok(Character::Nagoriyuki),
            "GLD" => Ok(Character::Goldlewis),
            "JKO" => Ok(Character::Jacko),
            "COS" => Ok(Character::HappyChaos),
            _ => Err(Error::InvalidCharacterCode(code.into())),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Stats {
    pub level: usize,
    pub wins: usize,
}
