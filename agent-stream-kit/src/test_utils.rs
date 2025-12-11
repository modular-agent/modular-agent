use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::{
    sync::{Mutex as AsyncMutex, mpsc},
    time::timeout,
};

use crate::{ASKit, AgentContext, AgentData, AgentError, AgentSpec, AgentValue, AsAgent};
use askit_macros::askit_agent;

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
    inputs = ["*"],
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
        let spec = AgentSpec::from_def(&def);
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
        let spec = AgentSpec::from_def(&def);
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
        let spec = AgentSpec::from_def(&def);
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
