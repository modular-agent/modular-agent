#![cfg(feature = "test-utils")]

use std::cell::RefCell;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::{
    sync::{Mutex as AsyncMutex, mpsc},
    time::timeout,
};

use crate::{
    AgentContext, AgentData, AgentError, AgentSpec, AgentValue, AsAgent, ModularAgent, ModularAgentEvent,
    modular_agent,
};

static PORT_VALUE: &str = "value";

/// Setting up ModularAgent
pub async fn setup_modular_agent() -> ModularAgent {
    let ma = ModularAgent::init().unwrap();
    ma.ready().await.unwrap();

    // set an observer to receive external output events
    subscribe_external_output_observer(&ma).unwrap();

    ma
}

/// Load and start an preset from a file.
pub async fn open_and_start_preset(ma: &ModularAgent, path: &str) -> Result<String, AgentError> {
    let id = ma.open_preset_from_file(path, None).await?;
    ma.start_preset(&id).await?;
    Ok(id)
}

// External Output Event Subscription

type ExternalOutputReceiver = Arc<AsyncMutex<mpsc::UnboundedReceiver<(String, AgentValue)>>>;

thread_local! {
    static EXTERNAL_OUTPUT_RX: RefCell<Option<ExternalOutputReceiver>> = RefCell::new(None);
}

pub fn subscribe_external_output_observer(ma: &ModularAgent) -> Result<(), AgentError> {
    let output_event_rx = ma.subscribe_to_event(|event| {
        if let ModularAgentEvent::ExternalOutput(name, value) = event {
            Some((name, value))
        } else {
            None
        }
    });

    EXTERNAL_OUTPUT_RX.with(|slot| {
        *slot.borrow_mut() = Some(Arc::new(AsyncMutex::new(output_event_rx)));
    });
    Ok(())
}

pub const DEFAULT_OUTPUT_TIMEOUT: Duration = Duration::from_secs(1);

fn external_output_rx() -> Result<ExternalOutputReceiver, AgentError> {
    EXTERNAL_OUTPUT_RX
        .with(|slot| slot.borrow().clone())
        .ok_or_else(|| AgentError::SendMessageFailed("external output receiver not initialized".into()))
}

pub async fn recv_external_output_with_timeout(
    duration: Duration,
) -> Result<(String, AgentValue), AgentError> {
    let rx = external_output_rx()?;
    let mut rx = rx.lock().await;
    timeout(duration, rx.recv())
        .await
        .map_err(|_| AgentError::SendMessageFailed("external output receive timed out".into()))?
        .ok_or_else(|| AgentError::SendMessageFailed("external output channel closed".into()))
}

pub async fn expect_external_output(
    expected_name: &str,
    expected_value: &AgentValue,
) -> Result<(), AgentError> {
    let (name, value) = recv_external_output_with_timeout(DEFAULT_OUTPUT_TIMEOUT).await?;
    if name == expected_name && &value == expected_value {
        Ok(())
    } else {
        Err(AgentError::SendMessageFailed(format!(
            "expected output '{}' with value {:?}, got '{}' with value {:?}",
            expected_name, expected_value, name, value
        )))
    }
}

pub async fn expect_local_value(
    flow_id: &str,
    var_name: &str,
    expected_value: &AgentValue,
) -> Result<(), AgentError> {
    let expected_name = format!("%{}/{}", flow_id, var_name);
    expect_external_output(&expected_name, expected_value).await
}

/// Write a value to a local input variable and expect the same value
/// to appear as an external output on the same variable name.
pub async fn write_and_expect_local_value(
    ma: &ModularAgent,
    preset_id: &str,
    var_name: &str,
    value: AgentValue,
) -> Result<(), AgentError> {
    ma.write_local_input(preset_id, var_name, value.clone())
        .await?;
    expect_local_value(preset_id, var_name, &value).await
}

// TestProbeAgent

pub type ProbeEvent = (AgentContext, AgentValue);

pub const DEFAULT_PROBE_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Clone)]
pub struct ProbeReceiver(Arc<AsyncMutex<mpsc::UnboundedReceiver<ProbeEvent>>>);

impl ProbeReceiver {
    pub async fn recv_with_timeout(&self, duration: Duration) -> Result<ProbeEvent, AgentError> {
        let mut rx = self.0.lock().await;
        timeout(duration, rx.recv())
            .await
            .map_err(|_| AgentError::SendMessageFailed("probe receive timed out".into()))?
            .ok_or_else(|| AgentError::SendMessageFailed("probe channel closed".into()))
    }

    pub async fn recv(&self) -> Result<ProbeEvent, AgentError> {
        self.recv_with_timeout(DEFAULT_PROBE_TIMEOUT).await
    }
}

