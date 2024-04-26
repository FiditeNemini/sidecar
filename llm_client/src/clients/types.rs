use async_trait::async_trait;
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::fmt;
use thiserror::Error;
use tokio::sync::mpsc::UnboundedSender;

use crate::provider::{LLMProvider, LLMProviderAPIKeys};

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub enum LLMType {
    Mixtral,
    MistralInstruct,
    Gpt4,
    GPT3_5_16k,
    Gpt4_32k,
    Gpt4Turbo,
    DeepSeekCoder1_3BInstruct,
    DeepSeekCoder33BInstruct,
    DeepSeekCoder6BInstruct,
    CodeLLama70BInstruct,
    CodeLlama13BInstruct,
    CodeLlama7BInstruct,
    Llama3_8bInstruct,
    ClaudeOpus,
    ClaudeSonnet,
    ClaudeHaiku,
    PPLXSonnetSmall,
    CohereRerankV3,
    GeminiPro,
    Custom(String),
}

impl Serialize for LLMType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            LLMType::Custom(s) => serializer.serialize_str(s),
            _ => serializer.serialize_str(&format!("{:?}", self)),
        }
    }
}

impl<'de> Deserialize<'de> for LLMType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct LLMTypeVisitor;

        impl<'de> Visitor<'de> for LLMTypeVisitor {
            type Value = LLMType;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string representing an LLMType")
            }

            fn visit_str<E>(self, value: &str) -> Result<LLMType, E>
            where
                E: de::Error,
            {
                match value {
                    "Mixtral" => Ok(LLMType::Mixtral),
                    "MistralInstruct" => Ok(LLMType::MistralInstruct),
                    "Gpt4" => Ok(LLMType::Gpt4),
                    "GPT3_5_16k" => Ok(LLMType::GPT3_5_16k),
                    "Gpt4_32k" => Ok(LLMType::Gpt4_32k),
                    "Gpt4Turbo" => Ok(LLMType::Gpt4Turbo),
                    "DeepSeekCoder1.3BInstruct" => Ok(LLMType::DeepSeekCoder1_3BInstruct),
                    "DeepSeekCoder6BInstruct" => Ok(LLMType::DeepSeekCoder6BInstruct),
                    "CodeLLama70BInstruct" => Ok(LLMType::CodeLLama70BInstruct),
                    "CodeLlama13BInstruct" => Ok(LLMType::CodeLlama13BInstruct),
                    "CodeLlama7BInstruct" => Ok(LLMType::CodeLlama7BInstruct),
                    "DeepSeekCoder33BInstruct" => Ok(LLMType::DeepSeekCoder33BInstruct),
                    "ClaudeOpus" => Ok(LLMType::ClaudeOpus),
                    "ClaudeSonnet" => Ok(LLMType::ClaudeSonnet),
                    "ClaudeHaiku" => Ok(LLMType::ClaudeHaiku),
                    "PPLXSonnetSmall" => Ok(LLMType::PPLXSonnetSmall),
                    "CohereRerankV3" => Ok(LLMType::CohereRerankV3),
                    "GeminiPro1.5" => Ok(LLMType::GeminiPro),
                    "Llama3_8bInstruct" => Ok(LLMType::Llama3_8bInstruct),
                    _ => Ok(LLMType::Custom(value.to_string())),
                }
            }
        }

        deserializer.deserialize_string(LLMTypeVisitor)
    }
}

impl LLMType {
    pub fn is_openai(&self) -> bool {
        matches!(
            self,
            LLMType::Gpt4 | LLMType::GPT3_5_16k | LLMType::Gpt4_32k | LLMType::Gpt4Turbo
        )
    }

    pub fn is_custom(&self) -> bool {
        matches!(self, LLMType::Custom(_))
    }

    pub fn is_anthropic(&self) -> bool {
        matches!(
            self,
            LLMType::ClaudeOpus | LLMType::ClaudeSonnet | LLMType::ClaudeHaiku
        )
    }

    pub fn is_gemini_pro(&self) -> bool {
        self == &LLMType::GeminiPro
    }

    pub fn is_togetherai_model(&self) -> bool {
        matches!(
            self,
            LLMType::CodeLlama13BInstruct
                | LLMType::CodeLlama7BInstruct
                | LLMType::DeepSeekCoder33BInstruct
        )
    }
}

