use crate::character::Character;
use crate::model::user::Floor;
use chrono::prelude::*;

#[derive(Hash, PartialEq, Eq, Debug)]
pub struct Player {
    pub id: String,
    pub name: String,
    pub character: Character,
}

#[derive(Hash, PartialEq, Eq, Debug)]
pub enum Winner {
    Player1,
    Player2,
}

#[derive(Hash, PartialEq, Eq, Debug)]
pub struct Match {
    pub floor: Floor,
    pub timestamp: DateTime<Utc>,
    pub players: (Player, Player),
    pub winner: Winner,
}
