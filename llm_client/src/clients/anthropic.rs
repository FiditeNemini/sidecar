use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use tokio::sync::mpsc::UnboundedSender;
use tracing::debug;

use crate::provider::{LLMProvider, LLMProviderAPIKeys};

use super::types::{
    LLMClient, LLMClientCompletionRequest, LLMClientCompletionResponse,
    LLMClientCompletionStringRequest, LLMClientError, LLMType,
};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct AnthropicMessage {
    role: String,
    content: String,
}

impl AnthropicMessage {
    pub fn new(role: String, content: String) -> Self {
        Self { role, content }
    }
}

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum AnthropicEvent {
    #[serde(rename = "message_start")]
    MessageStart {
        #[serde(rename = "message")]
        _message: MessageData,
    },
    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        #[serde(rename = "index")]
        _index: u32,
        content_block: ContentBlock,
    },
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta {
        #[serde(rename = "index")]
        _index: u32,
        delta: ContentBlockDelta,
    },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop {
        #[serde(rename = "index")]
        _index: u32,
    },
    #[serde(rename = "message_delta")]
    MessageDelta {
        #[serde(rename = "edit")]
        _delta: MessageDeltaData,
        #[serde(rename = "usage")]
        _usage: Usage,
    },
    #[serde(rename = "message_stop")]
    MessageStop,
}

#[derive(Debug, Deserialize)]
struct MessageData {
    // id: String,
    // #[serde(rename = "type")]
    // message_type: String,
    // role: String,
    // content: Vec<String>,
    // model: String,
    // stop_reason: Option<String>,
    // stop_sequence: Option<String>,
    // usage: Usage,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    // #[serde(rename = "type")]
    // content_block_type: String,
    text: String,
}

#[derive(Debug, Deserialize)]
struct ContentBlockDelta {
    // #[serde(rename = "type")]
    // delta_type: String,
    text: String,
}

#[derive(Debug, Deserialize)]
struct MessageDeltaData {
    // stop_reason: String,
    // stop_sequence: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    // input_tokens: u32,
    // output_tokens: u32,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct AnthropicRequest {
    system: Option<String>,
    messages: Vec<AnthropicMessage>,
    temperature: f32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    model: String,
}

impl AnthropicRequest {
    fn from_client_completion_request(
        completion_request: LLMClientCompletionRequest,
        model_str: String,
    ) -> Self {
        let temperature = completion_request.temperature();
        let max_tokens = match completion_request.get_max_tokens() {
            Some(tokens) => Some(tokens),
            None => Some(4096),
        };
        let messages = completion_request.messages();
        // First we try to find the system message
        let system_message = messages
            .iter()
            .find(|message| message.role().is_system())
            .map(|message| message.content().to_owned());

        let normal_conversation = messages
            .into_iter()
            .filter(|message| message.role().is_user() || message.role().is_assistant())
            .map(|message| {
                AnthropicMessage::new(message.role().to_string(), message.content().to_owned())
            })
            .collect::<Vec<_>>();
        AnthropicRequest {
            system: system_message,
            messages: normal_conversation,
            temperature,
            stream: true,
            max_tokens,
            model: model_str,
        }
    }

    fn from_client_string_request(
        completion_request: LLMClientCompletionStringRequest,
        model_str: String,
    ) -> Self {
        let temperature = completion_request.temperature();
        let max_tokens = completion_request.get_max_tokens();
        let messages = vec![AnthropicMessage::new(
            "user".to_owned(),
            completion_request.prompt().to_owned(),
        )];
        AnthropicRequest {
            system: None,
            messages,
            temperature,
            stream: true,
            max_tokens,
            model: model_str,
        }
    }
}

pub struct AnthropicClient {
    client: reqwest::Client,
    base_url: String,
    chat_endpoint: String,
}

impl AnthropicClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: "https://api.anthropic.com".to_owned(),
            chat_endpoint: "/v1/messages".to_owned(),
        }
    }

    pub fn new_with_custom_urls(base_url: String, chat_endpoint: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url,
            chat_endpoint,
        }
    }

    pub fn chat_endpoint(&self) -> String {
        format!("{}{}", &self.base_url, &self.chat_endpoint)
    }

    fn generate_api_bearer_key(
        &self,
        api_key: LLMProviderAPIKeys,
    ) -> Result<String, LLMClientError> {
        match api_key {
            LLMProviderAPIKeys::Anthropic(api_key) => Ok(api_key.api_key),
            _ => Err(LLMClientError::WrongAPIKeyType),
        }
    }

    fn get_model_string(&self, llm_type: &LLMType) -> Result<String, LLMClientError> {
        match llm_type {
            LLMType::ClaudeOpus => Ok("claude-3-opus-20240229".to_owned()),
            LLMType::ClaudeSonnet => Ok("claude-3-sonnet-20240229".to_owned()),
            LLMType::ClaudeHaiku => Ok("claude-3-haiku-20240307".to_owned()),
            _ => Err(LLMClientError::UnSupportedModel),
        }
    }
}

