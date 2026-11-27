use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;

use crate::domain::{
    ChatMessage, Citation, Conversation, ConversationSummary, DomainError, HistoryTurn, NewMessage,
};

#[async_trait]
pub trait ConversationStore: Send + Sync {
    async fn create(&self, user_id: &str, title: &str) -> anyhow::Result<Conversation>;
    async fn summaries(&self, user_id: &str) -> anyhow::Result<Vec<ConversationSummary>>;
    async fn find_owned(&self, user_id: &str, id: &str) -> anyhow::Result<Option<Conversation>>;
    async fn rename(
        &self,
        user_id: &str,
        id: &str,
        title: &str,
    ) -> anyhow::Result<Option<Conversation>>;
    async fn delete_owned(&self, user_id: &str, id: &str) -> anyhow::Result<bool>;
    /// Appends both messages in one write; returns the stored assistant message, id and time
    /// set here. `new_title` renames the conversation in that same write.
    async fn append_exchange(
        &self,
        user_id: &str,
        id: &str,
        user_message: NewMessage,
        assistant_message: NewMessage,
        new_title: Option<&str>,
    ) -> anyhow::Result<Option<ChatMessage>>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentEvent {
    Token(String),
    /// `answer`, when sent, replaces the joined tokens: agent strips unbacked citations and
    /// text streamed before a tool call. `None` from an older agent.
    Done {
        answer: Option<String>,
        citations: Vec<Citation>,
    },
}

pub type AnswerStream = Pin<Box<dyn Stream<Item = anyhow::Result<AgentEvent>> + Send>>;

#[async_trait]
pub trait AgentClient: Send + Sync {
    /// Resolves once agent has accepted the question; tokens then arrive on the stream.
    /// Unreachable or non-2xx is `BadGateway` here, not a stream error.
    async fn ask(
        &self,
        question: &str,
        history: &[HistoryTurn],
        claims_header: &str,
    ) -> Result<AnswerStream, DomainError>;
}
