//! Allows us to go to type definition for a symbol
use crate::agentic::tool::{errors::ToolError, input::ToolInput, output::ToolOutput, r#type::Tool};
use async_trait::async_trait;

use super::gotodefintion::GoToDefinitionResponse;

/// We are resuing the types from go to definition since the response and the request
/// are the one and the same
pub struct LSPGoToTypeDefinition {
    client: reqwest::Client,
}

impl LSPGoToTypeDefinition {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Tool for LSPGoToTypeDefinition {
    async fn invoke(&self, input: ToolInput) -> Result<ToolOutput, ToolError> {
        let context = input.is_go_to_type_definition()?;
        let editor_endpoint = context.editor_url().to_owned() + "/go_to_type_definition";
        let response = self
            .client
            .post(editor_endpoint)
            .body(serde_json::to_string(&context).map_err(|_e| ToolError::SerdeConversionFailed)?)
            .send()
            .await
            .map_err(|_e| ToolError::ErrorCommunicatingWithEditor)?;
        let response: GoToDefinitionResponse = response
            .json()
            .await
            .map_err(|_e| ToolError::SerdeConversionFailed)?;

        Ok(ToolOutput::GoToTypeDefinition(response))
    }
}