#[async_trait]
impl LLMClient for AnthropicClient {
    fn client(&self) -> &LLMProvider {
        &LLMProvider::Anthropic
    }

    async fn completion(
        &self,
        api_key: LLMProviderAPIKeys,
        request: LLMClientCompletionRequest,
    ) -> Result<String, LLMClientError> {
        let (sender, _) = tokio::sync::mpsc::unbounded_channel();
        self.stream_completion(api_key, request, sender).await
    }

    async fn stream_completion(
        &self,
        api_key: LLMProviderAPIKeys,
        request: LLMClientCompletionRequest,
        sender: UnboundedSender<LLMClientCompletionResponse>,
    ) -> Result<String, LLMClientError> {
        let endpoint = self.chat_endpoint();
        let model_str = self.get_model_string(request.model())?;
        let message_tokens = request
            .messages()
            .iter()
            .map(|message| message.content().len())
            .collect::<Vec<_>>();
        let mut message_tokens_count = 0;
        message_tokens.into_iter().for_each(|tokens| {
            message_tokens_count += tokens;
        });
        let anthropic_request =
            AnthropicRequest::from_client_completion_request(request, model_str.to_owned());

        // println!("{:?}", &anthropic_request);

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let response_stream = self
            .client
            .post(endpoint)
            .header(
                "x-api-key".to_owned(),
                self.generate_api_bearer_key(api_key)?,
            )
            .header("anthropic-version".to_owned(), "2023-06-01".to_owned())
            .header("content-type".to_owned(), "application/json".to_owned())
            .json(&anthropic_request)
            .send()
            .await
            .map_err(|e| {
                println!("sidecar.anthropic.error: {:?}", &e);
                e
            })?;

        let mut event_source = response_stream.bytes_stream().eventsource();

        // let event_next = event_source.next().await;
        // dbg!(&event_next);

        let mut buffered_string = "".to_owned();
        while let Some(Ok(event)) = event_source.next().await {
            // TODO: debugging this
            let event = serde_json::from_str::<AnthropicEvent>(&event.data);
            match event {
                Ok(AnthropicEvent::ContentBlockStart { content_block, .. }) => {
                    buffered_string = buffered_string + &content_block.text;
                    let _ = sender.send(LLMClientCompletionResponse::new(
                        buffered_string.to_owned(),
                        Some(content_block.text),
                        model_str.to_owned(),
                    ));
                }
                Ok(AnthropicEvent::ContentBlockDelta { delta, .. }) => {
                    buffered_string = buffered_string + &delta.text;
                    let time_now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis();
                    let time_diff = time_now - current_time;
                    debug!(
                        event_name = "anthropic.buffered_string",
                        message_tokens_count = message_tokens_count,
                        generated_tokens_count = &buffered_string.len(),
                        time_taken = time_diff,
                    );
                    let _ = sender.send(LLMClientCompletionResponse::new(
                        buffered_string.to_owned(),
                        Some(delta.text),
                        model_str.to_owned(),
                    ));
                }
                Err(_e) => {
                    // dbg!(e);
                    break;
                }
                _ => {
                    // dbg!(&event);
                }
            }
        }

        println!("{:?}", &buffered_string);

        Ok(buffered_string)
    }

    async fn stream_prompt_completion(
        &self,
        api_key: LLMProviderAPIKeys,
        request: LLMClientCompletionStringRequest,
        sender: UnboundedSender<LLMClientCompletionResponse>,
    ) -> Result<String, LLMClientError> {
        let endpoint = self.chat_endpoint();
        let model_str = self.get_model_string(request.model())?;
        let anthropic_request =
            AnthropicRequest::from_client_string_request(request, model_str.to_owned());

        let mut response_stream = self
            .client
            .post(endpoint)
            .header(
                "x-api-key".to_owned(),
                self.generate_api_bearer_key(api_key)?,
            )
            .header("anthropic-version".to_owned(), "2023-06-01".to_owned())
            .header("content-type".to_owned(), "application/json".to_owned())
            .json(&anthropic_request)
            .send()
            .await?
            .bytes_stream()
            .eventsource();

        let mut buffered_string = "".to_owned();
        while let Some(Ok(event)) = response_stream.next().await {
            let event = serde_json::from_str::<AnthropicEvent>(&event.data);
            match event {
                Ok(AnthropicEvent::ContentBlockStart { content_block, .. }) => {
                    buffered_string = buffered_string + &content_block.text;
                    let _ = sender.send(LLMClientCompletionResponse::new(
                        buffered_string.to_owned(),
                        Some(content_block.text),
                        model_str.to_owned(),
                    ));
                }
                Ok(AnthropicEvent::ContentBlockDelta { delta, .. }) => {
                    buffered_string = buffered_string + &delta.text;
                    let _ = sender.send(LLMClientCompletionResponse::new(
                        buffered_string.to_owned(),
                        Some(delta.text),
                        model_str.to_owned(),
                    ));
                }
                Err(_) => {
                    break;
                }
                _ => {
                    dbg!(&event);
                }
            }
        }

        Ok(buffered_string)
    }
}
