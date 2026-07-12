use modular_agent_core::{
    Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AgentValueMap,
    AsAgent, Message, ModularAgent, Usage, async_trait, modular_agent,
};

use crate::capabilities::lookup_capabilities;
use crate::provider::ModelIdentifier;

const CATEGORY: &str = "LLM";

const PORT_MESSAGE: &str = "message";
const PORT_RESET: &str = "reset";
const PORT_USAGE: &str = "usage";

const CONFIG_MODEL: &str = "model";

/// Accumulate token usage across assistant messages and report totals.
///
/// Connect the `message` output of a chat agent to `message`: each final
/// (non-streaming) message carrying [`Usage`] is added to running totals and
/// the totals object is emitted. Streaming partials and messages without
/// usage are silently skipped. When a message arrives with the same `id` as
/// the previous one (a re-emitted final replacing an earlier emission), the
/// previous usage is subtracted before the new one is added, so retried
/// turns are not double-counted. Totals are cleared when the agent stops, so
/// a preset restart does not double-count replayed history.
///
/// When the `model` config names a known model with cost rates, the totals
/// include `cost_usd`. Cache token rates missing from the registry fall back
/// to the input rate. For Claude models the built-in cache-write rate assumes
/// the 5-minute TTL (1.25x input); flows using ChatAgent
/// `cache_retention = "long"` (1-hour writes, billed at 2x) understate the
/// cache-write component unless `cache_write` is overridden via models.json.
///
/// # Ports
/// - Input `message`: Assistant message whose usage is accumulated
/// - Input `reset`: Any value clears the totals and emits the zeroed object
/// - Output `usage`: Totals object with `input_tokens`, `output_tokens`,
///   `cache_read_tokens`, `cache_write_tokens`, `total_tokens`, and
///   `cost_usd` (only when the model's cost rates are known)
///
/// # Configuration
/// - `model`: Provider-prefixed model name (e.g. `claude/claude-sonnet-4-6`)
///   used to look up cost rates; empty or unknown omits `cost_usd`
#[modular_agent(
    title = "Usage",
    category = CATEGORY,
    inputs = [PORT_MESSAGE, PORT_RESET],
    outputs = [PORT_USAGE],
    string_config(name = CONFIG_MODEL, default = ""),
    hint(width = 2, height = 1),
)]
pub struct UsageAgent {
    data: AgentData,
    totals: Usage,
    last_id: Option<String>,
    last_usage: Usage,
}

impl UsageAgent {
    fn clear(&mut self) {
        self.totals = Usage::default();
        self.last_id = None;
        self.last_usage = Usage::default();
    }

    /// Cost in USD for the current totals, or `None` when the model config
    /// is empty / unparseable or the registry has no cost rates for it.
    fn cost_usd(&self, model: &str) -> Option<f64> {
        if model.is_empty() {
            return None;
        }
        let id = ModelIdentifier::parse(model).ok()?;
        let rates = lookup_capabilities(&id).cost?;
        let t = &self.totals;
        Some(
            (t.input_tokens as f64 * rates.input
                + t.output_tokens as f64 * rates.output
                + t.cache_read_tokens as f64 * rates.cache_read.unwrap_or(rates.input)
                + t.cache_write_tokens as f64 * rates.cache_write.unwrap_or(rates.input))
                / 1_000_000.0,
        )
    }

    async fn emit_totals(&mut self, ctx: AgentContext) -> Result<(), AgentError> {
        let t = self.totals;
        let total_tokens = t
            .input_tokens
            .saturating_add(t.output_tokens)
            .saturating_add(t.cache_read_tokens)
            .saturating_add(t.cache_write_tokens);

        let mut map: AgentValueMap<String, AgentValue> = AgentValueMap::new();
        map.insert("input_tokens".into(), token_value(t.input_tokens));
        map.insert("output_tokens".into(), token_value(t.output_tokens));
        map.insert("cache_read_tokens".into(), token_value(t.cache_read_tokens));
        map.insert(
            "cache_write_tokens".into(),
            token_value(t.cache_write_tokens),
        );
        map.insert("total_tokens".into(), token_value(total_tokens));

        let model = self.configs()?.get_string_or_default(CONFIG_MODEL);
        if let Some(cost) = self.cost_usd(&model) {
            map.insert("cost_usd".into(), AgentValue::number(cost));
        }

        self.output(ctx, PORT_USAGE, AgentValue::object(map)).await
    }
}