impl fmt::Display for LLMType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LLMType::Mixtral => write!(f, "Mixtral"),
            LLMType::MistralInstruct => write!(f, "MistralInstruct"),
            LLMType::Gpt4 => write!(f, "Gpt4"),
            LLMType::GPT3_5_16k => write!(f, "GPT3_5_16k"),
            LLMType::Gpt4_32k => write!(f, "Gpt4_32k"),
            LLMType::Gpt4Turbo => write!(f, "Gpt4Turbo"),
            LLMType::DeepSeekCoder1_3BInstruct => write!(f, "DeepSeekCoder1.3BInstruct"),
            LLMType::DeepSeekCoder6BInstruct => write!(f, "DeepSeekCoder6BInstruct"),
            LLMType::CodeLLama70BInstruct => write!(f, "CodeLLama70BInstruct"),
            LLMType::CodeLlama13BInstruct => write!(f, "CodeLlama13BInstruct"),
            LLMType::CodeLlama7BInstruct => write!(f, "CodeLlama7BInstruct"),
            LLMType::DeepSeekCoder33BInstruct => write!(f, "DeepSeekCoder33BInstruct"),
            LLMType::ClaudeOpus => write!(f, "ClaudeOpus"),
            LLMType::ClaudeSonnet => write!(f, "ClaudeSonnet"),
            LLMType::ClaudeHaiku => write!(f, "ClaudeHaiku"),
            LLMType::PPLXSonnetSmall => write!(f, "PPLXSonnetSmall"),
            LLMType::CohereRerankV3 => write!(f, "CohereRerankV3"),
            LLMType::Llama3_8bInstruct => write!(f, "Llama3_8bInstruct"),
            LLMType::GeminiPro => write!(f, "GeminiPro1.5"),
            LLMType::Custom(s) => write!(f, "Custom({})", s),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub enum LLMClientRole {
    System,
    User,
    Assistant,
    // function calling is weird, its only supported by openai right now
    // and not other LLMs, so we are going to make this work with the formatters
    // and still keep it as it is
    Function,
}

impl LLMClientRole {
    pub fn is_system(&self) -> bool {
        matches!(self, LLMClientRole::System)
    }

    pub fn is_user(&self) -> bool {
        matches!(self, LLMClientRole::User)
    }

    pub fn is_assistant(&self) -> bool {
        matches!(self, LLMClientRole::Assistant)
    }

    pub fn is_function(&self) -> bool {
        matches!(self, LLMClientRole::Function)
    }

    pub fn to_string(&self) -> String {
        match self {
            LLMClientRole::System => "system".to_owned(),
            LLMClientRole::User => "user".to_owned(),
            LLMClientRole::Assistant => "assistant".to_owned(),
            LLMClientRole::Function => "function".to_owned(),
        }
    }
}

#[derive(serde::Serialize, Debug, Clone)]
pub struct LLMClientMessageFunctionCall {
    name: String,
    // arguments are generally given as a JSON string, so we keep it as a string
    // here, validate in the upper handlers for this
    arguments: String,
}

impl LLMClientMessageFunctionCall {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn arguments(&self) -> &str {
        &self.arguments
    }
}

#[derive(serde::Serialize, Debug, Clone)]
pub struct LLMClientMessageFunctionReturn {
    name: String,
    content: String,
}

impl LLMClientMessageFunctionReturn {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn content(&self) -> &str {
        &self.content
    }
}

#[derive(serde::Serialize, Debug, Clone)]
pub struct LLMClientMessage {
    role: LLMClientRole,
    message: String,
    function_call: Option<LLMClientMessageFunctionCall>,
    function_return: Option<LLMClientMessageFunctionReturn>,
}

impl LLMClientMessage {
    pub fn new(role: LLMClientRole, message: String) -> Self {
        Self {
            role,
            message,
            function_call: None,
            function_return: None,
        }
    }

    pub fn concat_message(&mut self, message: &str) {
        self.message = self.message.to_owned() + "\n" + message;
    }

    pub fn concat(self, other: Self) -> Self {
        // We are going to concatenate the 2 llm client messages togehter, at this moment
        // we are just gonig to join the message with a \n
        Self {
            role: self.role,
            message: self.message + "\n" + &other.message,
            function_call: match self.function_call {
                Some(function_call) => Some(function_call),
                None => other.function_call,
            },
            function_return: match other.function_return {
                Some(function_return) => Some(function_return),
                None => self.function_return,
            },
        }
    }

