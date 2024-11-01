use async_trait::async_trait;
use futures::StreamExt;
use quick_xml::de::from_str;
use serde::Deserialize;
use std::{sync::Arc, time::Instant};
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use llm_client::{
    broker::LLMBroker,
    clients::types::{LLMClientCompletionRequest, LLMClientMessage, LLMType},
    provider::{CodeStoryLLMTypes, CodestoryAccessToken, LLMProvider, LLMProviderAPIKeys},
};

use crate::{
    agentic::{
        symbol::{identifier::LLMProperties, ui_event::UIEventWithID},
        tool::{
            errors::ToolError,
            helpers::cancellation_future::run_with_cancellation,
            input::ToolInput,
            lsp::file_diagnostics::DiagnosticMap,
            output::ToolOutput,
            r#type::Tool,
            session::chat::{SessionChatMessage, SessionChatRole},
        },
    },
    user_context::types::UserContext,
};

use super::plan_step::PlanStep;

pub struct StepTitleFound {
    step_index: usize,
    session_id: String,
    exchange_id: String,
    title: String,
}

impl StepTitleFound {
    fn new(step_index: usize, title: String, session_id: String, exchange_id: String) -> Self {
        Self {
            step_index,
            session_id,
            exchange_id,
            title,
        }
    }

    pub fn step_index(&self) -> usize {
        self.step_index
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn exchange_id(&self) -> &str {
        &self.exchange_id
    }

    pub fn title(&self) -> &str {
        &self.title
    }
}

pub struct StepDescriptionUpdate {
    delta: Option<String>,
    description_up_until_now: String,
    session_id: String,
    exchange_id: String,
    index: usize,
}

impl StepDescriptionUpdate {
    fn new(
        delta: Option<String>,
        description_up_until_now: String,
        session_id: String,
        exchange_id: String,
        index: usize,
    ) -> Self {
        Self {
            delta,
            description_up_until_now,
            session_id,
            exchange_id,
            index,
        }
    }

    pub fn delta(&self) -> Option<String> {
        self.delta.clone()
    }

    pub fn description_up_until_now(&self) -> &str {
        &self.description_up_until_now
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn exchange_id(&self) -> &str {
        &self.exchange_id
    }

    pub fn index(&self) -> usize {
        self.index
    }
}

/// Exposes 2 kinds of events one which notifies that a new step has been
/// added and another which is the case when the steps are done
pub enum StepSenderEvent {
    NewStep(Step),
    NewStepTitle(StepTitleFound),
    NewStepDescription(StepDescriptionUpdate),
    Done,
}

// consider possibility of constraining number of steps
#[derive(Debug, Clone)]
pub struct StepGeneratorRequest {
    user_query: String,
    previous_messages: Vec<SessionChatMessage>,
    user_context: Option<UserContext>,
    is_deep_reasoning: bool,
    root_request_id: String,
    editor_url: String,
    diagnostics: Option<DiagnosticMap>,
    // the exchange id which belongs to the session
    exchange_id: String,
    ui_event: UnboundedSender<UIEventWithID>,
    // if we should stream the steps which we are generating
    // the caller takes care of reacting to this stream if they are interested in
    // this
    stream_steps: Option<UnboundedSender<StepSenderEvent>>,
    cancellation_token: tokio_util::sync::CancellationToken,
    access_token: String,
}

impl StepGeneratorRequest {
    pub fn new(
        user_query: String,
        is_deep_reasoning: bool,
        previous_messages: Vec<SessionChatMessage>,
        root_request_id: String,
        editor_url: String,
        exchange_id: String,
        ui_event: UnboundedSender<UIEventWithID>,
        stream_steps: Option<UnboundedSender<StepSenderEvent>>,
        cancellation_token: tokio_util::sync::CancellationToken,
        access_token: String,
    ) -> Self {
        Self {
            user_query,
            previous_messages,
            root_request_id,
            editor_url,
            is_deep_reasoning,
            user_context: None,
            diagnostics: None,
            exchange_id,
            ui_event,
            stream_steps,
            cancellation_token,
            access_token,
        }
    }

    pub fn user_query(&self) -> &str {
        &self.user_query
    }