fn token_value(v: u64) -> AgentValue {
    // Token counts can never realistically exceed i64::MAX; saturate anyway
    // to keep the conversion total.
    AgentValue::integer(i64::try_from(v).unwrap_or(i64::MAX))
}

fn add_usage(totals: &mut Usage, u: &Usage) {
    totals.input_tokens = totals.input_tokens.saturating_add(u.input_tokens);
    totals.output_tokens = totals.output_tokens.saturating_add(u.output_tokens);
    totals.cache_read_tokens = totals.cache_read_tokens.saturating_add(u.cache_read_tokens);
    totals.cache_write_tokens = totals
        .cache_write_tokens
        .saturating_add(u.cache_write_tokens);
}

fn subtract_usage(totals: &mut Usage, u: &Usage) {
    totals.input_tokens = totals.input_tokens.saturating_sub(u.input_tokens);
    totals.output_tokens = totals.output_tokens.saturating_sub(u.output_tokens);
    totals.cache_read_tokens = totals.cache_read_tokens.saturating_sub(u.cache_read_tokens);
    totals.cache_write_tokens = totals
        .cache_write_tokens
        .saturating_sub(u.cache_write_tokens);
}

#[async_trait]
impl AsAgent for UsageAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
            totals: Usage::default(),
            last_id: None,
            last_usage: Usage::default(),
        })
    }

    async fn stop(&mut self) -> Result<(), AgentError> {
        self.clear();
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        if port == PORT_RESET {
            self.clear();
            return self.emit_totals(ctx).await;
        }

        let message = Message::try_from(value)?;
        if message.streaming {
            return Ok(());
        }
        let Some(usage) = message.usage else {
            return Ok(());
        };

        // A final message re-emitted with the same id replaces the previous
        // one (e.g. an error-marked final after a retried stream): back out
        // the previously counted usage before adding the new one.
        if message.id.is_some() && message.id == self.last_id {
            subtract_usage(&mut self.totals, &self.last_usage);
        }
        add_usage(&mut self.totals, &usage);
        self.last_id = message.id;
        self.last_usage = usage;

        self.emit_totals(ctx).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use modular_agent_core::ConnectionSpec;
    use modular_agent_core::test_utils::{ProbeReceiver, TestProbeAgent, probe_receiver};

    /// Build a running preset with a UsageAgent whose `usage` port feeds a
    /// probe, so emitted totals can be observed end to end.
    async fn setup_usage_with_probe(model: &str) -> (ModularAgent, String, ProbeReceiver) {
        let ma = ModularAgent::init().unwrap();
        ma.ready().await.unwrap();

        let preset_id = ma.new_preset().unwrap();
        let usage_def = ma.get_agent_definition(UsageAgent::DEF_NAME).unwrap();
        let mut usage_spec = usage_def.to_spec();
        if let Some(configs) = usage_spec.configs.as_mut() {
            configs.set(CONFIG_MODEL.to_string(), AgentValue::string(model));
        }
        let usage_id = ma.add_agent(preset_id.clone(), usage_spec).await.unwrap();
        let probe_def = ma.get_agent_definition(TestProbeAgent::DEF_NAME).unwrap();
        let probe_id = ma
            .add_agent(preset_id.clone(), probe_def.to_spec())
            .await
            .unwrap();
        ma.add_connection(
            &preset_id,
            ConnectionSpec {
                source: usage_id.clone(),
                source_handle: PORT_USAGE.into(),
                target: probe_id.clone(),
                target_handle: "value".into(),
            },
        )
        .await
        .unwrap();
        ma.start_preset(&preset_id).await.unwrap();
        let probe_rx = probe_receiver(&ma, &probe_id).await.unwrap();

        (ma, usage_id, probe_rx)
    }

    async fn send(ma: &ModularAgent, usage_id: &str, port: &str, value: AgentValue) {
        let agent = ma.get_agent(usage_id).unwrap();
        let mut guard = agent.lock().await;
        let usage = guard.as_agent_mut::<UsageAgent>().unwrap();
        AsAgent::process(usage, AgentContext::new(), port.into(), value)
            .await
            .unwrap();
    }

    async fn recv_totals(probe_rx: &ProbeReceiver) -> AgentValueMap<String, AgentValue> {
        let (_ctx, value) = probe_rx.recv().await.unwrap();
        value.as_object().unwrap().clone()
    }

    fn tokens(map: &AgentValueMap<String, AgentValue>, key: &str) -> i64 {
        map.get(key).unwrap().as_i64().unwrap()
    }

    fn message_with_usage(id: &str, usage: Usage) -> AgentValue {
        let mut message = Message::assistant("hi".to_string());
        message.id = Some(id.to_string());
        message.usage = Some(usage);
        message.into()
    }

    fn usage(input: u64, output: u64, cache_read: u64, cache_write: u64) -> Usage {
        Usage {
            input_tokens: input,
            output_tokens: output,
            cache_read_tokens: cache_read,
            cache_write_tokens: cache_write,
        }
    }

    #[tokio::test]
    async fn accumulates_across_messages() {
        let (ma, usage_id, probe_rx) = setup_usage_with_probe("").await;

        send(
            &ma,
            &usage_id,
            PORT_MESSAGE,
            message_with_usage("m1", usage(10, 20, 30, 40)),
        )
        .await;
        let totals = recv_totals(&probe_rx).await;
        assert_eq!(tokens(&totals, "input_tokens"), 10);
        assert_eq!(tokens(&totals, "output_tokens"), 20);
        assert_eq!(tokens(&totals, "cache_read_tokens"), 30);
        assert_eq!(tokens(&totals, "cache_write_tokens"), 40);
        assert_eq!(tokens(&totals, "total_tokens"), 100);

        send(
            &ma,
            &usage_id,
            PORT_MESSAGE,
            message_with_usage("m2", usage(1, 2, 3, 4)),
        )
        .await;
        let totals = recv_totals(&probe_rx).await;
        assert_eq!(tokens(&totals, "input_tokens"), 11);
        assert_eq!(tokens(&totals, "output_tokens"), 22);
        assert_eq!(tokens(&totals, "cache_read_tokens"), 33);
        assert_eq!(tokens(&totals, "cache_write_tokens"), 44);
        assert_eq!(tokens(&totals, "total_tokens"), 110);

        ma.quit();
    }

    #[tokio::test]
    async fn skips_streaming_and_usage_less_messages() {
        let (ma, usage_id, probe_rx) = setup_usage_with_probe("").await;

        // Streaming partial with usage: skipped (no emit).
        let mut streaming = Message::assistant("par".to_string());
        streaming.id = Some("m1".to_string());
        streaming.streaming = true;
        streaming.usage = Some(usage(100, 100, 0, 0));
        send(&ma, &usage_id, PORT_MESSAGE, streaming.into()).await;

        // Final without usage: skipped (no emit).
        let mut no_usage = Message::assistant("done".to_string());
        no_usage.id = Some("m1".to_string());
        send(&ma, &usage_id, PORT_MESSAGE, no_usage.into()).await;

        // A counted message: the first (and only) emit reflects it alone.
        send(
            &ma,
            &usage_id,
            PORT_MESSAGE,
            message_with_usage("m2", usage(5, 6, 0, 0)),
        )
        .await;
        let totals = recv_totals(&probe_rx).await;
        assert_eq!(tokens(&totals, "input_tokens"), 5);
        assert_eq!(tokens(&totals, "output_tokens"), 6);
        assert_eq!(tokens(&totals, "total_tokens"), 11);

        ma.quit();
    }

    #[tokio::test]
    async fn same_id_replaces_previous_usage() {
        let (ma, usage_id, probe_rx) = setup_usage_with_probe("").await;

        send(
            &ma,
            &usage_id,
            PORT_MESSAGE,
            message_with_usage("m1", usage(10, 10, 10, 10)),
        )
        .await;
        let _ = recv_totals(&probe_rx).await;

        send(
            &ma,
            &usage_id,
            PORT_MESSAGE,
            message_with_usage("m2", usage(100, 100, 100, 100)),
        )
        .await;
        let _ = recv_totals(&probe_rx).await;

        // Re-emitted final for m2 replaces its previous contribution.
        send(
            &ma,
            &usage_id,
            PORT_MESSAGE,
            message_with_usage("m2", usage(150, 150, 150, 150)),
        )
        .await;
        let totals = recv_totals(&probe_rx).await;
        assert_eq!(tokens(&totals, "input_tokens"), 160);
        assert_eq!(tokens(&totals, "output_tokens"), 160);
        assert_eq!(tokens(&totals, "cache_read_tokens"), 160);
        assert_eq!(tokens(&totals, "cache_write_tokens"), 160);
        assert_eq!(tokens(&totals, "total_tokens"), 640);

        ma.quit();
    }

    #[tokio::test]
    async fn reset_zeroes_and_emits() {
        let (ma, usage_id, probe_rx) = setup_usage_with_probe("").await;

        send(
            &ma,
            &usage_id,
            PORT_MESSAGE,
            message_with_usage("m1", usage(10, 20, 0, 0)),
        )
        .await;
        let _ = recv_totals(&probe_rx).await;

        send(&ma, &usage_id, PORT_RESET, AgentValue::unit()).await;
        let totals = recv_totals(&probe_rx).await;
        assert_eq!(tokens(&totals, "input_tokens"), 0);
        assert_eq!(tokens(&totals, "output_tokens"), 0);
        assert_eq!(tokens(&totals, "cache_read_tokens"), 0);
        assert_eq!(tokens(&totals, "cache_write_tokens"), 0);
        assert_eq!(tokens(&totals, "total_tokens"), 0);

        // After reset the last_id dedup state is cleared: the same id counts
        // fresh, not as a replacement of pre-reset usage.
        send(
            &ma,
            &usage_id,
            PORT_MESSAGE,
            message_with_usage("m1", usage(3, 4, 0, 0)),
        )
        .await;
        let totals = recv_totals(&probe_rx).await;
        assert_eq!(tokens(&totals, "input_tokens"), 3);
        assert_eq!(tokens(&totals, "output_tokens"), 4);

        ma.quit();
    }

    #[cfg(feature = "claude")]
    #[tokio::test]
    async fn cost_usd_from_builtin_claude_rates() {
        let (ma, usage_id, probe_rx) = setup_usage_with_probe("claude/claude-sonnet-4-6").await;

        // claude-sonnet-4-6 rates: input 3.0, output 15.0, cache_read 0.3,
        // cache_write 3.75 (USD per Mtok). One Mtok of each sums the rates.
        send(
            &ma,
            &usage_id,
            PORT_MESSAGE,
            message_with_usage("m1", usage(1_000_000, 1_000_000, 1_000_000, 1_000_000)),
        )
        .await;
        let totals = recv_totals(&probe_rx).await;
        let cost = totals.get("cost_usd").unwrap().as_f64().unwrap();
        assert!((cost - 22.05).abs() < 1e-9, "cost_usd was {cost}");

        ma.quit();
    }

    #[tokio::test]
    async fn no_cost_usd_without_model_config() {
        let (ma, usage_id, probe_rx) = setup_usage_with_probe("").await;

        send(
            &ma,
            &usage_id,
            PORT_MESSAGE,
            message_with_usage("m1", usage(1_000_000, 1_000_000, 0, 0)),
        )
        .await;
        let totals = recv_totals(&probe_rx).await;
        assert!(!totals.contains_key("cost_usd"));

        ma.quit();
    }
}
