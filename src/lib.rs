pub mod error;
pub mod requests;

use chrono::prelude::*;
use error::*;
#[cfg(feature = "serde")]
use serde_crate::{Deserialize, Serialize};
use std::fmt;

// Reexport the functions and structs from requests.rs
pub use requests::*;

/// Player information associated with a match
#[derive(Hash, Clone, Debug, PartialOrd, Ord)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(crate = "serde_crate")
)]
pub struct Player {
    id: u64,
    character: Character,
    name: String,
}

impl PartialEq for Player {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.character == other.character
    }
}

impl Eq for Player {}

impl fmt::Display for Player {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} as {}", self.name, self.character)
    }
}

impl Player {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn character(&self) -> Character {
        self.character
    }
}

/// Indicates which player won a match
#[derive(Hash, PartialEq, Eq, Debug, Clone, Copy, PartialOrd, Ord)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(crate = "serde_crate")
)]
enum Winner {
    Player1,
    Player2,
}

/// A match received by the get_replay API
/// Use requests::get_replays() to query for replays to get a set of this struct
#[derive(Hash, PartialEq, Eq, Debug, Clone, PartialOrd, Ord)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(crate = "serde_crate")
)]
pub struct Match {
    timestamp: DateTime<Utc>,
    floor: Floor,
    players: (Player, Player),
    winner: Winner,
}

impl Match {
    pub fn floor(&self) -> &Floor {
        &self.floor
    }

    pub fn timestamp(&self) -> &DateTime<Utc> {
        &self.timestamp
    }

    pub fn players(&self) -> (&Player, &Player) {
        (&self.players.0, &self.players.1)
    }

    /// Get the player information about the winner
    pub fn winner(&self) -> &Player {
        match self.winner {
            Winner::Player1 => &self.players.0,
            Winner::Player2 => &self.players.1,
        }
    }

    /// Get the player information about the winner
    pub fn loser(&self) -> &Player {
        match self.winner {
            Winner::Player1 => &self.players.1,
            Winner::Player2 => &self.players.0,
        }
    }
}

impl fmt::Display for Match {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} on floor {:?} {{\n  Winner: {}\n  Loser: {}\n}}",
            self.timestamp(),
            self.floor(),
            self.winner(),
            self.loser()
        )
    }
}

/// Enum for characters in the game
#[derive(Hash, Debug, PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(crate = "serde_crate")
)]
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
            _ => Err(Error::InvalidArgument(format!(
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
}

/// Enum mapping for floors present in the game
#[derive(PartialOrd, Ord, Debug, PartialEq, Eq, Clone, Copy, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(crate = "serde_crate")
)]
pub enum Floor {
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    Celestial,
}

impl Floor {
    /// Create a floor from a byte representation
    ///
    /// See https://github.com/optix2000/totsugeki/issues/35#issuecomment-922516535 for mapping
    fn from_u8(c: u8) -> Result<Self> {
        match c {
            0x01 => Ok(Floor::F1),
            0x02 => Ok(Floor::F2),
            0x03 => Ok(Floor::F3),
            0x04 => Ok(Floor::F4),
            0x05 => Ok(Floor::F5),
            0x06 => Ok(Floor::F6),
            0x07 => Ok(Floor::F7),
            0x08 => Ok(Floor::F8),
            0x09 => Ok(Floor::F9),
            0x0a => Ok(Floor::F10),
            0x63 => Ok(Floor::Celestial),
            _ => Err(Error::InvalidArgument(format!(
                "{:x} is not a valid floor code",
                c
            ))),
        }
    }

    /// Similar to to_u8() but it directly returns its string representation for url building
    fn to_hex(&self) -> String {
        match self {
            Floor::F1 => "01".into(),
            Floor::F2 => "02".into(),
            Floor::F3 => "03".into(),
            Floor::F4 => "04".into(),
            Floor::F5 => "05".into(),
            Floor::F6 => "06".into(),
            Floor::F7 => "07".into(),
            Floor::F8 => "08".into(),
            Floor::F9 => "09".into(),
            Floor::F10 => "0a".into(),
            Floor::Celestial => "63".into(),
        }
    }
}
