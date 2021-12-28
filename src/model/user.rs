use super::character;
use super::matches;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
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

#[derive(Clone, Debug)]
pub struct User {
    pub user_id: String,
    pub name: String,
    pub comment: String,
    pub floor: Floor,
    pub stats: matches::MatchStats,
    pub celestial_stats: matches::MatchStats,
    pub char_stats: HashMap<character::Character, character::Stats>,
}
