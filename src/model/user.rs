use super::character;
use crate::error::*;
use std::collections::HashMap;

#[derive(Hash, Debug, PartialEq, Eq, Clone, Copy)]
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
    /// Convert a floor back to its byte representation
    ///
    /// See https://github.com/optix2000/totsugeki/issues/35#issuecomment-922516535 for mapping 
    pub fn to_u8(&self) -> u8 {
        match self {
            Floor::F1 => 0x00,
            Floor::F2 => 0x01,
            Floor::F3 => 0x02,
            Floor::F4 => 0x03,
            Floor::F5 => 0x04,
            Floor::F6 => 0x05,
            Floor::F7 => 0x06,
            Floor::F8 => 0x07,
            Floor::F9 => 0x08,
            Floor::F10 => 0x09,
            Floor::Celestial => 0x63,
        }
    }

    /// Create a floor from a byte representation
    ///
    /// See https://github.com/optix2000/totsugeki/issues/35#issuecomment-922516535 for mapping
    pub fn from_u8(c: u8) -> Result<Self> {
        match c {
            0x00 => Ok(Floor::F1),
            0x01 => Ok(Floor::F2),
            0x02 => Ok(Floor::F3),
            0x03 => Ok(Floor::F4),
            0x04 => Ok(Floor::F5),
            0x05 => Ok(Floor::F6),
            0x06 => Ok(Floor::F7),
            0x07 => Ok(Floor::F8),
            0x08 => Ok(Floor::F9),
            0x09 => Ok(Floor::F10),
            0x63 => Ok(Floor::Celestial),
            _ => Err(Error::InvalidArguments(format!(
                "{:x} is not a valid floor code",
                c
            ))),
        }
    }

    pub fn to_hex(&self) -> String {
        match self {
            Floor::F1 => "00".into(),
            Floor::F2 => "01".into(),
            Floor::F3 => "02".into(),
            Floor::F4 => "03".into(),
            Floor::F5 => "04".into(),
            Floor::F6 => "05".into(),
            Floor::F7 => "06".into(),
            Floor::F8 => "07".into(),
            Floor::F9 => "08".into(),
            Floor::F10 => "0a".into(),
            Floor::Celestial => "63".into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct MatchStats {
    pub total: usize,
    pub wins: usize,
}

#[derive(Clone, Debug)]
pub struct User {
    pub user_id: String,
    pub name: String,
    pub comment: String,
    pub floor: Floor,
    pub stats: MatchStats,
    pub celestial_stats: MatchStats,
    pub char_stats: HashMap<character::Character, character::Stats>,
}
