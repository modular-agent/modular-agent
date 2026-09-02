//! Conversation-summarization machinery behind the `Messages` agents'
//! rolling summary of evicted history: the prompt builder, the transcript
//! renderer, and a provider-routed single-request summarizer.

use modular_agent_core::{AgentContext, AgentError, Message, ModularAgent};

use crate::provider::ModelIdentifier;
use crate::retry::RetryPolicy;

#[cfg(any(feature = "openai", feature = "claude", feature = "ollama"))]
use crate::chat::request_or_cancelled;

#[cfg(feature = "claude")]
use crate::claude_client;
#[cfg(feature = "ollama")]
use crate::ollama_client;
#[cfg(feature = "openai")]
use crate::openai_client;

// Summarization is a single request without retries (the Messages agents
// process inputs serially, so retrying would stall the conversation); the
// retry machinery still enforces the per-attempt timeout.
pub(crate) const SUMMARY_RETRY_BASE_DELAY_MS: i64 = 1000;
pub(crate) const SUMMARY_TIMEOUT_SECS: i64 = 300;

/// The provider client caches an LLM-calling agent holds. Each manager
/// resolves credentials from the `Chat` agent's global configs (falling back
/// to environment variables), so any agent embedding this bundle gets
/// provider access without registering global configs of its own.
pub(crate) struct ProviderManagers {
    #[cfg(feature = "claude")]
    claude: claude_client::ClaudeManager,
    #[cfg(feature = "openai")]
    openai: openai_client::OpenAIManager,
    #[cfg(feature = "ollama")]
    ollama: ollama_client::OllamaManager,
}

impl ProviderManagers {
    pub(crate) fn new() -> Self {
        Self {
            #[cfg(feature = "claude")]
            claude: claude_client::ClaudeManager::new(),
            #[cfg(feature = "openai")]
            openai: openai_client::OpenAIManager::new(),
            #[cfg(feature = "ollama")]
            ollama: ollama_client::OllamaManager::new(),
        }
    }

    /// One non-streaming summarization request against the given model: no
    /// tools, no thinking, provider defaults for sampling. Runs under the
    /// retry policy and the flow's cancellation token. `max_output_tokens`
    /// caps the response length when set; `None` keeps each provider's
    /// default behavior.
    #[cfg_attr(
        not(any(feature = "openai", feature = "claude", feature = "ollama")),
        allow(unused_variables)
    )]
    pub(crate) async fn summarize(
        &self,
        ma: &ModularAgent,
        ctx: &AgentContext,
        model_id: &ModelIdentifier,
        prompt: String,
        retry: RetryPolicy,
        max_output_tokens: Option<u32>,
    ) -> Result<String, AgentError> {
        // Annotated: with no provider feature enabled only the diverging
        // fallback arm remains, and the binding's type cannot be inferred.
        let message: Message = match model_id.provider {
            #[cfg(feature = "openai")]
            crate::provider::ProviderKind::OpenAI => {
                let client = self.openai.get_client(ma)?;
                let mut request = serde_json::json!({
                    "model": model_id.model_name,
                    "messages": [openai_client::message_to_chat_json(&Message::user(prompt))],
                    "stream": false,
                });
                if let Some(n) = max_output_tokens {
                    request["max_tokens"] = serde_json::json!(n);
                }
                let url = client.chat_completions_url();
                let res: openai_client::ChatCompletionResponse = request_or_cancelled(
                    ctx.cancel_token(),
                    retry.run(|| client.post_json(&url, &request)),
                )
                .await?;
                let choice = res.choices.first().ok_or_else(|| {
                    AgentError::IoError("Summarization response has no choices".to_string())
                })?;
                openai_client::message_from_chat_response(&choice.message)
            }
            #[cfg(feature = "claude")]
            crate::provider::ProviderKind::Claude => {
                let client = self.claude.get_client(ma)?;
                // Same non-streaming default as ChatAgent: cap at 8192 so a
                // runaway generation cannot outlive the per-attempt timeout.
                let default_cap = crate::capabilities::resolve_entry(model_id)
                    .max_tokens
                    .unwrap_or(crate::capabilities::DEFAULT_MAX_TOKENS)
                    .min(crate::capabilities::DEFAULT_MAX_TOKENS);
                let max_tokens = match max_output_tokens {
                    Some(n) => n.min(default_cap),
                    None => default_cap,
                };
                let prompt_messages =
                    im::vector![modular_agent_core::AgentValue::from(Message::user(prompt))];
                let (system, claude_messages) = claude_client::messages_to_claude(&prompt_messages);
                let request = claude_client::ClaudeRequest {
                    model: model_id.model_name.clone(),
                    max_tokens,
                    messages: claude_messages,
                    system: system.map(claude_client::ClaudeContent::Text),
                    stream: None,
                    tools: None,
                    thinking: None,
                    output_config: None,
                    temperature: None,
                    top_p: None,
                };
                let response = request_or_cancelled(
                    ctx.cancel_token(),
                    retry.run(|| client.create_message(&request)),
                )
                .await?;
                claude_client::message_from_claude_response(&response)
            }
            #[cfg(feature = "ollama")]
            crate::provider::ProviderKind::Ollama => {
                let client = self.ollama.get_client(ma)?;
                let mut request = serde_json::json!({
                    "model": model_id.model_name,
                    "messages": [
                        serde_json::to_value(ollama_client::message_to_ollama(&Message::user(
                            prompt,
                        )))
                        .unwrap_or(serde_json::json!({}))
                    ],
                    "stream": false,
                });
                if let Some(n) = max_output_tokens {
                    request["options"] = serde_json::json!({ "num_predict": n });
                }
                let url = client.chat_url();
                let res: ollama_client::ChatResponse = request_or_cancelled(
                    ctx.cancel_token(),
                    retry.run(|| client.post_json(&url, &request)),
                )
                .await?;
                ollama_client::message_from_ollama(&res.message)
            }
            #[allow(unreachable_patterns)]
            _ => {
                return Err(AgentError::InvalidConfig(format!(
                    "Provider {:?} not enabled. Enable the corresponding feature.",
                    model_id.provider
                )));
            }
        };

        let summary = message.text().trim().to_string();
        if summary.is_empty() {
            return Err(AgentError::IoError(
                "Summarization returned an empty response".to_string(),
            ));
        }
        Ok(summary)
    }
}