    pub fn access_token(&self) -> &str {
        &self.access_token
    }

    pub fn root_request_id(&self) -> &str {
        &self.root_request_id
    }

    pub fn editor_url(&self) -> &str {
        &self.editor_url
    }

    pub fn diagnostics(&self) -> Option<&DiagnosticMap> {
        self.diagnostics.as_ref()
    }

    pub fn with_user_context(mut self, user_context: &UserContext) -> Self {
        self.user_context = Some(user_context.to_owned());
        self
    }

    pub fn with_diagnostics(mut self, diagnostics: DiagnosticMap) -> Self {
        self.diagnostics = Some(diagnostics);
        self
    }

    pub fn user_context(&self) -> Option<&UserContext> {
        self.user_context.as_ref()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename = "steps")]
#[serde(rename_all = "lowercase")]
pub struct StepGeneratorResponse {
    pub step: Vec<Step>,
    human_help: Option<String>,
}

impl StepGeneratorResponse {
    pub fn into_steps(self) -> Vec<Step> {
        self.step
    }

    pub fn into_plan_steps(self) -> Vec<PlanStep> {
        let plan_steps = self
            .step
            .into_iter()
            .map(|step| step.into_plan_step())
            .collect::<Vec<_>>();

        plan_steps
    }

    pub fn huamn_help(&self) -> Option<String> {
        self.human_help.clone()
    }

    pub fn set_human_help(mut self, help: String) -> Self {
        self.human_help = Some(help);
        self
    }
}

impl StepGeneratorResponse {
    pub fn parse_response(response: &str) -> Result<Self, ToolError> {
        let response = response
            .lines()
            .into_iter()
            .skip_while(|line| !line.contains("<response>"))
            .skip(1)
            .take_while(|line| !line.contains("</response>"))
            .collect::<Vec<&str>>()
            .join("\n");

        from_str::<Self>(&response).map_err(|e| {
            println!("{:?}", e);
            ToolError::SerdeConversionFailed
        })
    }

    pub fn grab_human_ask_for_help(response: &str) -> Option<String> {
        let response = response
            .lines()
            .into_iter()
            .skip_while(|line| !line.contains("<ask_human_for_help>"))
            .skip(1)
            .take_while(|line| !line.contains("</ask_human_for_help>"))
            .collect::<Vec<&str>>()
            .join("\n");
        Some(response)
    }
}

#[derive(Debug, Deserialize, Clone, serde::Serialize)]
pub struct Step {
    pub files_to_edit: FilesToEdit,
    pub title: String,
    pub changes: String,
}

impl Step {
    pub fn into_plan_step(self) -> PlanStep {
        PlanStep::new(
            Uuid::new_v4().to_string(),
            self.files_to_edit.file,
            self.title,
            self.changes,
            UserContext::new(vec![], vec![], None, vec![]),
        )
    }

    pub fn file_to_edit(&self) -> Option<String> {
        self.files_to_edit.file.first().cloned()
    }

    pub fn description(&self) -> &str {
        &self.changes
    }
}

#[derive(Debug, Deserialize, Clone, serde::Serialize)]
pub struct FilesToEdit {
    pub file: Vec<String>,
}

pub struct StepGeneratorClient {
    llm_client: Arc<LLMBroker>,
}

impl StepGeneratorClient {
    pub fn new(llm_client: Arc<LLMBroker>) -> Self {
        Self { llm_client }
    }

    pub fn plan_schema() -> String {
        format!(
            r#"<response>
<steps>
<step>
<files_to_edit>
<file>
/Users/zi/codestory/sidecar/sidecar/src/agentic/tool/lib.rs
</file>
<file>
/Users/zi/codestory/sidecar/sidecar/src/agentic/tool/main.rs
</file>
</files_to_edit>
<title>
Represent Execution State if Necessary
</title>
<changes>
If you need to track whether a step is paused, pending, or completed, you can introduce an ExecutionState enum:

```rust
pub struct PlanStep {{
    // ... existing fields ...
    execution_state: ExecutionState,
}}
```
Reasons for this approach:

State Management: Clearly represents the current state of the step's execution.
Extensibility: Allows for additional states in the future if needed (e.g., Failed, Skipped).
Separation of Concerns: Keeps execution state separate from other data, making the code cleaner and more maintainable.
</changes>
</step>
</steps>
</response>"#
        )
    }

