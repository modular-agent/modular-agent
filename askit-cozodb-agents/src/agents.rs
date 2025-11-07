use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};
use std::vec;

use agent_stream_kit::{
    ASKit, Agent, AgentConfigEntry, AgentConfigs, AgentContext, AgentData, AgentDefinition,
    AgentError, AgentOutput, AsAgent, AsAgentData, async_trait, new_agent_boxed,
};
use cozo::DbInstance;

static DB_MAP: OnceLock<Mutex<BTreeMap<String, DbInstance>>> = OnceLock::new();

fn get_db_instance(path: &str) -> Result<DbInstance, AgentError> {
    let db_map = DB_MAP.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut map_guard = db_map.lock().unwrap();

    if let Some(db) = map_guard.get(path) {
        return Ok(db.clone());
    }

    let db = if path.is_empty() {
        DbInstance::new("mem", "", "")
    } else {
        DbInstance::new("sqlite", path, "")
    }
    .map_err(|e| AgentError::IoError(format!("Cozo Error: {}", e)))?;

    map_guard.insert(path.to_string(), db.clone());

    Ok(db)
}

// CozoDB Script
struct CozoDbScriptgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for CozoDbScriptgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        data: AgentData,
    ) -> Result<(), AgentError> {
        let config = self.configs()?;
        let db = get_db_instance(&config.get_string_or_default(CONFIG_DB))?;
        let script = config.get_string(CONFIG_SCRIPT)?;
        if script.is_empty() {
            return Ok(());
        }

        let params: BTreeMap<String, cozo::DataValue> = if let Some(params) = data.as_object() {
            params
                .iter()
                .map(|(k, v)| (k.clone(), v.to_json().into()))
                .collect()
        } else {
            BTreeMap::new()
        };

        let result = db
            .run_script(&script, params, cozo::ScriptMutability::Mutable)
            .map_err(|e| AgentError::IoError(format!("Cozo Error: {}", e)))?;

        let data = AgentData::from_serialize(&result)?;

        self.try_output(ctx, PORT_RESULT, data)
    }
}

static AGENT_KIND: &str = "agent";
static CATEGORY: &str = "Database";

static PORT_PARAMS: &str = "params";
static PORT_RESULT: &str = "result";

static CONFIG_DB: &str = "db";
static CONFIG_SCRIPT: &str = "script";

pub fn register_agents(askit: &ASKit) {
    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "cozodb_script",
            Some(new_agent_boxed::<CozoDbScriptgent>),
        )
        .with_title("CozoDB Script")
        .with_category(CATEGORY)
        .with_inputs(vec![PORT_PARAMS])
        .with_outputs(vec![PORT_RESULT])
        .with_default_configs(vec![
            (
                CONFIG_DB,
                AgentConfigEntry::new("", "string").with_title("Database"),
            ),
            (
                CONFIG_SCRIPT,
                AgentConfigEntry::new("", "text").with_title("Script"),
            ),
        ]),
    );
}