/// The structured summarization prompt. With a previous summary the prompt
/// switches to UPDATE mode, merging it with the newly dropped messages; the
/// caller's `instructions` are appended verbatim when non-empty.
pub(crate) fn build_summary_prompt(
    previous_summary: Option<&str>,
    dropped: &[Message],
    instructions: &str,
) -> String {
    let mut prompt = String::new();
    if let Some(previous) = previous_summary {
        prompt.push_str(
            "You are updating the running summary of an ongoing conversation.\n\n\
             Current summary:\n",
        );
        prompt.push_str(previous);
        prompt.push_str("\n\nMerge the following new conversation messages into the summary:\n\n");
    } else {
        prompt.push_str("Summarize the following conversation:\n\n");
    }
    prompt.push_str(&render_transcript(dropped));
    prompt.push_str(
        "\nWrite a concise summary that captures:\n\
         - the user's goals and intent\n\
         - key decisions made and why\n\
         - constraints and requirements that must be respected\n\
         - important facts, names, and data mentioned\n\
         - unresolved questions and pending tasks\n\
         Respond with the summary text only.",
    );
    if !instructions.is_empty() {
        prompt.push_str("\n\nAdditional instructions:\n");
        prompt.push_str(instructions);
    }
    prompt
}

/// Renders dropped messages as a plain-text transcript for the
/// summarization prompt. Tool calls are listed by name and arguments so
/// decisions carried through tools survive into the summary; image blocks
/// leave an `[image: <mime_type>]` placeholder so the summary keeps a
/// record that a tool returned an image.
pub(crate) fn render_transcript(messages: &[Message]) -> String {
    let mut out = String::new();
    for m in messages {
        out.push('[');
        out.push_str(&m.role);
        out.push_str("]\n");
        // Not text(): that drops image blocks without a trace.
        let text = crate::content::tool_result_fallback_text(&m.content);
        if !text.is_empty() {
            out.push_str(&text);
            out.push('\n');
        }
        if let Some(tool_calls) = &m.tool_calls {
            for call in tool_calls {
                let args = serde_json::to_string(&call.function.parameters).unwrap_or_default();
                out.push_str(&format!("(tool call: {} {})\n", call.function.name, args));
            }
        }
        out.push('\n');
    }
    out
}
