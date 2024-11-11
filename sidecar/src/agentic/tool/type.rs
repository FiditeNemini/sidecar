//! Contains the basic tool and how to extract data from it

use axum::async_trait;

use super::{errors::ToolError, input::ToolInput, output::ToolOutput};

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ToolType {
    // AskDocumentation,
    // AskUser,
    PlanningBeforeCodeEdit,
    CodeEditing,
    OpenFile,
    // Search,
    GoToDefinitions,
    GoToReferences,
    // FileSystem,
    // FolderOutline,
    // Terminal,
    LSPDiagnostics,
    ReRank,
    // WebScrape,
    // searches of different kind are over here
    FindCodeSnippets,
    RequestImportantSymbols,
    FindCodeSymbolsCodeBaseWide,
    UtilityCodeSymbolSearch,
    GrepInFile,
    GoToImplementations,
    // filtering queries go here
    FilterCodeSnippetsForEditing,
    FilterCodeSnippetsSingleSymbolForEditing,
    // editor requests
    EditorApplyEdits,
    // quick fix options
    GetQuickFix,
    // apply quick fix
    ApplyQuickFix,
    // Error correction tool selection
    CodeCorrectnessActionSelection,
    CodeEditingForError,
    // Followup decision
    ClassSymbolFollowup,
    // COT chains
    CodeEditingCOT,
    // Probe operation
    ProbeCreateQuestionForSymbol,
    ProbeEnoughOrDeeper,
    ProbeSubSymbolFiltering,
    ProbePossible,
    ProbeQuestion,
    ProbeSubSymbol,
    ProbeFollowAlongSymbol,
    ProbeSummarizeAnswer,
    ProbeTryHardAnswer,
    // Repo map Search
    RepoMapSearch,
    // Get important files by inferring from repo tree
    ImportantFilesFinder,
    // SWE Bench tool endpoint
    SWEBenchToolEndpoint,
    // Test correction
    TestCorrection,
    // Code symbols which we want to follow
    CodeSymbolsToFollowInitialRequest,
    // Tool to use to generate the final probe answer
    ProbeFinalAnswerSummary,
    // New sub symbol in class for code editing
    NewSubSymbolRequired,
    // Find symbol in the codebase using the vscode api
    GrepSymbolInCodebase,
    // Find new symbol file location
    FindFileForNewSymbol,
    // Find symbol to edit in user context
    FindSymbolsToEditInContext,
    // ReRanking code snippets for code editing context
    ReRankingCodeSnippetsForCodeEditingContext,
    // Apply the outline of the changes to the range we are interested in
    ApplyOutlineEditToRange,
    // Big search
    BigSearch,
    // Filter edit operation
    FilterEditOperation,
    // Keyword search
    KeywordSearch,
    // inlay hints for the code
    InLayHints,
    // code location for the new symbol
    CodeSymbolNewLocation,
    // should edit the code or is it just a check
    ShouldEditCode,
    // use search and replace blocks for edits
    SearchAndReplaceEditing,
    // Grabs the git-diff
    GitDiff,
    // code editing warmup tool
    CodeEditingWarmupTool,
    // grab outline nodes using the editor
    OutlineNodesUsingEditor,
    // filters references
    ReferencesFilter,
    // scratch pad agent
    ScratchPadAgent,
    // edited files
    EditedFiles,
    // Reasoning (This is just plain reasoning with no settings right now)
    Reasoning,
    // Plan updater
    PlanUpdater,
    // Step generator
    StepGenerator,
    // Create a new file
    CreateFile,
    // File diagnostics
    FileDiagnostics,
    // Add steps to the plan
    PlanStepAdd,
    // Go to previous word at a position
    GoToPreviousWordRange,
    // Go to type definition
    GoToTypeDefinition,
    // Context driven chat reply
    ContextDrivenChatReply,
    // Create a new exchange during a session
    NewExchangeDuringSession,
    // Undo changes made via exchange
    UndoChangesMadeDuringSession,
    // context driven hot streak reply which looks at LSP errors
    ContextDriveHotStreakReply,
    // Searches the files given a regex pattern
    SearchFileContentWithRegex,
    // List files
    ListFiles,
}

