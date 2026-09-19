use std::sync::atomic::AtomicUsize;

use crate::{
    FnvIndexMap,
    spec::{ConnectionSpec, ModuleSpec},
};

static ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

pub(crate) fn new_id() -> String {
    ID_COUNTER
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        .to_string()
}

pub(crate) fn update_ids(
    modules: &Vec<ModuleSpec>,
    connections: &Vec<ConnectionSpec>,
) -> (Vec<ModuleSpec>, Vec<ConnectionSpec>) {
    let mut new_modules = Vec::new();
    let mut module_id_map = FnvIndexMap::default();
    for module in modules {
        let new_id = new_id();
        module_id_map.insert(module.id.clone(), new_id.clone());
        let mut new_module = module.clone();
        new_module.id = new_id;
        new_modules.push(new_module);
    }

    let mut new_connections = Vec::new();
    for connection in connections {
        let source = module_id_map
            .get(&connection.source)
            .cloned()
            .unwrap_or_else(|| connection.source.clone());
        let target = module_id_map
            .get(&connection.target)
            .cloned()
            .unwrap_or_else(|| connection.target.clone());
        let mut new_connection = connection.clone();
        new_connection.source = source.clone();
        new_connection.target = target.clone();
        new_connections.push(new_connection);
    }

    (new_modules, new_connections)
}
