use std::sync::atomic::{AtomicUsize, Ordering};

use serde::{Deserialize, Serialize};

use crate::error::AgentError;
use crate::value::AgentValue;

/// Event-scoped context that identifies a single flow across agents and carries auxiliary metadata.
///
/// A context is created per externally triggered event (user input, timer, webhook, etc.) so that
/// agents connected through channels can recognize they are handling the same flow. It can carry
/// auxiliary metadata useful for processing without altering the primary payload.
///
/// When a single datum fans out into multiple derived items (e.g., a `map` operation), frames track
/// the branching lineage. Because mapping can nest, frames behave like a stack to preserve ancestry.
/// Instances are cheap to clone and return new copies instead of mutating in place.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AgentContext {
    /// Unique identifier assigned when the context is created.
    id: usize,

    #[serde(skip_serializing_if = "Option::is_none")]
    vars: Option<im::HashMap<String, AgentValue>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    frames: Option<im::Vector<Frame>>,
}

pub const FRAME_MAP: &str = "map";
pub const FRAME_KEY_INDEX: &str = "index";
pub const FRAME_KEY_LENGTH: &str = "length";

impl AgentContext {
    /// Creates a new context with a unique identifier and no state.
    pub fn new() -> Self {
        Self {
            id: new_id(),
            vars: None,
            frames: None,
        }
    }

    /// Returns the unique identifier for this context.
    pub fn id(&self) -> usize {
        self.id
    }

    // Variables

    /// Retrieves an immutable reference to a stored variable, if present.
    pub fn get_var(&self, key: &str) -> Option<&AgentValue> {
        self.vars.as_ref().and_then(|vars| vars.get(key))
    }

    /// Returns a new context with the provided variable inserted while keeping the current context unchanged.
    pub fn with_var(&self, key: String, value: AgentValue) -> Self {
        let mut vars = if let Some(vars) = &self.vars {
            vars.clone()
        } else {
            im::HashMap::new()
        };
        vars.insert(key, value);
        Self {
            id: self.id,
            vars: Some(vars),
            frames: self.frames.clone(),
        }
    }
}

// ID generation
static CONTEXT_ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

/// Generates a monotonically increasing identifier for contexts.
fn new_id() -> usize {
    CONTEXT_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

// Frame stack

/// Describes a single stack frame captured during agent execution.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Frame {
    pub name: String,
    pub data: AgentValue,
}

fn map_frame_data(index: usize, len: usize) -> AgentValue {
    let mut data = AgentValue::object_default();
    let _ = data.set(
        FRAME_KEY_INDEX.to_string(),
        AgentValue::integer(index as i64),
    );
    let _ = data.set(
        FRAME_KEY_LENGTH.to_string(),
        AgentValue::integer(len as i64),
    );
    data
}

fn read_map_frame(frame: &Frame) -> Result<(usize, usize), AgentError> {
    let idx = frame
        .data
        .get(FRAME_KEY_INDEX)
        .and_then(|v| v.as_i64())
        .ok_or_else(|| AgentError::InvalidValue("map frame missing integer index".into()))?;
    let len = frame
        .data
        .get(FRAME_KEY_LENGTH)
        .and_then(|v| v.as_i64())
        .ok_or_else(|| AgentError::InvalidValue("map frame missing integer length".into()))?;
    if idx < 0 || len < 1 {
        return Err(AgentError::InvalidValue("Invalid map frame values".into()));
    }
    let (idx, len) = (idx as usize, len as usize);
    if idx >= len {
        return Err(AgentError::InvalidValue(
            "map frame index is out of bounds".into(),
        ));
    }
    Ok((idx, len))
}

impl AgentContext {
    /// Returns the current frame stack, if any frames have been pushed.
    pub fn frames(&self) -> Option<&im::Vector<Frame>> {
        self.frames.as_ref()
    }

    /// Appends a new frame to the end of the stack and returns the updated context.
    pub fn push_frame(&self, name: String, data: AgentValue) -> Self {
        let mut frames = if let Some(frames) = &self.frames {
            frames.clone()
        } else {
            im::Vector::new()
        };
        frames.push_back(Frame { name, data });
        Self {
            id: self.id,
            vars: self.vars.clone(),
            frames: Some(frames),
        }
    }

