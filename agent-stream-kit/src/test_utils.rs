#![cfg(feature = "test-utils")]

use std::cell::RefCell;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::{
    sync::{Mutex as AsyncMutex, mpsc},
    time::timeout,
};

use crate::{
    ASKit, ASKitEvent, AgentContext, AgentData, AgentError, AgentSpec, AgentStreamSpec, AgentValue,
    AsAgent, askit_agent,
};

static PIN_VALUE: &str = "value";

/// Setting up ASKit
pub async fn setup_askit() -> ASKit {
    let askit = ASKit::init().unwrap();
    askit.ready().await.unwrap();

    // set an observer to receive board events
    subscribe_board_observer(&askit).unwrap();

    askit
}

/// Load and start an agent stream from a file.
pub async fn load_and_start_stream(askit: &ASKit, path: &str) -> Result<String, AgentError> {
    let stream_json = std::fs::read_to_string(path)
        .map_err(|e| AgentError::IoError(format!("Failed to read stream file: {}", e)))?;
    let spec = AgentStreamSpec::from_json(&stream_json)?;
    let name = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("stream")
        .to_string();
    let id = askit.add_agent_stream(name, spec)?;
    askit.start_agent_stream(&id).await?;
    Ok(id)
}

// Board Event Subscription

type BoardReceiver = Arc<AsyncMutex<mpsc::UnboundedReceiver<(String, AgentValue)>>>;

thread_local! {
    static BOARD_RX: RefCell<Option<BoardReceiver>> = RefCell::new(None);
}

pub fn subscribe_board_observer(askit: &ASKit) -> Result<(), AgentError> {
    let board_event_rx = askit.subscribe_to_event(|event| {
        if let ASKitEvent::Board(name, value) = event {
            Some((name, value))
        } else {
            None
        }
    });

    BOARD_RX.with(|slot| {
        *slot.borrow_mut() = Some(Arc::new(AsyncMutex::new(board_event_rx)));
    });
    Ok(())
}

pub const DEFAULT_BOARD_TIMEOUT: Duration = Duration::from_secs(1);

fn board_rx() -> Result<BoardReceiver, AgentError> {
    BOARD_RX
        .with(|slot| slot.borrow().clone())
        .ok_or_else(|| AgentError::SendMessageFailed("board receiver not initialized".into()))
}

pub async fn recv_board_with_timeout(
    duration: Duration,
) -> Result<(String, AgentValue), AgentError> {
    let rx = board_rx()?;
    let mut rx = rx.lock().await;
    timeout(duration, rx.recv())
        .await
        .map_err(|_| AgentError::SendMessageFailed("board receive timed out".into()))?
        .ok_or_else(|| AgentError::SendMessageFailed("board channel closed".into()))
}

pub async fn expect_board_value(
    expected_name: &str,
    expected_value: &AgentValue,
) -> Result<(), AgentError> {
    let (name, value) = recv_board_with_timeout(DEFAULT_BOARD_TIMEOUT).await?;
    if name == expected_name && &value == expected_value {
        Ok(())
    } else {
        Err(AgentError::SendMessageFailed(format!(
            "expected board '{}' with value {:?}, got '{}' with value {:?}",
            expected_name, expected_value, name, value
        )))
    }
}

pub async fn expect_var_value(
    flow_id: &str,
    var_name: &str,
    expected_value: &AgentValue,
) -> Result<(), AgentError> {
    let expected_name = format!("%{}/{}", flow_id, var_name);
    expect_board_value(&expected_name, expected_value).await
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

#[askit_agent(
    title = "TestProbeAgent",
    category = "Test",
    inputs = [PIN_VALUE],
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
pub async fn probe_receiver(askit: &ASKit, agent_id: &str) -> Result<ProbeReceiver, AgentError> {
    let probe = askit
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
    fn new(askit: crate::ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let (tx, rx) = mpsc::unbounded_channel();
        let rx = ProbeReceiver(Arc::new(AsyncMutex::new(rx)));

        Ok(Self {
            data: AgentData::new(askit, id, spec),
            tx,
            rx,
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
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

    use agent_stream_kit::test_utils::TestProbeAgent;
    use agent_stream_kit::{ASKit, AgentContext, AgentError, AgentValue};
    use tokio::time::Duration;

    #[tokio::test]
    async fn probe_receives_in_order() {
        let askit = ASKit::new();
        let def = TestProbeAgent::agent_definition();
        let spec = def.to_spec();
        let mut probe = TestProbeAgent::new(askit, "p1".into(), spec).unwrap();

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
        let askit = ASKit::new();
        let def = TestProbeAgent::agent_definition();
        let spec = def.to_spec();
        let probe = TestProbeAgent::new(askit, "p1".into(), spec).unwrap();
        let err = probe
            .recv_with_timeout(Duration::from_millis(10))
            .await
            .unwrap_err();
        assert!(matches!(err, AgentError::SendMessageFailed(_)));
    }

    #[tokio::test]
    async fn probe_receiver_clone_works() {
        let askit = ASKit::new();
        let def = TestProbeAgent::agent_definition();
        let spec = def.to_spec();
        let mut probe = TestProbeAgent::new(askit, "p1".into(), spec).unwrap();
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
