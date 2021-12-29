use crate::error::{Error, Result};
use crate::model::matches::*;
use crate::model::user::*;
use hex::ToHex;
use lazy_static::lazy_static;
use regex::Regex;
use reqwest;
use serde_json::Value;
use std::collections::HashMap;

const TOKEN_PREFIX: &str = "9295";
const TOKEN_SUFFIX: &str = "02a5302e302e380396b2";
const AOB_USER_STATS: &str = "070101ffffff";
const DEFAULT_UTILS_BASE_URL: &str =
    "https://ggst-utils-default-rtdb.europe-west1.firebasedatabase.app";
const DEFAULT_BASE_URL: &str = "https://ggst-api-proxy.herokuapp.com";
//const TOKEN: &str = "b2323131303237313133313233303038333834ad36313930643632363837393737";
//const AOB_MATCH_STATS: &str = "0101ffffff";

pub struct Context {
    token: String,
    base_url: String,
    utils_base_url: String,
}

impl Context {
    pub fn new(token: String) -> Self {
        Context {
            token,
            base_url: DEFAULT_BASE_URL.to_string(),
            utils_base_url: DEFAULT_UTILS_BASE_URL.to_string(),
        }
    }

    pub fn base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }

    pub fn utils_base_url(mut self, utils_base_url: String) -> Self {
        self.utils_base_url = utils_base_url;
        self
    }
}

async fn userid_from_steamid<'a>(context: &'a Context, steamid: &'a str) -> Result<'a, String> {
    let request_url = format!("{}/{}.json", context.utils_base_url, steamid);
    let response = reqwest::get(request_url).await?;
    let d: Value = serde_json::from_str(&response.text().await?)?;
    match d.get("UserID") {
        Some(s) => Ok(String::from(s.as_str().ok_or(Error::UnexpectedResponse)?)),
        None => Err(Error::UnexpectedResponse),
    }
}

pub async fn user_from_steamid<'a>(context: &'a Context, steamid: &'a str) -> Result<'a, User> {
    // Get the user id from the steamid
    let id = userid_from_steamid(context, steamid).await?;

    // Construct the request with token and appropriate AOB
    let request_url = format!("{}/api/statistics/get", context.base_url);
    let client = reqwest::Client::new();
    let query = format!(
        "{}{}{}{}{}",
        TOKEN_PREFIX,
        context.token,
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
        let ctx = Context::new(
            "b2323131303237313133313233303038333834ad36313930643632363837393737".to_string(),
        );
        let id = userid_from_steamid(&ctx, "76561198045733267")
            .await
            .unwrap();
        assert_eq!(id, "210611132841904307");
    }

    #[tokio::test]
    async fn get_user_stats() {
        let ctx = Context::new(
            "b2323131303237313133313233303038333834ad36313930643632363837393737".to_string(),
        );
        let user = user_from_steamid(&ctx, "76561198045733267").await.unwrap();
        assert_eq!(user.name, "enemy fungus");
    }
}