    /// Pushes a map frame with index/length metadata after validating bounds.
    pub fn push_map_frame(&self, index: usize, len: usize) -> Result<Self, AgentError> {
        if len == 0 {
            return Err(AgentError::InvalidValue(
                "map frame length must be positive".into(),
            ));
        }
        if index >= len {
            return Err(AgentError::InvalidValue(
                "map frame index is out of bounds".into(),
            ));
        }
        Ok(self.push_frame(FRAME_MAP.to_string(), map_frame_data(index, len)))
    }

    /// Returns the most recent map frame's (index, length) if present at the top of the stack.
    pub fn current_map_frame(&self) -> Result<Option<(usize, usize)>, AgentError> {
        let frames = match self.frames() {
            Some(frames) => frames,
            None => return Ok(None),
        };
        let Some(last_index) = frames.len().checked_sub(1) else {
            return Ok(None);
        };
        let Some(frame) = frames.get(last_index) else {
            return Ok(None);
        };
        if frame.name != FRAME_MAP {
            return Ok(None);
        }
        read_map_frame(frame).map(Some)
    }

    /// Removes the most recent map frame, erroring if the top frame is missing or not a map frame.
    pub fn pop_map_frame(&self) -> Result<AgentContext, AgentError> {
        let (frame, next_ctx) = self.pop_frame();
        match frame {
            Some(f) if f.name == FRAME_MAP => Ok(next_ctx),
            Some(f) => Err(AgentError::InvalidValue(format!(
                "Unexpected frame '{}', expected map",
                f.name
            ))),
            None => Err(AgentError::InvalidValue(
                "Missing map frame in context".into(),
            )),
        }
    }

    /// Collects all map frame (index, length) tuples in order, validating each entry.
    pub fn map_frame_indices(&self) -> Result<Vec<(usize, usize)>, AgentError> {
        let mut indices = Vec::new();
        let Some(frames) = self.frames() else {
            return Ok(indices);
        };
        for frame in frames.iter() {
            if frame.name != FRAME_MAP {
                continue;
            }
            let (idx, len) = read_map_frame(frame)?;
            indices.push((idx, len));
        }
        Ok(indices)
    }

    /// Returns a stable key combining the context id with all map frame indices, if present.
    pub fn ctx_key(&self) -> Result<String, AgentError> {
        let map_frames = self.map_frame_indices()?;
        if map_frames.is_empty() {
            return Ok(self.id().to_string());
        }
        let parts: Vec<String> = map_frames
            .iter()
            .map(|(idx, len)| format!("{}:{}", idx, len))
            .collect();
        Ok(format!("{}:{}", self.id(), parts.join(",")))
    }

    /// Removes the most recently pushed frame and returns it together with the updated context.
    /// If the stack is empty, `None` is returned alongside an unchanged context.
    pub fn pop_frame(&self) -> (Option<Frame>, Self) {
        if let Some(frames) = &self.frames {
            if frames.is_empty() {
                return (None, self.clone());
            }
            let mut frames = frames.clone();
            let last = frames.pop_back().unwrap(); // safe unwrap after is_empty check

            let new_frames = if frames.is_empty() {
                None
            } else {
                Some(frames)
            };
            return (
                Some(last),
                Self {
                    id: self.id,
                    vars: self.vars.clone(),
                    frames: new_frames,
                },
            );
        }
        (None, self.clone())
    }
}

