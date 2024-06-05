//! This helps us search the repo map to get the relevant information

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use llm_client::{
    broker::LLMBroker,
    clients::types::LLMType,
    provider::{LLMProvider, LLMProviderAPIKeys},
};

use crate::agentic::tool::{base::Tool, errors::ToolError, input::ToolInput, output::ToolOutput};

use super::{
    important::CodeSymbolImportantResponse, models::anthropic::AnthropicCodeSymbolImportant,
    types::CodeSymbolError,
};

// TODO(skcd): We need to figure out here how to get the gemini flash/pro
// models to be the default ones which we can use for searching
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RepoMapSearchQuery {
    repo_map: String,
    user_query: String,
    llm: LLMType,
    provider: LLMProvider,
    api_keys: LLMProviderAPIKeys,
}

impl RepoMapSearchQuery {
    pub fn new(
        repo_map: String,
        user_query: String,
        llm: LLMType,
        provider: LLMProvider,
        api_keys: LLMProviderAPIKeys,
    ) -> Self {
        Self {
            repo_map,
            user_query,
            llm,
            provider,
            api_keys,
        }
    }

    pub fn repo_map(&self) -> &str {
        &self.repo_map
    }

    pub fn user_query(&self) -> &str {
        &self.user_query
    }

    pub fn llm(&self) -> &LLMType {
        &self.llm
    }

    pub fn provider(&self) -> &LLMProvider {
        &self.provider
    }

    pub fn api_keys(&self) -> &LLMProviderAPIKeys {
        &self.api_keys
    }
}

/// Trait definition for searhing on the repo map and getting the initial response
/// back
#[async_trait]
pub trait RepoMapSearch {
    async fn get_repo_symbols(
        &self,
        request: RepoMapSearchQuery,
    ) -> Result<CodeSymbolImportantResponse, CodeSymbolError>;
}

pub struct RepoMapSearchBroker {
    llms: HashMap<LLMType, Box<dyn RepoMapSearch + Send + Sync>>,
}

impl RepoMapSearchBroker {
    pub fn new(llm_client: Arc<LLMBroker>) -> Self {
        let mut llms: HashMap<LLMType, Box<dyn RepoMapSearch + Send + Sync>> = Default::default();
        llms.insert(
            LLMType::ClaudeHaiku,
            Box::new(AnthropicCodeSymbolImportant::new(llm_client.clone())),
        );
        llms.insert(
            LLMType::ClaudeSonnet,
            Box::new(AnthropicCodeSymbolImportant::new(llm_client.clone())),
        );
        llms.insert(
            LLMType::ClaudeOpus,
            Box::new(AnthropicCodeSymbolImportant::new(llm_client.clone())),
        );
        llms.insert(
            LLMType::GeminiPro,
            Box::new(AnthropicCodeSymbolImportant::new(llm_client.clone())),
        );
        llms.insert(
            LLMType::GeminiProFlash,
            Box::new(AnthropicCodeSymbolImportant::new(llm_client.clone())),
        );
        Self { llms }
    }
}

#[async_trait]
impl Tool for RepoMapSearchBroker {
    async fn invoke(&self, input: ToolInput) -> Result<ToolOutput, ToolError> {
        let context = input.repo_map_search_query()?;
        if let Some(implementation) = self.llms.get(context.llm()) {
            let output = implementation
                .get_repo_symbols(context)
                .await
                .map_err(|e| ToolError::CodeSymbolError(e))?;
            Ok(ToolOutput::RepoMapSearch(output))
        } else {
            Err(ToolError::LLMNotSupported)
        }
    }
}