    pub fn function_call(name: String, arguments: String) -> Self {
        Self {
            role: LLMClientRole::Assistant,
            message: "".to_owned(),
            function_call: Some(LLMClientMessageFunctionCall { name, arguments }),
            function_return: None,
        }
    }

    pub fn function_return(name: String, content: String) -> Self {
        Self {
            role: LLMClientRole::Function,
            message: "".to_owned(),
            function_call: None,
            function_return: Some(LLMClientMessageFunctionReturn { name, content }),
        }
    }

    pub fn user(message: String) -> Self {
        Self::new(LLMClientRole::User, message)
    }

    pub fn assistant(message: String) -> Self {
        Self::new(LLMClientRole::Assistant, message)
    }

    pub fn system(message: String) -> Self {
        Self::new(LLMClientRole::System, message)
    }

    pub fn content(&self) -> &str {
        &self.message
    }

    pub fn set_empty_content(&mut self) {
        self.message =
            "empty message found here, possibly an error but keep following the conversation"
                .to_owned();
    }

    pub fn function(message: String) -> Self {
        Self::new(LLMClientRole::Function, message)
    }

    pub fn role(&self) -> &LLMClientRole {
        &self.role
    }

    pub fn get_function_call(&self) -> Option<&LLMClientMessageFunctionCall> {
        self.function_call.as_ref()
    }

    pub fn get_function_return(&self) -> Option<&LLMClientMessageFunctionReturn> {
        self.function_return.as_ref()
    }
}

#[derive(Clone, Debug)]
pub struct LLMClientCompletionRequest {
    model: LLMType,
    messages: Vec<LLMClientMessage>,
    temperature: f32,
    frequency_penalty: Option<f32>,
    stop_words: Option<Vec<String>>,
    max_tokens: Option<usize>,
}

#[derive(Clone)]
pub struct LLMClientCompletionStringRequest {
    model: LLMType,
    prompt: String,
    temperature: f32,
    frequency_penalty: Option<f32>,
    stop_words: Option<Vec<String>>,
    max_tokens: Option<usize>,
}

impl LLMClientCompletionStringRequest {
    pub fn new(
        model: LLMType,
        prompt: String,
        temperature: f32,
        frequency_penalty: Option<f32>,
    ) -> Self {
        Self {
            model,
            prompt,
            temperature,
            frequency_penalty,
            stop_words: None,
            max_tokens: None,
        }
    }

    pub fn set_stop_words(mut self, stop_words: Vec<String>) -> Self {
        self.stop_words = Some(stop_words);
        self
    }

    pub fn model(&self) -> &LLMType {
        &self.model
    }

    pub fn temperature(&self) -> f32 {
        self.temperature
    }

    pub fn frequency_penalty(&self) -> Option<f32> {
        self.frequency_penalty
    }

    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    pub fn stop_words(&self) -> Option<&[String]> {
        self.stop_words.as_deref()
    }

    pub fn set_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    pub fn get_max_tokens(&self) -> Option<usize> {
        self.max_tokens
    }
}

impl LLMClientCompletionRequest {
    pub fn new(
        model: LLMType,
        messages: Vec<LLMClientMessage>,
        temperature: f32,
        frequency_penalty: Option<f32>,
    ) -> Self {
        Self {
            model,
            messages,
            temperature,
            frequency_penalty,
            stop_words: None,
            max_tokens: None,
        }
    }

    pub fn fix_message_structure(mut self: Self) -> Self {
        // fix here can mean many things, but here we are going to focus on
        // anthropic since there we need alternating human and assistant message
        // if we have repeating human or assistant messages, then we can just compress
        // them to a single message
        if self.model().is_anthropic() {
            let messages = self.messages;
            let mut final_messages = vec![];
            let mut running_message: Option<LLMClientMessage> = None;
            let mut index = 0;
            dbg!("sidecar.fixies_roles");
            while index < messages.len() {
                let current_message = &messages[index];
                index = index + 1;

                if let Some(ref mut running_message_ref) = running_message {
                    if running_message_ref.role() == current_message.role() {
                        running_message_ref.concat_message(current_message.content());
                    } else {
                        if running_message_ref.content().is_empty() {
                            // figure out how to get rid of empty content messages here
                            running_message_ref.set_empty_content();
                        }
                        final_messages.push(running_message_ref.clone());
                        // and also set the running message as the current
                        // message
                        running_message = Some(current_message.clone());
                    }
                } else {
                    running_message = Some(current_message.clone());
                }
            }
            if let Some(mut message) = running_message {
                if message.message.is_empty() {
                    message.set_empty_content();
                }
                final_messages.push(message);
            }
            self.messages = final_messages;
        }
        self
    }

