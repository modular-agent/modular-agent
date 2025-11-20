use std::sync::OnceLock;
use std::vec;

use agent_stream_kit::{
    ASKit, Agent, AgentConfigs, AgentContext, AgentData, AgentDefinition, AgentError, AgentOutput,
    AgentValue, AgentValueMap, AsAgent, AsAgentData, async_trait, new_agent_boxed,
};

use rhai::{AST, Dynamic, Engine, Scope};

static RHAI_ENGINE: OnceLock<Engine> = OnceLock::new();

fn get_engine() -> &'static Engine {
    RHAI_ENGINE.get_or_init(|| {
        let engine = Engine::new();
        engine
    })
}

// Rhai Script
struct RhaiScriptAgent {
    data: AsAgentData,
    ast: Option<AST>,
}

impl RhaiScriptAgent {
    fn set_script(&mut self, script: String) -> Result<(), AgentError> {
        let engine = get_engine();
        if script.is_empty() {
            self.ast = None;
            return Ok(());
        }
        let ast = engine
            .compile(&script)
            .map_err(|e| AgentError::IoError(format!("Rhai Compile Error: {}", e)))?;
        self.ast = Some(ast);
        Ok(())
    }
}

#[async_trait]
impl AsAgent for RhaiScriptAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        let script = config
            .as_ref()
            .and_then(|c| c.get_string(CONFIG_SCRIPT).ok())
            .unwrap_or_default();
        let mut agent = Self {
            data: AsAgentData::new(askit, id, def_name, config),
            ast: None,
        };
        if !script.is_empty() {
            agent.set_script(script)?;
        }
        Ok(agent)
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    fn configs_changed(&mut self) -> Result<(), AgentError> {
        let engine = get_engine();
        let script = self.configs()?.get_string(CONFIG_SCRIPT)?;
        if script.is_empty() {
            self.ast = None;
            return Ok(());
        }
        let ast = engine
            .compile(&script)
            .map_err(|e| AgentError::IoError(format!("Rhai Compile Error: {}", e)))?;
        self.ast = Some(ast);
        Ok(())
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        data: AgentData,
    ) -> Result<(), AgentError> {
        let Some(ast) = &self.ast else {
            return Ok(());
        };
        let engine = get_engine();

        let mut scope = Scope::new();
        // scope.push("ctx", Dynamic::from(ctx.clone()));
        scope.push("data", from_data_to_dynamic(data)?);

        let result = engine
            .eval_ast_with_scope::<Dynamic>(&mut scope, ast)
            .map_err(|e| AgentError::IoError(format!("Rhai Runtime Error: {}", e)))?;

        let out_data: AgentData = from_dynamic_to_data(&result)?;

        self.try_output(ctx, PORT_DATA, out_data)
    }
}

fn from_data_to_dynamic(data: AgentData) -> Result<Dynamic, AgentError> {
    from_value_to_dynamic(data.value)
}

fn from_value_to_dynamic(value: AgentValue) -> Result<Dynamic, AgentError> {
    match value {
        AgentValue::Unit => Ok(().into()),
        AgentValue::Boolean(b) => Ok(Dynamic::from(b)),
        AgentValue::Integer(i) => Ok(Dynamic::from(i)),
        AgentValue::Number(f) => Ok(Dynamic::from(f)),
        AgentValue::String(s) => Ok(Dynamic::from((*s).clone())),
        AgentValue::Array(arr) => {
            let mut dyn_arr: Vec<Dynamic> = Vec::with_capacity(arr.len());
            for v in arr.iter() {
                let d = from_value_to_dynamic(v.clone())?;
                dyn_arr.push(d);
            }
            Ok(Dynamic::from_array(dyn_arr))
        }
        AgentValue::Object(map) => {
            let mut dyn_map = rhai::Map::new();
            for (k, v) in map.iter() {
                let d = from_value_to_dynamic(v.clone())?;
                dyn_map.insert(k.into(), d);
            }
            Ok(Dynamic::from_map(dyn_map))
        }

        // Just store AgentValue directly
        _ => Ok(Dynamic::from(value)),
    }
}

fn from_dynamic_to_data(value: &Dynamic) -> Result<AgentData, AgentError> {
    let agent_value = from_dynamic_to_value(value)?;
    Ok(AgentData::from_value(agent_value))
}

fn from_dynamic_to_value(value: &Dynamic) -> Result<AgentValue, AgentError> {
    if value.is_unit() {
        return Ok(AgentValue::unit());
    }
    if value.is_bool() {
        let value = value
            .as_bool()
            .map_err(|e| AgentError::InvalidValue(format!("Failed as_bool: {}", e)))?;
        return Ok(AgentValue::boolean(value));
    }
    if value.is_int() {
        let value = value
            .as_int()
            .map_err(|e| AgentError::InvalidValue(format!("Failed as_int: {}", e)))?;
        return Ok(AgentValue::integer(value));
    }
    if value.is_float() {
        let value = value
            .as_float()
            .map_err(|e| AgentError::InvalidValue(format!("Failed as_float: {}", e)))?;
        return Ok(AgentValue::number(value));
    }
    if value.is_string() {
        let value = value
            .clone()
            .into_string()
            .map_err(|e| AgentError::InvalidValue(format!("Failed into_string: {}", e)))?;
        return Ok(AgentValue::string(value));
    }

    if value.is_array() {
        let arr = value
            .as_array_ref()
            .map_err(|e| AgentError::InvalidValue(format!("Failed as_array_ref: {}", e)))?;
        let mut value_array: Vec<AgentValue> = Vec::with_capacity(arr.len());
        for v in arr.iter() {
            let d = from_dynamic_to_value(v)?;
            value_array.push(d);
        }
        return Ok(AgentValue::array(value_array));
    }

    if value.is_map() {
        let map = value
            .as_map_ref()
            .map_err(|e| AgentError::InvalidValue(format!("Failed as_map_ref: {}", e)))?;
        let mut value_map = AgentValueMap::new();
        for (k, v) in map.iter() {
            let av = from_dynamic_to_value(v)?;
            value_map.insert(k.to_string(), av);
        }
        return Ok(AgentValue::object(value_map));
    }

    if value.is::<AgentValue>() {
        let value = value.clone().cast::<AgentValue>();
        return Ok(value);
    }

    Err(AgentError::InvalidValue(format!(
        "Unsupported Rhai data type: {}",
        value.type_name()
    )))
}

static AGENT_KIND: &str = "agent";
static CATEGORY: &str = "Scripting";

static PORT_DATA: &str = "data";

static CONFIG_SCRIPT: &str = "script";

pub fn register_agents(askit: &ASKit) {
    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "rhai_script",
            Some(new_agent_boxed::<RhaiScriptAgent>),
        )
        .title("Rhai Script")
        .category(CATEGORY)
        .inputs(vec![PORT_DATA])
        .outputs(vec![PORT_DATA])
        .text_config_with(CONFIG_SCRIPT, "", |entry| entry.title("Script")),
    );
}