    pub fn system_message() -> String {
        format!(
            r#"You are a senior software engineer, expert planner and system architect.
- Given a request and context, you will generate a step by step plan to accomplish it. Use prior art seen in context where applicable.
- Your job is to be precise and effective, so avoid extraneous steps even if they offer convenience.
- Do not talk about testing out the changes unless you are instructed to do so.
- Please ensure that each step includes all required fields and that the steps are logically ordered.
- Please ensure each code block you emit is INDENTED either using spaces or tabs the original context.
- Always give the full path in <file> section, do not use the user friendly name but the original path as present on the disk.
- Each step you suggest must only change a single file and must be a logical unit of work, logic units of work are defined as code changes where the change is complete and encapsulates a logical step forward.
For example, if you have to import a helper function and use it in the code, it should be combined to a single step instead of it being 2 steps, one which imports the helper function and another which makes the changes.
- Do not leave placeholder code when its the critical section of the code which you know needs to change
- Since an editing system will depend your exact instructions, they must be precise. Include abridged code snippets and reasoning if it helps clarify but make sure the changes are complete and never leave core part of the logic or `// .. rest of the code` in the output
- DO NOT suggest any changes for the files which you can not see in your context.
- Your response must strictly follow the following schema:
<response>
<steps>
{{There can be as many steps as you need}}
<step>
<files_to_edit>
<file>
{{File you want to edit or CREATE a new file if required}}
</file>
</files_to_edit>
<title>
{{The title for the change you are about to make}}
</title>
<changes>
{{The changes you want to make along with your thoughts the code here should be interleaved with // ... rest of the code only containing the necessary changes in total}}
</changes>
</step>
</steps>
</response>

Below we show you an example of how the output will look like:
{}

Each xml tag in the response should be in its own line and the content in the xml tag should be on the line after the xml tag. This is essential because we are going to be parsing the output as it is generating line by line"#,
            Self::plan_schema()
        )
    }

    // TODO(skcd): Send a reminder about the perferred output over here
    pub async fn user_message(user_query: &str, user_context: Option<&UserContext>) -> String {
        let context_xml = match user_context {
            Some(ctx) => match ctx.to_owned().to_xml(Default::default()).await {
                Ok(xml) => xml,
                Err(e) => {
                    eprintln!("Failed to convert context to XML: {:?}", e);
                    String::from("No context")
                }
            },
            None => String::from("No context"),
        };

        let reminder_for_format = r#"As as reminder your format for reply is strictly this:
- Your response must strictly follow the following schema:
<response>
<steps>
{{There can be as many steps as you need}}
<step>
<files_to_edit>
<file>
{{File you want to edit or CREATE a new file if required}}
</file>
</files_to_edit>
<title>
{{The title for the change you are about to make}}
</title>
<changes>
{{The changes you want to make along with your thoughts the code here should be interleaved with // ... rest of the code only containing the necessary changes in total}}
</changes>
</step>
</steps>
</response>"#;

        format!(
            "Context:
{context_xml}
---
Request:
{user_query}
---
Reminder for format:
{reminder_for_format}"
        )
    }
}

#[async_trait]
impl Tool for StepGeneratorClient {
    async fn invoke(&self, input: ToolInput) -> Result<ToolOutput, ToolError> {
        let context = ToolInput::step_generator(input)?;

        let previous_messages = context.previous_messages.to_vec();
        let _editor_url = context.editor_url.to_owned();
        let session_id = context.root_request_id.to_owned();
        let cancellation_token = context.cancellation_token.clone();
        let exchange_id = context.exchange_id.to_owned();
        let ui_sender = context.ui_event.clone();
        let root_id = context.root_request_id.to_owned();
        let is_deep_reasoning = context.is_deep_reasoning;
        let stream_steps = context.stream_steps.clone();

        let mut messages = vec![LLMClientMessage::system(Self::system_message())];
        // Add the previous running messages over here
        messages.extend(previous_messages.into_iter().map(|previous_message| {
            match previous_message.role() {
                SessionChatRole::User => {
                    LLMClientMessage::user(previous_message.message().to_owned())
                }
                SessionChatRole::Assistant => {
                    LLMClientMessage::assistant(previous_message.message().to_owned())
                }
            }
        }));
        messages.push(LLMClientMessage::user(
            Self::user_message(context.user_query(), context.user_context()).await,
        ));

        // delete!
        let is_deep_reasoning = true;

        let request = if is_deep_reasoning {
            LLMClientCompletionRequest::new(LLMType::O1Preview, messages, 0.2, None)
        } else {
            LLMClientCompletionRequest::new(LLMType::ClaudeSonnet, messages, 0.2, None)
        };

        let llm_provider = LLMProvider::CodeStory(CodeStoryLLMTypes::new());
        let codestory_access_token = CodestoryAccessToken {
            access_token: context.access_token.clone(),
        };

        let llm_properties = if is_deep_reasoning {
            LLMProperties::new(
                LLMType::O1Preview,
                llm_provider,
                LLMProviderAPIKeys::CodeStory(codestory_access_token),
            )
        } else {
            LLMProperties::new(
                LLMType::ClaudeSonnet,
                llm_provider,
                LLMProviderAPIKeys::CodeStory(codestory_access_token),
            )
        };
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();

        let start_time = Instant::now();

        let cloned_llm_client = self.llm_client.clone();

        // we have to poll both the futures in parallel and stream it back to the
        // editor so we can get it as quickly as possible
        let llm_response = run_with_cancellation(
            cancellation_token,
            tokio::spawn(async move {
                cloned_llm_client
                    .stream_completion(
                        llm_properties.api_key().clone(),
                        request,
                        llm_properties.provider().clone(),
                        vec![
                            ("root_id".to_owned(), root_id),
                            ("event_type".to_owned(), "generate_steps".to_owned()),
                        ]
                        .into_iter()
                        .collect(),
                        sender,
                    )
                    .await
            }),
        );

        let mut delta_stream = tokio_stream::wrappers::UnboundedReceiverStream::new(receiver);
        let mut plan_step_incremental_parser =
            PlanStepGenerator::new(session_id, exchange_id, ui_sender, stream_steps.clone());
        while let Some(stream_msg) = delta_stream.next().await {
            let delta = stream_msg.delta();
            if let Some(delta) = delta {
                plan_step_incremental_parser
                    .add_delta(delta.to_owned())
                    .await;
            }
        }

        // force flush the remaining part of the stream over here
        println!("step_generator_client::flusing_entries");
        plan_step_incremental_parser.process_answer().await;

        let elapsed_time = start_time.elapsed();
        println!("LLM request took: {:?}", elapsed_time);
        // now close the step sender since we are not going to be sending any more steps
        // at this point
        if let Some(stream_steps) = stream_steps {
            // we want to send a close event over here to let the receiver know that
            // we are done streaming all the steps which we have
            let _ = stream_steps.send(StepSenderEvent::Done);
        }

        match llm_response.await {
            Some(Ok(Ok(_))) => {
                let steps = plan_step_incremental_parser.generated_steps;
                Ok(ToolOutput::StepGenerator(StepGeneratorResponse {
                    step: steps,
                    human_help: None,
                }))
            }
            _ => Err(ToolError::RetriesExhausted),
        }
    }
}

/// The various parts of the steps which we have on the stream
#[derive(Debug, Clone)]
enum StepBlockStatus {
    NoBlock,
    StepStart,
    StepFile,
    StepTitle,
    StepDescription,
}

/// Takes care of generating the plan steps and making sure we can parse it
/// while its arriving on the stream
struct PlanStepGenerator {
    stream_up_until_now: String,
    step_block_status: StepBlockStatus,
    previous_answer_line_number: Option<usize>,
    current_files_to_edit: Vec<String>,
    current_title: Option<String>,
    current_description: Option<String>,
    generated_steps: Vec<Step>,
    session_id: String,
    exchange_id: String,
    ui_sender: UnboundedSender<UIEventWithID>,
    stream_steps: Option<UnboundedSender<StepSenderEvent>>,
    step_index: usize,
}

impl PlanStepGenerator {
    pub fn new(
        session_id: String,
        exchange_id: String,
        ui_sender: UnboundedSender<UIEventWithID>,
        stream_steps: Option<UnboundedSender<StepSenderEvent>>,
    ) -> Self {
        Self {
            stream_up_until_now: "".to_owned(),
            step_block_status: StepBlockStatus::NoBlock,
            previous_answer_line_number: None,
            current_files_to_edit: vec![],
            current_title: None,
            current_description: None,
            generated_steps: vec![],
            session_id,
            exchange_id,
            ui_sender,
            stream_steps,
            step_index: 0,
        }
    }

    fn generate_step_if_possible(&mut self) {
        match (self.current_title.clone(), self.current_description.clone()) {
            // at this point since we have a plan step which we are generating
            // we should also send the ui_event for this so the editor is updated in real time
            (Some(title), Some(description)) => {
                let step = Step {
                    files_to_edit: FilesToEdit {
                        file: self.current_files_to_edit.to_vec(),
                    },
                    title,
                    changes: description,
                };
                // send over the ui event here for the step
                let _ = self.ui_sender.send(UIEventWithID::plan_complete_added(
                    self.session_id.to_owned(),
                    self.exchange_id.to_owned(),
                    self.generated_steps.len(),
                    step.files_to_edit.file.to_vec(),
                    step.title.to_owned(),
                    step.changes.to_owned(),
                ));
                if let Some(step_sender) = self.stream_steps.clone() {
                    let _ = step_sender.send(StepSenderEvent::NewStep(step.clone()));
                }
                self.generated_steps.push(step);
            }
            _ => {}
        }
        self.current_title = None;
        self.current_description = None;
        self.current_files_to_edit = vec![];
    }

    async fn add_delta(&mut self, delta: String) {
        self.stream_up_until_now.push_str(&delta);
        self.process_answer().await;
    }

    async fn process_answer(&mut self) {
        let line_number_to_process = get_last_newline_line_number(&self.stream_up_until_now);
        if line_number_to_process.is_none() {
            return;
        }
        let line_number_to_process_until =
            line_number_to_process.expect("is_none to hold above") - 1;

        let stream_lines = self.stream_up_until_now.to_owned();
        let stream_lines = stream_lines.lines().into_iter().collect::<Vec<_>>();

        let start_index = self
            .previous_answer_line_number
            .map_or(0, |line_number| line_number + 1);

        for line_number in start_index..=line_number_to_process_until {
            self.previous_answer_line_number = Some(line_number);
            let answer_line_at_index = stream_lines[line_number];

            match self.step_block_status.clone() {
                StepBlockStatus::NoBlock => {
                    // we had no block but found the start of a step over here
                    if answer_line_at_index == "<step>" {
                        self.step_block_status = StepBlockStatus::StepStart;
                    }
                }
                StepBlockStatus::StepStart => {
                    if answer_line_at_index == "<file>" {
                        self.step_block_status = StepBlockStatus::StepFile;
                    } else if answer_line_at_index == "<title>" {
                        self.step_block_status = StepBlockStatus::StepTitle;
                    } else if answer_line_at_index == "<changes>" {
                        self.step_block_status = StepBlockStatus::StepDescription;
                    } else if answer_line_at_index == "</step>" {
                        // over here we should have a plan at this point which we can
                        // send back, for now we can start by logging this properly
                        self.generate_step_if_possible();

                        // increment our counter for the step index and move our state forward
                        self.step_index = self.step_index + 1;
                        self.step_block_status = StepBlockStatus::NoBlock;
                    }
                }
                StepBlockStatus::StepFile => {
                    if answer_line_at_index == "</file>" {
                        self.step_block_status = StepBlockStatus::StepStart;
                    } else {
                        self.current_files_to_edit
                            .push(answer_line_at_index.to_owned());
                    }
                }
                StepBlockStatus::StepTitle => {
                    if answer_line_at_index == "</title>" {
                        // send the title if we do have it for the plan step
                        if let Some(step_sender) = self.stream_steps.as_ref() {
                            if let Some(title) = self.current_title.as_ref() {
                                let _ = step_sender.send(StepSenderEvent::NewStepTitle(
                                    StepTitleFound::new(
                                        self.step_index,
                                        title.to_owned(),
                                        self.session_id.to_owned(),
                                        self.exchange_id.to_owned(),
                                    ),
                                ));
                            }
                        }
                        self.step_block_status = StepBlockStatus::StepStart;
                    } else {
                        match self.current_title.clone() {
                            Some(title) => {
                                self.current_title = Some(title + "\n" + answer_line_at_index);
                            }
                            None => {
                                self.current_title = Some(answer_line_at_index.to_owned());
                            }
                        }
                    }
                }
                StepBlockStatus::StepDescription => {
                    if answer_line_at_index == "</changes>" {
                        self.step_block_status = StepBlockStatus::StepStart;
                    } else {
                        match self.current_description.clone() {
                            Some(description) => {
                                self.current_description =
                                    Some(description + "\n" + answer_line_at_index);
                            }
                            None => {
                                self.current_description = Some(answer_line_at_index.to_owned());
                            }
                        }
                        // send over the description and the delta over here
                        if let Some(step_sender) = self.stream_steps.as_ref() {
                            if let Some(description) = self.current_description.as_ref() {
                                let _ = step_sender.send(StepSenderEvent::NewStepDescription(
                                    StepDescriptionUpdate::new(
                                        Some(answer_line_at_index.to_owned()),
                                        description.to_owned(),
                                        self.session_id.to_owned(),
                                        self.exchange_id.to_owned(),
                                        self.step_index,
                                    ),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Helps to get the last line number which has a \n
fn get_last_newline_line_number(s: &str) -> Option<usize> {
    s.rfind('\n')
        .map(|last_index| s[..=last_index].chars().filter(|&c| c == '\n').count())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_response_with_cdata() {
        let input = r#"Certainly! I'll create a stepped plan to implement a new Tool called StepGeneratorClient, similar to the ReasoningClient. Here's the plan:

<response>
<steps>
<step>
<files_to_edit>
<file>
/Users/zi/codestory/sidecar/sidecar/src/agentic/tool/plan/generator.rs
</file>
</files_to_edit>
<title>
Create StepGeneratorClient struct and implement basic methods
</title>
<changes>
Create a new file `generator.rs` in the `plan` directory. Define the `StepGeneratorClient` struct and implement basic methods:

```rust
use async_trait::async_trait;
use std::sync::Arc;
use llm_client::broker::LLMBroker;

pub struct StepGeneratorClient {
    llm_client: Arc<LLMBroker>,
}

impl StepGeneratorClient {
    pub fn new(llm_client: Arc<LLMBroker>) -> Self {
        Self { llm_client }
    }

    fn user_message(&self, context: StepGeneratorRequest) -> String {
        // Implement the user message formatting logic here
        // Similar to ReasoningClient's user_message method
    }
}
```
</changes>
</step>

<step>
<files_to_edit>
<file>
/Users/zi/codestory/sidecar/sidecar/src/agentic/tool/plan/generator.rs
</file>
</files_to_edit>
<title>
Define StepGeneratorRequest and StepGeneratorResponse structs
</title>
<changes>
Add the following structs to `generator.rs`:

```rust
#[derive(Debug, Clone)]
pub struct StepGeneratorResponse {
    response: String,
}

impl StepGeneratorResponse {
    pub fn response(self) -> String {
        self.response
    }
}

#[derive(Debug, Clone)]
pub struct StepGeneratorRequest {
    user_query: String,
    current_plan: String,
    context: String,
    // Add other necessary fields
}

impl StepGeneratorRequest {
    pub fn new(user_query: String, current_plan: String, context: String) -> Self {
        Self {
            user_query,
            current_plan,
            context,
        }
    }
}
```
</changes>
</step>

<step>
<files_to_edit>
<file>
/Users/zi/codestory/sidecar/sidecar/src/agentic/tool/plan/generator.rs
</file>
</files_to_edit>
<title>
Implement the Tool trait for StepGeneratorClient
</title>
<changes>
Implement the `Tool` trait for `StepGeneratorClient`:

```rust
use crate::agentic::tool::{errors::ToolError, input::ToolInput, output::ToolOutput, r#type::Tool};

#[async_trait]
impl Tool for StepGeneratorClient {
    async fn invoke(&self, input: ToolInput) -> Result<ToolOutput, ToolError> {
        let context = input.step_generator()?;
        
        // Implement the logic to generate steps here
        // Use self.llm_client to make API calls similar to ReasoningClient
        
        // For now, return a placeholder response
        Ok(ToolOutput::step_generator(StepGeneratorResponse {
            response: "Placeholder step generator response".to_string(),
        }))
    }
}
```
</changes>
</step>

<step>
<files_to_edit>
<file>
/Users/zi/codestory/sidecar/sidecar/src/agentic/tool/input.rs
</file>
</files_to_edit>
<title>
Update ToolInput enum to include StepGenerator
</title>
<changes>
Add a new variant to the `ToolInput` enum in `input.rs`:

```rust
pub enum ToolInput {
    // ... existing variants ...
    GenerateStep(StepGeneratorRequest),
}

impl ToolInput {
    // ... existing methods ...

    pub fn step_generator(self) -> Result<StepGeneratorRequest, ToolError> {
        if let ToolInput::GenerateStep(request) = self {
            Ok(request)
        } else {
            Err(ToolError::WrongToolInput(ToolType::StepGenerator))
        }
    }
}
```
</changes>
</step>

<step>
<files_to_edit>
<file>
/Users/zi/codestory/sidecar/sidecar/src/agentic/tool/output.rs
</file>
</files_to_edit>
<title>
Update ToolOutput enum to include StepGenerator
</title>
<changes>
Add a new variant to the `ToolOutput` enum in `output.rs`:

```rust
pub enum ToolOutput {
    // ... existing variants ...
    StepGenerator(StepGeneratorResponse),
}

impl ToolOutput {
    // ... existing methods ...

    pub fn step_generator(response: StepGeneratorResponse) -> Self {
        ToolOutput::StepGenerator(response)
    }

    pub fn get_step_generator_output(self) -> Option<StepGeneratorResponse> {
        match self {
            ToolOutput::StepGenerator(response) => Some(response),
            _ => None,
        }
    }
}
```
</changes>
</step>

<step>
<files_to_edit>
<file>
/Users/zi/codestory/sidecar/sidecar/src/agentic/tool/type.rs
</file>
</files_to_edit>
<title>
Update ToolType enum to include StepGenerator
</title>
<changes>
Add a new variant to the `ToolType` enum in `type.rs`:

```rust
pub enum ToolType {
    // ... existing variants ...
    StepGenerator,
}

impl std::fmt::Display for ToolType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // ... existing matches ...
            ToolType::StepGenerator => write!(f, "Step generator"),
        }
    }
}
```
</changes>
</step>

<step>
<files_to_edit>
<file>
/Users/zi/codestory/sidecar/sidecar/src/agentic/tool/broker.rs
</file>
</files_to_edit>
<title>
Update ToolBroker to include StepGeneratorClient
</title>
<changes>
Update the `ToolBroker::new` method in `broker.rs` to include the `StepGeneratorClient`:

```rust
use super::plan::generator::StepGeneratorClient;

impl ToolBroker {
    pub fn new(
        // ... existing parameters ...
    ) -> Self {
        let mut tools: HashMap<ToolType, Box<dyn Tool + Send + Sync>> = Default::default();
        
        // ... existing tool insertions ...

        tools.insert(
            ToolType::StepGenerator,
            Box::new(StepGeneratorClient::new(llm_client.clone())),
        );

        // ... rest of the method ...
    }
}
```
</changes>
</step>
</steps>
</response>

This plan outlines the steps to create a new `StepGeneratorClient` tool, similar to the `ReasoningClient`. It includes creating the necessary structs, implementing the `Tool` trait, and updating the relevant enums and broker to include the new tool. You can follow these steps to implement the `StepGeneratorClient` in your project."#;
        let session_id = "".to_owned();
        let exchange_id = "".to_owned();
        let (sender, _receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut plan_step_generator = PlanStepGenerator::new(session_id, exchange_id, sender, None);
        plan_step_generator.add_delta(input.to_owned()).await;

        // we should have 7 steps over here
        let steps = plan_step_generator.generated_steps;
        assert_eq!(steps.len(), 7);
    }
}
