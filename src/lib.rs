use pgx::*;
use std::env;
use std::time::Duration;
use std::error::Error;
use serde::{Deserialize};

pg_module_magic!();


fn env_get(key: &str) -> String{
    match env::var(key) {
        Ok(v) => v,
        Err(_) => "".to_string()
    }
}

#[pg_extern]
fn fetch_env(key: &str) -> String {
    env_get(key)
}

#[pg_extern]
fn set_env(key: &str, value: &str) -> String {
    env::set_var(key, value);
    env_get(key)
}


#[derive(Deserialize)]
struct AccessTokenResponse {
    access_token: String,
    token_type: String,
    scope: String,
    created_at: i64
}

#[derive(Deserialize)]
struct Account {
    id: String,
    //username: String,
    //display_name: String,
    acct: String,
    bot: bool,
}

#[derive(Deserialize)]
struct Reblog {
    //id: String,
    content: String,
}

#[derive(Deserialize)]
struct HomeList {
    id: String,
    created_at: String,
    sensitive: bool,
    visibility: String,
    account: Account,
    reblog: Option<Reblog>,
    content: String,
}

#[derive(Deserialize)]
struct JustId {
    id: String,
}



fn api_login(username: &str, password: &str, server: &str) -> Result<AccessTokenResponse, Box<dyn Error>> {
    let client_id: &str = &env_get("PG_MASTO_CLIENT_ID");
    let client_secret: &str = &env_get("PG_MASTO_CLIENT_SECRET");
    let params = [
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("grant_type", "password"),
        ("username", username),
        ("password", password),
        ("scope", "read write"),
    ];

    let url = format!("https://{}/oauth/token", server);
    let resp: AccessTokenResponse = ureq::post(&url)
        .timeout(Duration::from_secs(10))
        .send_form(&params)?
        .into_json()?;

    Ok(resp)
}

fn api_home() -> Result<Vec<HomeList>, Box<dyn Error>> {
    let url = format!("https://{}/api/v1/timelines/home",
                      env_get("PG_MASTO_SERVER"));
    let header = format!("Bearer {}", env_get("PG_MASTO_BEARER"));
    let resp: Vec<HomeList> = ureq::get(&url)
        .set("Authorization",  &header)
        .timeout(Duration::from_secs(10))
        .call()?
        .into_json()?;

    Ok(resp)
}

fn api_user_toots(account_id: String) -> Result<Vec<HomeList>, Box<dyn Error>> {
    let url = format!("https://{}/api/v1/accounts/{}/statuses",
                      env_get("PG_MASTO_SERVER"), account_id);
    let header = format!("Bearer {}", env_get("PG_MASTO_BEARER"));
    let resp: Vec<HomeList> = ureq::get(&url)
        .set("Authorization",  &header)
        .timeout(Duration::from_secs(10))
        .call()?
        .into_json()?;

    Ok(resp)
}

fn api_toot(text: &str, sensitive: bool, spoiler_text: &str, visibility: &str) -> Result<String, Box<dyn Error>> {

    let url = format!("https://{}/api/v1/statuses",
                      env_get("PG_MASTO_SERVER"));
    let header = format!("Bearer {}", env_get("PG_MASTO_BEARER"));

    let resp: JustId = ureq::post(&url)
        .set("Authorization",  &header)
        .set("Content-Type",  "application/json")
        .timeout(Duration::from_secs(10))
        .send_json(ureq::json!({
            "status": text,
            "sensitive": sensitive,
            "spoiler_text": spoiler_text,
            "visibility": visibility,
        }))?
        .into_json()?;

    Ok(resp.id)
}

fn api_toot_simple(text: &str, visibility: &str) -> Result<String, Box<dyn Error>> {
    api_toot(text, false, "", visibility)
}

fn api_toot_cw(cw: &str, text: &str, visibility: &str) -> Result<String, Box<dyn Error>> {
    api_toot(text, true, cw, visibility)
}

#[pg_extern]
fn login(
    username: &str, password: &str, server: &str
) -> impl Iterator<Item = (
    name!(token_type, String),
    name!(scope, String),
    name!(created_at, i64),
)>{

    env::set_var("PG_MASTO_SERVER", server);

    let atr: AccessTokenResponse = api_login(username, password, server).unwrap();
    env::set_var("PG_MASTO_BEARER", atr.access_token);

    let res: Vec<(String, String, i64)> = vec![(
        atr.token_type, atr.scope, atr.created_at
    )];
    res.into_iter()
}


#[pg_extern]
fn toot(text: &str, visibility: &str) -> String {
    let res = api_toot_simple(text, visibility).unwrap_or_else(|error| {
        error!("{}", error);
    });
    res
}

#[pg_extern]
fn toot_cw(cw: &str, text: &str, visibility: &str) -> String {
    let res = api_toot_cw(cw, text, visibility).unwrap_or_else(|error| {
        error!("{}", error);
    });
    res
}


#[pg_extern]
fn home(

) -> impl Iterator<Item = (
    name!(toot_id, String),
    name!(created_at, String),
    name!(sensitive, bool),
    name!(visibility, String),
    name!(acct, String),
    name!(account_id, String),
    name!(bot, bool),
    name!(type, String),
    name!(content, String),

)> {
    let q: Vec<HomeList> = api_home().unwrap();
    let res: Vec<(String, String, bool, String, String, String, bool, String, String)> = q
        .into_iter()
        .map(|x: HomeList| (
                x.id,
                x.created_at,
                x.sensitive,
                x.visibility,
                x.account.acct,
                x.account.id,
                x.account.bot,
                if x.reblog.is_some() { "Boost".to_string() } else { "Toot".to_string() },
                if x.reblog.is_some() { x.reblog.unwrap().content } else { x.content },
                //letter: x.created_at
            ))
        .collect();

    res.into_iter()
}

#[pg_extern]
fn account(
    account_id: String,
) -> impl Iterator<Item = (
    name!(id, String),
    name!(created_at, String),
    name!(sensitive, bool),
    name!(visibility, String),
    name!(acct, String),
    name!(type, String),
    name!(content, String),

)> {
    let q: Vec<HomeList> = api_user_toots(account_id).unwrap();
    let res: Vec<(String, String, bool, String, String, String, String)> = q
        .into_iter()
        .map(|x: HomeList| (
                x.id,
                x.created_at,
                x.sensitive,
                x.visibility,
                x.account.acct,
                if x.reblog.is_some() { "Boost".to_string() } else { "Toot".to_string() },
                if x.reblog.is_some() { x.reblog.unwrap().content } else { x.content },
                //letter: x.created_at
            ))
        .collect();

    res.into_iter()
}
