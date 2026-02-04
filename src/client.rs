use crate::config::{Config, Provider};
use crate::context::Context;
use async_openai::{
    Client, config::OpenAIConfig, error::OpenAIError, types::responses::CreateResponseArgs,
};
use serde::{Deserialize, Serialize};

const INSTRUCTIONS: &str = r#"
<system_instructions>
  <role>
    You are an error diagnosis assistant. You receive error output from CLI tools, compilers, and runtimes, along with context about the user's OS, shell, and project type.
  </role>

  <output_format>
    You MUST use this exact format. No deviations.

    CAUSE: One sentence explaining what went wrong and why.

    FIX: The concrete fix — either a command or code snippet. No explanation here, just the fix itself. If it's a command, write it on its own line with no backticks or formatting. If it's a code change, show only the minimal relevant lines.

    That's it. Two sections. Nothing else in standard mode.
  </output_format>

  <modes>
    <mode name="standard" default="true">
      Exactly CAUSE + FIX as described above. Maximum 5 lines total.
    </mode>

    <mode name="verbose">
      When [verbose] flag is present, use this format:

      CAUSE: One sentence.

      WHY: 2-3 sentences with deeper context on why this happens.

      COMMON CAUSES:
      - First common cause
      - Second common cause
      - Third common cause

      FIX: The concrete fix.

      DOCS: One relevant documentation link if applicable. Omit if none.
    </mode>

    <mode name="alt">
      When [alt] flag is present, use this format:

      CAUSE: One sentence.

      FIX 1: The recommended fix.

      FIX 2: An alternative approach.
      TRADE-OFF: One sentence on when to prefer this.

      FIX 3: Another alternative.
      TRADE-OFF: One sentence on when to prefer this.
    </mode>
  </modes>

  <constraints>
    STRICT RULES — violating these is a failure:
    - Use ONLY the section headers specified above (CAUSE, FIX, WHY, etc). No other headers.
    - NO markdown formatting. No backticks, no bold, no bullet points except where specified.
    - NO preamble, greeting, or sign-off.
    - NO repeating the error back to the user.
    - NO "you can also try" or "another option" in standard mode.
    - NO asking clarifying questions.
    - Be specific to the detected project type and language.
    - If a file/line is referenced in the error, mention it in the fix.
    - If you genuinely can't determine the cause, say so in CAUSE and suggest a debugging step in FIX.
    - Keep it terse. Every word must earn its place.
  </constraints>
</system_instructions>
"#;

pub struct RequestClient {
    input: String,
    context: Context,
    config: Config,
}

#[derive(Clone, Copy)]
pub enum RequestMode {
    Standard,
    Verbose,
    Alt,
}

impl RequestClient {
    pub fn new(input: String, context: Context, config: Config) -> Self {
        Self { input, context, config }
    }

    fn gen_prompt(&self, mode: RequestMode) -> String {
        let mode_tag = match mode {
            RequestMode::Standard => "",
            RequestMode::Verbose => " [verbose]",
            RequestMode::Alt => " [alt]",
        };
        format!(
            "{}\n\n<error_output>{}</error_output>{}",
            self.context.as_prompt_context(),
            self.input,
            mode_tag
        )
    }

    fn get_max_tokens(mode: RequestMode) -> u32 {
        match mode {
            RequestMode::Standard => 512,
            RequestMode::Verbose | RequestMode::Alt => 1024,
        }
    }

    pub async fn make_request(&self, mode: RequestMode) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        match self.config.provider {
            Provider::OpenAI => self.request_openai(mode).await,
            Provider::Anthropic => self.request_anthropic(mode).await,
            Provider::Ollama => self.request_ollama(mode).await,
        }
    }

    async fn request_openai(&self, mode: RequestMode) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let client: Client<OpenAIConfig> = Client::new();
        let prompt = self.gen_prompt(mode);
        let request = CreateResponseArgs::default()
            .model(self.config.openai_model())
            .instructions(INSTRUCTIONS)
            .input(prompt)
            .temperature(0.2)
            .max_output_tokens(Self::get_max_tokens(mode))
            .build()?;

        let response = client.responses().create(request).await?;

        if let Some(text) = response.output_text() {
            Ok(text.clone())
        } else {
            Err(OpenAIError::InvalidArgument("Empty response".to_string()).into())
        }
    }

    async fn request_anthropic(&self, mode: RequestMode) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| "ANTHROPIC_API_KEY not set")?;

        let prompt = self.gen_prompt(mode);

        let request_body = AnthropicRequest {
            model: self.config.anthropic_model().to_string(),
            max_tokens: Self::get_max_tokens(mode),
            system: INSTRUCTIONS.to_string(),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: prompt,
            }],
        };

        let client = reqwest::Client::new();
        let response = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Anthropic API error: {}", error_text).into());
        }

        let response_body: AnthropicResponse = response.json().await?;

        response_body
            .content
            .first()
            .map(|c| c.text.clone())
            .ok_or_else(|| "Empty response from Anthropic".into())
    }

    async fn request_ollama(&self, mode: RequestMode) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let prompt = self.gen_prompt(mode);
        let url = format!("{}/api/chat", self.config.ollama_url());

        let request_body = OllamaRequest {
            model: self.config.ollama_model().to_string(),
            messages: vec![
                OllamaMessage {
                    role: "system".to_string(),
                    content: INSTRUCTIONS.to_string(),
                },
                OllamaMessage {
                    role: "user".to_string(),
                    content: prompt,
                },
            ],
            stream: false,
            options: OllamaOptions {
                temperature: 0.2,
                num_predict: Self::get_max_tokens(mode),
            },
        };

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Ollama error: {}. Is Ollama running?", error_text).into());
        }

        let response_body: OllamaResponse = response.json().await?;
        Ok(response_body.message.content)
    }
}

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<AnthropicMessage>,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

#[derive(Deserialize)]
struct AnthropicContent {
    text: String,
}

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Serialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f32,
    num_predict: u32,
}

#[derive(Deserialize)]
struct OllamaResponse {
    message: OllamaResponseMessage,
}

#[derive(Deserialize)]
struct OllamaResponseMessage {
    content: String,
}
