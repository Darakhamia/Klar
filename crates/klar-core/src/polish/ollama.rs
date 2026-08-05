//! Polishing through a model running on this machine.
//!
//! Ollama over plain HTTP to localhost. Nothing leaves the machine, which is
//! the whole reason it is the default and the only one wired up in M4 — the
//! cloud path exists behind the same trait and is opt-in, visible, and not this
//! file's problem.
//!
//! The 400 ms budget is not a target this stage tries to hit by being clever.
//! It is enforced: a model that has not answered in time is abandoned and the
//! transcript is used, because text arriving late is worse than text arriving
//! plain.

use super::{PolishError, PolishRequest, TextPolisher, guard};
use std::time::Duration;

/// Where Ollama listens unless told otherwise.
pub const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:11434";

/// No default model name.
///
/// Whatever is on this machine is what works, the useful sizes change every few
/// months, and a hard-coded name would send a first-time user to a download
/// that may not be the right one. The settings window lists what Ollama
/// actually has.
#[derive(Debug, Clone)]
pub struct OllamaConfig {
    pub endpoint: String,
    pub model: String,
    /// How long the model gets before the transcript is used instead.
    pub budget: Duration,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            endpoint: DEFAULT_ENDPOINT.to_owned(),
            model: String::new(),
            budget: Duration::from_millis(400),
        }
    }
}

pub struct Ollama {
    config: OllamaConfig,
    client: reqwest::Client,
}

impl Ollama {
    pub fn new(config: OllamaConfig) -> Result<Self, PolishError> {
        let client = reqwest::Client::builder()
            // Slightly past the budget: the budget is what the pipeline will
            // wait for, and a connection that is merely slow should still get
            // the chance to say what went wrong.
            .timeout(config.budget + Duration::from_millis(200))
            .build()
            .map_err(|error| PolishError::Unreadable(error.to_string()))?;

        Ok(Self { config, client })
    }

    pub fn config(&self) -> &OllamaConfig {
        &self.config
    }

    /// The models this Ollama has pulled, for the settings window to offer.
    pub async fn models(&self) -> Result<Vec<String>, PolishError> {
        let url = format!("{}/api/tags", self.config.endpoint);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|_| PolishError::Unreachable(self.config.endpoint.clone()))?;

        let body: Tags = response
            .json()
            .await
            .map_err(|error| PolishError::Unreadable(error.to_string()))?;

        Ok(body.models.into_iter().map(|model| model.name).collect())
    }
}

impl TextPolisher for Ollama {
    async fn polish(&mut self, request: PolishRequest<'_>) -> Result<String, PolishError> {
        let Some(instructions) = request.strength.prompt() else {
            // Verbatim reaching here is a caller bug, not a reason to send the
            // user's words to a model.
            return Ok(request.text.to_owned());
        };

        let system = if request.vocabulary.is_empty() {
            instructions.to_owned()
        } else {
            // Names the speaker has taught Klar. Without this a model helpfully
            // "corrects" them, which is the opposite of the dictionary's job.
            format!(
                "{instructions}\nThese words are spelled correctly and must not be changed: {}.",
                request.vocabulary.join(", ")
            )
        };

        let body = serde_json::json!({
            "model": self.config.model,
            "system": system,
            "prompt": request.text,
            "stream": false,
            "options": {
                // The same transcript must polish the same way twice. This is
                // an editing pass, not a writing one.
                "temperature": 0,
                "top_p": 1,
            },
        });

        let url = format!("{}/api/generate", self.config.endpoint);
        let call = self.client.post(&url).json(&body).send();

        let response = match tokio::time::timeout(self.config.budget, call).await {
            Ok(Ok(response)) => response,
            Ok(Err(error)) if error.is_connect() => {
                return Err(PolishError::Unreachable(self.config.endpoint.clone()));
            }
            Ok(Err(error)) => return Err(PolishError::Unreadable(error.to_string())),
            Err(_) => return Err(PolishError::TooSlow(self.config.budget)),
        };

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(PolishError::Refused {
                status: status.as_u16(),
                body: body.chars().take(200).collect(),
            });
        }

        let generated: Generated = response
            .json()
            .await
            .map_err(|error| PolishError::Unreadable(error.to_string()))?;

        Ok(guard(request.text, &generated.response, request.strength)?)
    }

    async fn available(&mut self) -> bool {
        self.models().await.is_ok()
    }
}

#[derive(serde::Deserialize)]
struct Generated {
    response: String,
}

#[derive(serde::Deserialize)]
struct Tags {
    models: Vec<TaggedModel>,
}

#[derive(serde::Deserialize)]
struct TaggedModel {
    name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::polish::Strength;

    #[tokio::test]
    async fn a_server_that_is_not_there_is_reported_as_such() {
        let mut ollama = Ollama::new(OllamaConfig {
            // Port 1 is reserved and nothing listens on it.
            endpoint: "http://127.0.0.1:1".to_owned(),
            model: "whatever".to_owned(),
            budget: Duration::from_millis(400),
        })
        .expect("client builds");

        let error = ollama
            .polish(PolishRequest {
                text: "some words",
                strength: Strength::Balanced,
                vocabulary: &[],
            })
            .await
            .expect_err("nothing is listening");

        assert!(
            matches!(error, PolishError::Unreachable(_)),
            "expected unreachable, got {error}"
        );
        assert!(!ollama.available().await);
    }

    #[tokio::test]
    async fn verbatim_never_reaches_the_network() {
        // The endpoint is unreachable on purpose: if verbatim tried to call it,
        // this would fail rather than return the text.
        let mut ollama = Ollama::new(OllamaConfig {
            endpoint: "http://127.0.0.1:1".to_owned(),
            model: "whatever".to_owned(),
            budget: Duration::from_millis(400),
        })
        .expect("client builds");

        let text = "left exactly as it was";
        let polished = ollama
            .polish(PolishRequest {
                text,
                strength: Strength::Verbatim,
                vocabulary: &[],
            })
            .await
            .expect("verbatim cannot fail");

        assert_eq!(polished, text);
    }
}
