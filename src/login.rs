use crate::account::Account;
use crate::error::{Result, XScraperError};
use crate::imap::imap_get_email_code;
use crate::utils::now_utc;
use reqwest::Client;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{self, Write};

pub const LOGIN_URL: &str = "https://api.x.com/1.1/onboarding/task.json";

#[derive(Debug, Clone, Copy, Default)]
pub struct LoginConfig {
    pub email_first: bool,
    pub manual: bool,
}

#[derive(Debug)]
struct LoginContext<'a> {
    client: Client,
    account: &'a mut Account,
    config: LoginConfig,
    previous: Value,
}

pub async fn login(account: &mut Account, config: LoginConfig) -> Result<bool> {
    if account.active {
        return Ok(true);
    }

    let client = Client::builder()
        .default_headers(account.http_headers()?)
        .redirect(reqwest::redirect::Policy::limited(10))
        .cookie_store(true)
        .build()?;
    let guest_token = get_guest_token(&client).await?;
    let response = login_initiate(&client, &guest_token).await?;
    let mut ctx = LoginContext { client, account, config, previous: response };

    while let Some(next) = next_login_task(&mut ctx).await? {
        ctx.previous = next;
        merge_cookies_from_value(ctx.account, &ctx.previous);
    }

    let ct0 = ctx.account.cookies.get("ct0").cloned();
    if ct0.is_some() {
        ctx.account.active = true;
        ctx.account.headers.insert("x-twitter-auth-type".into(), "OAuth2Session".into());
        if let Some(ct0) = ct0 {
            ctx.account.headers.insert("x-csrf-token".into(), ct0);
        }
        Ok(true)
    } else {
        ctx.account.error_msg = Some("ct0 not in cookies (most likely ip ban)".into());
        Ok(false)
    }
}

async fn get_guest_token(client: &Client) -> Result<String> {
    let response = client
        .post("https://api.x.com/1.1/guest/activate.json")
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;
    response
        .get("guest_token")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| XScraperError::LoginFlow("guest_token missing".into()))
}

async fn login_initiate(client: &Client, guest_token: &str) -> Result<Value> {
    let payload = json!({
        "input_flow_data": {
            "flow_context": {
                "debug_overrides": {},
                "start_location": { "location": "unknown" }
            }
        },
        "subtask_versions": {}
    });
    post_login_task(client, guest_token, Some(("flow_name", "login")), payload).await
}

async fn next_login_task(ctx: &mut LoginContext<'_>) -> Result<Option<Value>> {
    let Some(flow_token) = ctx.previous.get("flow_token").and_then(Value::as_str) else {
        return Err(XScraperError::LoginFlow("flow_token missing".into()));
    };
    let subtasks =
        ctx.previous.get("subtasks").and_then(Value::as_array).cloned().unwrap_or_default();

    for task in subtasks {
        let task_id = task.get("subtask_id").and_then(Value::as_str).unwrap_or_default();
        let result = match task_id {
            "LoginSuccessSubtask" => login_success(ctx, flow_token).await,
            "LoginAcid" => {
                let is_code = task
                    .pointer("/enter_text/hint_text")
                    .and_then(Value::as_str)
                    .is_some_and(|hint| hint.eq_ignore_ascii_case("confirmation code"));
                if is_code {
                    login_confirm_email_code(ctx, flow_token).await
                } else {
                    login_confirm_email(ctx, flow_token).await
                }
            }
            "AccountDuplicationCheck" => login_duplication_check(ctx, flow_token).await,
            "LoginEnterPassword" => login_enter_password(ctx, flow_token).await,
            "LoginTwoFactorAuthChallenge" => login_two_factor_auth_challenge(ctx, flow_token).await,
            "LoginEnterUserIdentifierSSO" => login_enter_username(ctx, flow_token).await,
            "LoginJsInstrumentationSubtask" => login_instrumentation(ctx, flow_token).await,
            "LoginEnterAlternateIdentifierSubtask" => {
                login_alternate_identifier(ctx, flow_token).await
            }
            _ => continue,
        };

        if let Err(error) = &result {
            ctx.account.error_msg = Some(format!("login_step={task_id} err={error}"));
        }
        return result.map(Some);
    }

    Ok(None)
}

async fn login_alternate_identifier(ctx: &LoginContext<'_>, flow_token: &str) -> Result<Value> {
    post_login_json(
        ctx,
        json!({
            "flow_token": flow_token,
            "subtask_inputs": [{
                "subtask_id": "LoginEnterAlternateIdentifierSubtask",
                "enter_text": { "text": ctx.account.username, "link": "next_link" }
            }]
        }),
    )
    .await
}

async fn login_instrumentation(ctx: &LoginContext<'_>, flow_token: &str) -> Result<Value> {
    post_login_json(
        ctx,
        json!({
            "flow_token": flow_token,
            "subtask_inputs": [{
                "subtask_id": "LoginJsInstrumentationSubtask",
                "js_instrumentation": { "response": "{}", "link": "next_link" }
            }]
        }),
    )
    .await
}