    pub fn from_messages(messages: Vec<LLMClientMessage>, model: LLMType) -> Self {
        Self::new(model, messages, 0.0, None)
    }

    pub fn set_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
        self
    }

    pub fn messages(&self) -> &[LLMClientMessage] {
        self.messages.as_slice()
    }

    pub fn temperature(&self) -> f32 {
        self.temperature
    }

    pub fn frequency_penalty(&self) -> Option<f32> {
        self.frequency_penalty
    }

    pub fn model(&self) -> &LLMType {
        &self.model
    }

    pub fn stop_words(&self) -> Option<&[String]> {
        self.stop_words.as_deref()
    }

    pub fn set_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    pub fn get_max_tokens(&self) -> Option<usize> {
        self.max_tokens
    }
}

#[derive(Debug)]
pub struct LLMClientCompletionResponse {
    answer_up_until_now: String,
    delta: Option<String>,
    model: String,
}

impl LLMClientCompletionResponse {
    pub fn new(answer_up_until_now: String, delta: Option<String>, model: String) -> Self {
        Self {
            answer_up_until_now,
            delta,
            model,
        }
    }

    pub fn get_answer_up_until_now(self) -> String {
        self.answer_up_until_now
    }

    pub fn answer_up_until_now(&self) -> &str {
        &self.answer_up_until_now
    }

    pub fn delta(&self) -> Option<&str> {
        self.delta.as_deref()
    }

    pub fn model(&self) -> &str {
        &self.model
    }
}

#[derive(Error, Debug)]
pub enum LLMClientError {
    #[error("Failed to get response from LLM")]
    FailedToGetResponse,

    #[error("Reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),

    #[error("serde failed: {0}")]
    SerdeError(#[from] serde_json::Error),

    #[error("send error over channel: {0}")]
    SendError(#[from] tokio::sync::mpsc::error::SendError<LLMClientCompletionResponse>),

    #[error("unsupported model")]
    UnSupportedModel,

    #[error("OpenAI api error: {0}")]
    OpenAPIError(#[from] async_openai::error::OpenAIError),

    #[error("Wrong api key type")]
    WrongAPIKeyType,

    #[error("OpenAI does not support completion")]
    OpenAIDoesNotSupportCompletion,

    #[error("Sqlite setup error")]
    SqliteSetupError,

    #[error("tokio mspc error")]
    TokioMpscSendError,

    #[error("Failed to store in sqlite DB")]
    FailedToStoreInDB,

    #[error("Sqlx erorr: {0}")]
    SqlxError(#[from] sqlx::Error),

    #[error("Function calling role but not function call present")]
    FunctionCallNotPresent,

    #[error("Gemini pro does not support prompt completion")]
    GeminiProDoesNotSupportPromptCompletion,
}

#[async_trait]
pub trait LLMClient {
    fn client(&self) -> &LLMProvider;

    async fn stream_completion(
        &self,
        api_key: LLMProviderAPIKeys,
        request: LLMClientCompletionRequest,
        sender: UnboundedSender<LLMClientCompletionResponse>,
    ) -> Result<String, LLMClientError>;

    async fn completion(
        &self,
        api_key: LLMProviderAPIKeys,
        request: LLMClientCompletionRequest,
    ) -> Result<String, LLMClientError>;

    async fn stream_prompt_completion(
        &self,
        api_key: LLMProviderAPIKeys,
        request: LLMClientCompletionStringRequest,
        sender: UnboundedSender<LLMClientCompletionResponse>,
    ) -> Result<String, LLMClientError>;
}

#[cfg(test)]
mod tests {
    use super::LLMType;

    #[test]
    fn test_llm_type_from_string() {
        let llm_type = LLMType::Custom("skcd_testing".to_owned());
        let str_llm_type = serde_json::to_string(&llm_type).expect("to work");
        assert_eq!(str_llm_type, "");
    }
}