// Tests
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn new_assigns_unique_ids() {
        let ctx1 = AgentContext::new();
        let ctx2 = AgentContext::new();

        assert_ne!(ctx1.id(), 0);
        assert_ne!(ctx2.id(), 0);
        assert_ne!(ctx1.id(), ctx2.id());
        assert_eq!(ctx1.id(), ctx1.clone().id());
    }

    #[test]
    fn with_var_sets_value_without_mutating_original() {
        let ctx = AgentContext::new();
        assert!(ctx.get_var("answer").is_none());

        let updated = ctx.with_var("answer".into(), AgentValue::integer(42));

        assert!(ctx.get_var("answer").is_none());
        assert_eq!(updated.get_var("answer"), Some(&AgentValue::integer(42)));
        assert_eq!(ctx.id(), updated.id());
    }

    #[test]
    fn push_and_pop_frames() {
        let ctx = AgentContext::new();
        assert!(ctx.frames().is_none());

        let ctx = ctx
            .push_frame("first".into(), AgentValue::string("a"))
            .push_frame("second".into(), AgentValue::integer(2));

        let frames = ctx.frames().expect("frames should be present");
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].name, "first");
        assert_eq!(frames[1].name, "second");
        assert_eq!(frames[1].data, AgentValue::integer(2));

        let (popped_second, ctx) = ctx.pop_frame();
        let popped_second = popped_second.expect("second frame should exist");
        assert_eq!(popped_second.name, "second");
        assert_eq!(ctx.frames().unwrap().len(), 1);
        assert_eq!(ctx.frames().unwrap()[0].name, "first");

        let (popped_first, ctx) = ctx.pop_frame();
        assert_eq!(popped_first.unwrap().name, "first");
        assert!(ctx.frames().is_none());

        let (no_frame, ctx_after_empty) = ctx.pop_frame();
        assert!(no_frame.is_none());
        assert!(ctx_after_empty.frames().is_none());
    }

    #[test]
    fn clone_preserves_vars() {
        let ctx = AgentContext::new().with_var("key".into(), AgentValue::integer(1));
        let cloned = ctx.clone();

        assert_eq!(cloned.get_var("key"), Some(&AgentValue::integer(1)));
        assert_eq!(cloned.id(), ctx.id());
    }

    #[test]
    fn clone_preserves_frames() {
        let ctx = AgentContext::new().push_frame("frame".into(), AgentValue::string("data"));
        let cloned = ctx.clone();

        let frames = cloned.frames().expect("cloned frames should exist");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].name, "frame");
        assert_eq!(frames[0].data, AgentValue::string("data"));
        assert_eq!(cloned.id(), ctx.id());
    }

    #[test]
    fn serialization_skips_empty_optional_fields() {
        let ctx = AgentContext::new();
        let json_ctx = serde_json::to_value(&ctx).unwrap();

        assert!(json_ctx.get("id").and_then(|v| v.as_u64()).is_some());
        assert!(json_ctx.get("vars").is_none());
        assert!(json_ctx.get("frames").is_none());

        let populated = ctx
            .with_var("key".into(), AgentValue::string("value"))
            .push_frame("frame".into(), AgentValue::integer(1));
        let json_populated = serde_json::to_value(&populated).unwrap();

        assert_eq!(json_populated["vars"]["key"], json!("value"));
        let frames = json_populated["frames"]
            .as_array()
            .expect("frames should serialize as array");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0]["name"], json!("frame"));
        assert_eq!(frames[0]["data"], json!(1));
    }

    #[test]
    fn map_frame_helpers_validate_and_track_indices() -> Result<(), AgentError> {
        let ctx = AgentContext::new();
        let ctx = ctx.push_map_frame(0, 2)?;
        let ctx = ctx.push_map_frame(1, 3)?;

        let indices = ctx.map_frame_indices()?;
        assert_eq!(indices, vec![(0, 2), (1, 3)]);

        let current = ctx.current_map_frame()?.expect("map frame should exist");
        assert_eq!(current, (1, 3));

        let key = ctx.ctx_key()?;
        assert_eq!(key, format!("{}:0:2,1:3", ctx.id()));

        let ctx = ctx.pop_map_frame()?;
        let current_after_pop = ctx.current_map_frame()?.expect("map frame should remain");
        assert_eq!(current_after_pop, (0, 2));

        Ok(())
    }

    #[test]
    fn pop_map_frame_errors_when_missing_or_wrong_kind() {
        let ctx = AgentContext::new();
        assert!(ctx.pop_map_frame().is_err());

        let ctx = ctx.push_frame("other".into(), AgentValue::unit());
        assert!(ctx.pop_map_frame().is_err());
    }

    #[test]
    fn push_map_frame_rejects_invalid_bounds() {
        let ctx = AgentContext::new();
        assert!(ctx.push_map_frame(0, 0).is_err());
        assert!(ctx.push_map_frame(2, 1).is_err());
    }
}