async fn login_enter_username(ctx: &LoginContext<'_>, flow_token: &str) -> Result<Value> {
    post_login_json(
        ctx,
        json!({
            "flow_token": flow_token,
            "subtask_inputs": [{
                "subtask_id": "LoginEnterUserIdentifierSSO",
                "settings_list": {
                    "setting_responses": [{
                        "key": "user_identifier",
                        "response_data": { "text_data": { "result": ctx.account.username } }
                    }],
                    "link": "next_link"
                }
            }]
        }),
    )
    .await
}

async fn login_enter_password(ctx: &LoginContext<'_>, flow_token: &str) -> Result<Value> {
    post_login_json(
        ctx,
        json!({
            "flow_token": flow_token,
            "subtask_inputs": [{
                "subtask_id": "LoginEnterPassword",
                "enter_password": { "password": ctx.account.password, "link": "next_link" }
            }]
        }),
    )
    .await
}

async fn login_two_factor_auth_challenge(
    ctx: &LoginContext<'_>,
    flow_token: &str,
) -> Result<Value> {
    let secret = ctx
        .account
        .mfa_code
        .as_deref()
        .ok_or_else(|| XScraperError::LoginFlow("MFA code is required".into()))?;
    let code = totp_rs::TOTP::new(
        totp_rs::Algorithm::SHA1,
        6,
        1,
        30,
        totp_rs::Secret::Encoded(secret.to_string())
            .to_bytes()
            .map_err(|error| XScraperError::LoginFlow(error.to_string()))?,
    )
    .map_err(|error| XScraperError::LoginFlow(error.to_string()))?
    .generate_current()
    .map_err(|error| XScraperError::LoginFlow(error.to_string()))?;

    post_login_json(
        ctx,
        json!({
            "flow_token": flow_token,
            "subtask_inputs": [{
                "subtask_id": "LoginTwoFactorAuthChallenge",
                "enter_text": { "text": code, "link": "next_link" }
            }]
        }),
    )
    .await
}

async fn login_duplication_check(ctx: &LoginContext<'_>, flow_token: &str) -> Result<Value> {
    post_login_json(
        ctx,
        json!({
            "flow_token": flow_token,
            "subtask_inputs": [{
                "subtask_id": "AccountDuplicationCheck",
                "check_logged_in_account": { "link": "AccountDuplicationCheck_false" }
            }]
        }),
    )
    .await
}

async fn login_confirm_email(ctx: &LoginContext<'_>, flow_token: &str) -> Result<Value> {
    post_login_json(
        ctx,
        json!({
            "flow_token": flow_token,
            "subtask_inputs": [{
                "subtask_id": "LoginAcid",
                "enter_text": { "text": ctx.account.email, "link": "next_link" }
            }]
        }),
    )
    .await
}

async fn login_confirm_email_code(ctx: &LoginContext<'_>, flow_token: &str) -> Result<Value> {
    let code = if ctx.config.manual {
        print!("Enter email code for {} / {}: ", ctx.account.username, ctx.account.email);
        io::stdout().flush().map_err(|source| XScraperError::io("<stdout>", source))?;
        let mut value = String::new();
        io::stdin().read_line(&mut value).map_err(|source| XScraperError::io("<stdin>", source))?;
        value.trim().to_string()
    } else {
        imap_get_email_code(&ctx.account.email, &ctx.account.email_password, now_utc()).await?
    };

    post_login_json(
        ctx,
        json!({
            "flow_token": flow_token,
            "subtask_inputs": [{
                "subtask_id": "LoginAcid",
                "enter_text": { "text": code, "link": "next_link" }
            }]
        }),
    )
    .await
}

async fn login_success(ctx: &LoginContext<'_>, flow_token: &str) -> Result<Value> {
    post_login_json(ctx, json!({ "flow_token": flow_token, "subtask_inputs": [] })).await
}

async fn post_login_json(ctx: &LoginContext<'_>, payload: Value) -> Result<Value> {
    post_login_task(&ctx.client, "", None, payload).await
}

async fn post_login_task(
    client: &Client,
    guest_token: &str,
    query: Option<(&str, &str)>,
    payload: Value,
) -> Result<Value> {
    let mut request = client.post(LOGIN_URL);
    if !guest_token.is_empty() {
        request = request.header("x-guest-token", guest_token);
    }
    if let Some((key, value)) = query {
        request = request.query(&[(key, value)]);
    }
    let response = request.json(&payload).send().await?.error_for_status()?.json::<Value>().await?;
    Ok(response)
}

fn merge_cookies_from_value(account: &mut Account, value: &Value) {
    if let Some(cookies) = value.get("cookies").and_then(Value::as_object) {
        for (key, value) in cookies {
            if let Some(value) = value.as_str() {
                account.cookies.insert(key.clone(), value.to_string());
            }
        }
    }
}

pub fn cookie_map_from_response_headers(
    headers: &reqwest::header::HeaderMap,
) -> BTreeMap<String, String> {
    let mut cookies = BTreeMap::new();
    for value in headers.get_all(reqwest::header::SET_COOKIE) {
        let Ok(raw) = value.to_str() else {
            continue;
        };
        if let Some((name, rest)) = raw.split_once('=') {
            let cookie_value = rest.split(';').next().unwrap_or_default();
            cookies.insert(name.to_string(), cookie_value.to_string());
        }
    }
    cookies
}
