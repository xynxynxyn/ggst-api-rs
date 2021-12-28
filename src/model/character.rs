use crate::error::{Error, Result};
use std::fmt;

#[derive(Debug, Clone, Copy)]
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
            _ => Err(Error::InvalidCharacterCode(code)),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Stats {
    pub level: usize,
    pub wins: usize,
}
