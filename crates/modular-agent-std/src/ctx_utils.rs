use std::collections::VecDeque;

use agent_stream_kit::AgentValue;

/// Find the first key that appears in every queue and return its positions.
pub(crate) fn find_first_common_key(
    queues: &Vec<VecDeque<(String, AgentValue)>>,
) -> Option<(String, Vec<usize>)> {
    let (base_idx, base_queue) = queues
        .iter()
        .enumerate()
        .filter(|(_, q)| !q.is_empty())
        .min_by_key(|(_, q)| q.len())?;

    for (pos, (key, _)) in base_queue.iter().enumerate() {
        let mut positions = vec![usize::MAX; queues.len()];
        positions[base_idx] = pos;
        let mut found_in_all = true;
        for (idx, queue) in queues.iter().enumerate() {
            if idx == base_idx {
                continue;
            }
            if let Some(p) = queue.iter().position(|(k, _)| k == key) {
                positions[idx] = p;
            } else {
                found_in_all = false;
                break;
            }
        }
        if found_in_all {
            return Some((key.clone(), positions));
        }
    }
    None
}
