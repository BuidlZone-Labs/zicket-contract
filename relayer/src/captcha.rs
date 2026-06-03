use reqwest::Client;
use serde::Deserialize;

pub struct CaptchaService {
    secret_key: String,
    client: Client,
}

#[derive(Deserialize)]
struct CaptchaResponse {
    success: bool,
}

impl CaptchaService {
    pub fn new(secret_key: String) -> Self {
        Self {
            secret_key,
            client: Client::new(),
        }
    }

    pub async fn verify(&self, token: &str) -> bool {
        if token.is_empty() {
            return false;
        }

        // For local testing without a real captcha
        if self.secret_key == "dummy_secret" && token == "valid_dummy_token" {
            return true;
        }

        // Verify using Cloudflare Turnstile / reCAPTCHA API
        let url = "https://challenges.cloudflare.com/turnstile/v0/siteverify";

        let params = [("secret", self.secret_key.as_str()), ("response", token)];

        match self.client.post(url).form(&params).send().await {
            Ok(resp) => {
                if let Ok(result) = resp.json::<CaptchaResponse>().await {
                    result.success
                } else {
                    false
                }
            }
            Err(_) => false,
        }
    }
}