#[modular_agent(
    title = "TestProbeAgent",
    category = "Test",
    inputs = [PORT_VALUE],
    outputs = []
)]
pub struct TestProbeAgent {
    data: AgentData,
    tx: mpsc::UnboundedSender<ProbeEvent>,
    rx: ProbeReceiver,
}

impl TestProbeAgent {
    /// Receive next probe event using the instance's own receiver.
    pub async fn recv_with_timeout(&self, duration: Duration) -> Result<ProbeEvent, AgentError> {
        self.rx.recv_with_timeout(duration).await
    }

    /// Clone the internal receiver so callers can drop agent locks before awaiting.
    pub fn probe_receiver(&self) -> ProbeReceiver {
        self.rx.clone()
    }
}

/// Helper to fetch the probe receiver for a TestProbeAgent by id.
pub async fn probe_receiver(ma: &ModularAgent, agent_id: &str) -> Result<ProbeReceiver, AgentError> {
    let probe = ma
        .get_agent(agent_id)
        .ok_or_else(|| AgentError::AgentNotFound(agent_id.to_string()))?;
    let probe_guard = probe.lock().await;
    let probe_agent = probe_guard
        .as_agent::<TestProbeAgent>()
        .ok_or_else(|| AgentError::AgentNotFound(agent_id.to_string()))?;
    Ok(probe_agent.probe_receiver())
}

/// Await one probe event with timeout on the given receiver.
pub async fn recv_probe_with_timeout(
    probe_rec: &ProbeReceiver,
    duration: Duration,
) -> Result<ProbeEvent, AgentError> {
    probe_rec.recv_with_timeout(duration).await
}

/// Receive one probe event with the default timeout.
pub async fn recv_probe(probe_rec: &ProbeReceiver) -> Result<ProbeEvent, AgentError> {
    probe_rec.recv().await
}

#[async_trait]
impl AsAgent for TestProbeAgent {
    fn new(ma: crate::ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let (tx, rx) = mpsc::unbounded_channel();
        let rx = ProbeReceiver(Arc::new(AsyncMutex::new(rx)));

        Ok(Self {
            data: AgentData::new(ma, id, spec),
            tx,
            rx,
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        // Ignore send failures in tests; probe won't fail the pipeline
        let _ = self.tx.send((ctx, value));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use modular_agent_core::test_utils::TestProbeAgent;
    use modular_agent_core::{AgentContext, AgentError, AgentValue, ModularAgent};
    use tokio::time::Duration;

    #[tokio::test]
    async fn probe_receives_in_order() {
        let ma = ModularAgent::new();
        let def = TestProbeAgent::agent_definition();
        let spec = def.to_spec();
        let mut probe = TestProbeAgent::new(ma, "p1".into(), spec).unwrap();

        probe
            .process(AgentContext::new(), "in".into(), AgentValue::integer(1))
            .await
            .unwrap();
        let (_ctx, v1) = probe.probe_receiver().recv().await.unwrap();
        assert_eq!(v1, AgentValue::integer(1));

        probe
            .process(AgentContext::new(), "in".into(), AgentValue::integer(2))
            .await
            .unwrap();
        let (_ctx, v2) = probe.probe_receiver().recv().await.unwrap();
        assert_eq!(v2, AgentValue::integer(2));
    }

    #[tokio::test]
    async fn probe_times_out() {
        let ma = ModularAgent::new();
        let def = TestProbeAgent::agent_definition();
        let spec = def.to_spec();
        let probe = TestProbeAgent::new(ma, "p1".into(), spec).unwrap();
        let err = probe
            .recv_with_timeout(Duration::from_millis(10))
            .await
            .unwrap_err();
        assert!(matches!(err, AgentError::SendMessageFailed(_)));
    }

    #[tokio::test]
    async fn probe_receiver_clone_works() {
        let ma = ModularAgent::new();
        let def = TestProbeAgent::agent_definition();
        let spec = def.to_spec();
        let mut probe = TestProbeAgent::new(ma, "p1".into(), spec).unwrap();
        let rx1 = probe.probe_receiver();
        let rx2 = probe.probe_receiver();

        probe
            .process(AgentContext::new(), "in".into(), AgentValue::integer(42))
            .await
            .unwrap();

        // Either receiver can consume the message (both clone the same inner receiver)
        let (_ctx, v) = rx1.recv().await.unwrap();
        assert_eq!(v, AgentValue::integer(42));

        // Ensure timeout when no further messages exist
        let err = rx2
            .recv_with_timeout(Duration::from_millis(10))
            .await
            .unwrap_err();
        assert!(matches!(err, AgentError::SendMessageFailed(_)));
    }
}
