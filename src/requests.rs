use crate::error::{Error, Result};
use crate::model::matches::*;
use crate::model::user::*;
use hex::ToHex;
use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashMap;

use reqwest;
use serde_json::Value;
const TOKEN_PREFIX: &str = "9295";
const TOKEN_SUFFIX: &str = "02a5302e302e380396b2";
const TOKEN: &str = "b2323131303237313133313233303038333834ad36313930643632363837393737";
const AOB_USER_STATS: &str = "070101ffffff";
//const AOB_MATCH_STATS: &str = "0101ffffff";
const UTILS_BASE_URL: &str = "https://ggst-utils-default-rtdb.europe-west1.firebasedatabase.app";
const BASE_URL: &str = "https://ggst-api-proxy.herokuapp.com";

async fn userid_from_steamid(steamid: &str) -> Result<'_, String> {
    let request_url = format!("{}/{}.json", UTILS_BASE_URL, steamid);
    let response = reqwest::get(request_url).await?;
    let d: Value = serde_json::from_str(&response.text().await?)?;
    match d.get("UserID") {
        Some(s) => Ok(String::from(s.as_str().ok_or(Error::UnexpectedResponse)?)),
        None => Err(Error::UnexpectedResponse),
    }
}

pub async fn user_from_steamid<'a>(steamid: &str) -> Result<'_, User> {
    // Get the user id from the steamid
    let id = userid_from_steamid(steamid).await?;

    // Construct the request with token and appropriate AOB
    let request_url = format!("{}/api/statistics/get", BASE_URL);
    let client = reqwest::Client::new();
    let query = format!(
        "{}{}{}{}{}",
        TOKEN_PREFIX,
        TOKEN,
        TOKEN_SUFFIX,
        id.encode_hex::<String>(),
        AOB_USER_STATS
    );
    let response = client
        .post(request_url)
        .form(&[("data", query)])
        .send()
        .await?;

    // Remove invalid unicode stuff before the actual json body
    let content = &response.text().await?;
    lazy_static! {
        static ref RE: Regex = Regex::new(r"[^\{]*\{").expect("Could not compile regex");
    }
    let content = RE.replacen(content, 1, "{");
    let v: Value = serde_json::from_str(&content)?;

    // Assemble the user object
    Ok(User {
        user_id: id,
        name: String::from(
            v.get("NickName")
                .ok_or(Error::UnexpectedResponse)?
                .as_str()
                .ok_or(Error::UnexpectedResponse)?,
        ),
        comment: String::from(
            v.get("PublicComment")
                .ok_or(Error::UnexpectedResponse)?
                .as_str()
                .ok_or(Error::UnexpectedResponse)?,
        ),
        floor: Floor::Celestial,
        stats: MatchStats { total: 0, wins: 0 },
        celestial_stats: MatchStats { total: 0, wins: 0 },
        char_stats: HashMap::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn get_userid() {
        let id = userid_from_steamid("76561198045733267").await.unwrap();
        assert_eq!(id, "210611132841904307");
    }

    #[tokio::test]
    async fn get_user_stats() {
        let user = user_from_steamid("76561198045733267").await.unwrap();
        assert_eq!(user.name, "enemy fungus");
    }
}