impl std::fmt::Display for ToolType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolType::CodeEditing => write!(f, "Code Editing"),
            ToolType::OpenFile => write!(f, "Open File"),
            ToolType::GoToDefinitions => write!(f, "Go To Definitions"),
            ToolType::GoToReferences => write!(f, "Go To References"),
            ToolType::LSPDiagnostics => write!(f, "LSP Diagnostics"),
            ToolType::ReRank => write!(f, "Re-Rank"),
            ToolType::FindCodeSnippets => write!(f, "Find Code Snippets"),
            ToolType::RequestImportantSymbols => write!(f, "Request Important Symbols"),
            ToolType::FindCodeSymbolsCodeBaseWide => write!(f, "Find Code Symbols Code Base Wide"),
            ToolType::UtilityCodeSymbolSearch => write!(f, "Utility Code Symbol Search"),
            ToolType::GrepInFile => write!(f, "Grep In File"),
            ToolType::GoToImplementations => write!(f, "Go To Implementations"),
            ToolType::FilterCodeSnippetsForEditing => write!(f, "Filter Code Snippets For Editing"),
            ToolType::FilterCodeSnippetsSingleSymbolForEditing => {
                write!(f, "Filter Code Snippets Single Symbol For Editing")
            }
            ToolType::EditorApplyEdits => write!(f, "Editor Apply Edits"),
            ToolType::GetQuickFix => write!(f, "Get Quick Fix"),
            ToolType::ApplyQuickFix => write!(f, "Apply Quick Fix"),
            ToolType::CodeCorrectnessActionSelection => {
                write!(f, "Code Correctness Action Selection")
            }
            ToolType::CodeEditingForError => write!(f, "Code Editing For Error"),
            ToolType::ClassSymbolFollowup => write!(f, "Class Symbol Followup"),
            ToolType::ProbePossible => write!(f, "Probe Possible"),
            ToolType::ProbeQuestion => write!(f, "Probe Question"),
            ToolType::ProbeSubSymbol => write!(f, "Probe Sub Symbol"),
            ToolType::ProbeFollowAlongSymbol => write!(f, "Probe Follow Along Symbol"),
            ToolType::ProbeSummarizeAnswer => write!(f, "Probe Summarize Answer"),
            ToolType::RepoMapSearch => write!(f, "Repo Map Search"),
            ToolType::SWEBenchToolEndpoint => write!(f, "SWE Bench Tool Endpoint"),
            ToolType::TestCorrection => write!(f, "Test Correction"),
            ToolType::CodeEditingCOT => write!(f, "Code editing COT"),
            ToolType::CodeSymbolsToFollowInitialRequest => {
                write!(f, "Code Symbols to follow initial request")
            }
            ToolType::ProbeFinalAnswerSummary => write!(f, "Probe final answer summary"),
            ToolType::ProbeSubSymbolFiltering => write!(f, "Probe sub symbol filtering request"),
            ToolType::ProbeEnoughOrDeeper => write!(f, "Probe enough information or go deeper"),
            ToolType::ProbeCreateQuestionForSymbol => write!(f, "Probe create question for symbol"),
            ToolType::PlanningBeforeCodeEdit => write!(f, "Planning before code edit"),
            ToolType::NewSubSymbolRequired => write!(f, "New sub symbol required for code editing"),
            ToolType::ProbeTryHardAnswer => write!(f, "Probe try hard answer"),
            ToolType::GrepSymbolInCodebase => write!(f, "Grep symbol in the codebase"),
            ToolType::FindFileForNewSymbol => write!(f, "Find file for new symbol"),
            ToolType::FindSymbolsToEditInContext => write!(f, "Find Symbols to edit in context"),
            ToolType::ReRankingCodeSnippetsForCodeEditingContext => {
                write!(f, "ReRanking code snippets for code editing")
            }
            ToolType::ApplyOutlineEditToRange => write!(f, "Apply outline edit to range"),
            ToolType::ImportantFilesFinder => write!(f, "Important files finder"),
            ToolType::BigSearch => write!(f, "Big search"),
            ToolType::FilterEditOperation => write!(f, "Filter edit operation"),
            ToolType::KeywordSearch => write!(f, "Keyword search"),
            ToolType::InLayHints => write!(f, "Inlay hints"),
            ToolType::CodeSymbolNewLocation => write!(f, "Code symbol new location"),
            ToolType::ShouldEditCode => write!(f, "Should edit code"),
            ToolType::SearchAndReplaceEditing => write!(f, "Search and replace editing"),
            ToolType::GitDiff => write!(
                f,
                "Gets the git diff output for a certain file, also returns the original version"
            ),
            ToolType::CodeEditingWarmupTool => write!(f, "Code editing warmup tool"),
            ToolType::OutlineNodesUsingEditor => write!(f, "Outline nodes using the editor"),
            ToolType::ReferencesFilter => write!(f, "Filters references"),
            ToolType::ScratchPadAgent => write!(f, "Scratch pad agent"),
            ToolType::EditedFiles => write!(f, "Edited files"),
            ToolType::Reasoning => write!(f, "Reasoning"),
            ToolType::PlanUpdater => write!(f, "Plan Updater"),
            ToolType::StepGenerator => write!(f, "Step generator"),
            ToolType::CreateFile => write!(f, "Create File"),
            ToolType::FileDiagnostics => write!(f, "File Diagnostics"),
            ToolType::PlanStepAdd => write!(f, "Plan step add"),
            ToolType::GoToPreviousWordRange => write!(f, "Go to previous word range"),
            ToolType::GoToTypeDefinition => write!(f, "Go to type definition"),
            ToolType::ContextDrivenChatReply => write!(f, "Context driven chat reply"),
            ToolType::NewExchangeDuringSession => write!(f, "New exchange during session"),
            ToolType::UndoChangesMadeDuringSession => write!(f, "Undo changes made during session"),
            ToolType::ContextDriveHotStreakReply => write!(
                f,
                "Context driven hot streak reply which looks at things out of scope"
            ),
            ToolType::SearchFileContentWithRegex => write!(f, "Searches for files with a regex"),
            ToolType::ListFiles => write!(f, "List files"),
        }
    }
}

#[async_trait]
pub trait Tool {
    async fn invoke(&self, input: ToolInput) -> Result<ToolOutput, ToolError>;

    /// Provides a verbose description for the tool and what it ends up doing
    fn tool_description(&self) -> String;

    /// Provides an XML format for the input expected by the tool
    fn tool_input_format(&self) -> String;
}
