use std::sync::{Arc, Mutex};
use std::vec;

use async_trait::async_trait;

use agent_stream_kit::{
    ASKit, Agent, AgentConfig, AgentConfigEntry, AgentContext, AgentData, AgentDefinition,
    AgentError, AgentOutput, AgentValue, AgentValueMap, AsAgent, AsAgentData, new_boxed,
};
use rig::OneOrMany;
use rig::client::CompletionClient;
use rig::completion::CompletionRequestBuilder;
use rig::providers::ollama::{self, ClientBuilder};

// Memory Agent
//
// Retains the last `n` of the input data and outputs them.
// The output data `kind` matches that of the first data.
pub struct RigMemoryAgent {
    data: AsAgentData,
    memory: Vec<AgentValue>,
}

#[async_trait]
impl AsAgent for RigMemoryAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfig>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
            memory: vec![],
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    async fn process(&mut self, ctx: AgentContext, data: AgentData) -> Result<(), AgentError> {
        if ctx.ch() == CH_RESET {
            // Reset command empties the memory
            self.memory.clear();

            self.try_output(
                ctx,
                CH_MEMORY,
                AgentData::new_array("message", self.memory.clone()),
            )?;

            return Ok(());
        }

        let (user_message, history) = data_to_message_history(data)?;

        // Merge the history with memory
        self.memory.extend(history);

        // Trim to max size if needed
        if let Some(n) = self
            .config()
            .ok_or(AgentError::NoConfig)?
            .get_integer(CONFIG_N)
        {
            if n > 0 {
                let n = n as usize;

                // If the n is smaller than the current number of data,
                // trim the oldest data to fit the n
                if n < self.memory.len() {
                    let data_to_remove = self.memory.len() - n;
                    self.memory.drain(0..data_to_remove);
                }
            }
        }

        if let Some(user_message) = user_message {
            let mut map = user_message
                .value
                .as_object()
                .ok_or_else(|| {
                    AgentError::InvalidValue("user message is not an object".to_string())
                })?
                .clone();
            map.insert(
                "history".to_string(),
                AgentValue::new_array(self.memory.clone()),
            );

            self.try_output(
                ctx.clone(),
                CH_MESSAGE,
                AgentData::new_custom_object("message", map),
            )?;

            // Add the user message to the memory
            self.memory.push(user_message.value.clone());
        }

        self.try_output(
            ctx,
            CH_MEMORY,
            AgentData::new_array("message", self.memory.clone()),
        )?;

        Ok(())
    }
}

fn data_to_message_history(
    data: AgentData,
) -> Result<(Option<AgentData>, Vec<AgentValue>), AgentError> {
    value_to_message_history(data.value)
}

fn value_to_message_history(
    value: AgentValue,
) -> Result<(Option<AgentData>, Vec<AgentValue>), AgentError> {
    if value.is_array() {
        let arr = value
            .as_array()
            .ok_or_else(|| AgentError::InvalidValue("Array is empty".to_string()))?
            .to_owned();
        let mut out_value = Vec::new();
        for item in arr {
            let (message, history) = value_to_message_history(item)?;
            out_value.extend(history);
            if let Some(message) = message {
                out_value.push(message.value);
            }
        }

        // If the last message is from the user, return it as a message.
        let last_role = out_value
            .last()
            .and_then(|m| m.get_str("role"))
            .unwrap_or_default();
        if last_role == "user" {
            let last_message = out_value.pop().unwrap();
            return Ok((
                Some(AgentData::new_custom_object(
                    "message",
                    last_message
                        .as_object()
                        .ok_or_else(|| {
                            AgentError::InvalidValue("last message is not an object".to_string())
                        })?
                        .to_owned(),
                )),
                out_value,
            ));
        }

        return Ok((None, out_value));
    }

    if value.is_string() {
        let mut map = AgentValueMap::new();
        map.insert("content".to_string(), value.clone());
        map.insert("role".to_string(), AgentValue::new_string("user"));
        return Ok((Some(AgentData::new_custom_object("message", map)), vec![]));
    }

    if value.is_object() {
        let map = value
            .as_object()
            .ok_or_else(|| AgentError::InvalidValue("wrong object".to_string()))?;
        let Some(role) = map.get("role") else {
            return Err(AgentError::InvalidValue("data has no role".to_string()));
        };
        let Some(role) = role.as_str() else {
            return Err(AgentError::InvalidValue("role is not a string".to_string()));
        };

        if role == "user" {
            return Ok((
                Some(AgentData::new_custom_object("message", map.to_owned())),
                vec![],
            ));
        }

        // If the role is not "user", return the data as history.
        return Ok((None, vec![value]));
    }

    Err(AgentError::InvalidValue(
        "Unsupported data type".to_string(),
    ))
}

// Rig Ollama Agent
pub struct RigOllamaAgent {
    data: AsAgentData,
    client: Arc<Mutex<Option<ollama::Client>>>,
}

impl RigOllamaAgent {
    // fn get_ollama_url(&self) -> Result<String, AgentError> {
    //     let mut ollama_url = self
    //         .get_global_config()
    //         .ok_or(AgentError::NoGlobalConfig)?
    //         .get_string_or_default(CONFIG_OLLAMA_URL);
    //     if ollama_url.is_empty() {
    //         if let Ok(ollama_host) = std::env::var("OLLAMA_HOST") {
    //             ollama_url = format!("http://{}", ollama_host);
    //         } else {
    //             ollama_url = DEFAULT_OLLAMA_URL.to_string();
    //         }
    //     }
    //     Ok(ollama_url)
    // }

    fn get_client(&mut self) -> Result<ollama::Client, AgentError> {
        let mut client_guard = self.client.lock().unwrap();

        if let Some(client) = client_guard.as_ref() {
            return Ok(client.clone());
        }

        // let ollama_url = self.get_ollama_url()?;
        // let new_client = ollama::Client::from_url(&api_base);
        let api_base = std::env::var("OLLAMA_API_BASE_URL").expect("OLLAMA_API_BASE_URL not set");
        let new_client = ClientBuilder::new()
            .base_url(&api_base)
            .build()
            .map_err(|e| AgentError::IoError(format!("Failed to create Ollama client: {}", e)))?;

        *client_guard = Some(new_client.clone());

        Ok(new_client)
    }
}

#[async_trait]
impl AsAgent for RigOllamaAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfig>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
            client: Arc::new(Mutex::new(None)),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    async fn process(&mut self, ctx: AgentContext, data: AgentData) -> Result<(), AgentError> {
        let config_model = &self
            .config()
            .ok_or(AgentError::NoConfig)?
            .get_string_or_default(CONFIG_MODEL);
        if config_model.is_empty() {
            return Ok(());
        }

        let client = self.get_client()?;
        let comp_model = client.completion_model(config_model);

        let prompts = data_to_prompts(data)?;

        let mut out_messages = Vec::new();
        let mut out_responses = Vec::new();

        for prompt in prompts {
            let comp_model = comp_model.clone();
            let user_message = prompt.message;

            let mut builder = CompletionRequestBuilder::new(comp_model, user_message);
            if let Some(preamble) = prompt.preamble {
                builder = builder.preamble(preamble);
            }
            if prompt.history.len() > 0 {
                builder = builder.messages(prompt.history);
            }
            let response = builder
                .send()
                .await
                .map_err(|e| AgentError::IoError(format!("Ollama Error: {}", e)))?;

            let msg_json = serde_json::to_value(response.raw_response.message.clone())
                .map_err(|e| AgentError::InvalidValue(format!("serde_json error: {}", e)))?;
            let msg_value = AgentValue::from_json_value(msg_json)?;
            out_messages.push(msg_value);

            let resp_json = serde_json::to_value(response.raw_response)
                .map_err(|e| AgentError::InvalidValue(format!("serde_json error: {}", e)))?;
            let resp_value = AgentValue::from_json_value(resp_json)?;
            out_responses.push(resp_value);
        }

        if out_messages.len() == 1 {
            let out_message = AgentData::new_custom_object(
                "message",
                out_messages[0]
                    .as_object()
                    .ok_or_else(|| AgentError::InvalidValue("wrong object".to_string()))?
                    .to_owned(),
            );
            self.try_output(ctx.clone(), CH_MESSAGE, out_message)?;
        } else if out_messages.len() > 1 {
            let out_message = AgentData::new_array("message", out_messages);
            self.try_output(ctx.clone(), CH_MESSAGE, out_message)?;
        }

        if out_responses.len() == 1 {
            let out_response = AgentData::new_custom_object(
                "response",
                out_responses[0]
                    .as_object()
                    .ok_or_else(|| AgentError::InvalidValue("wrong object".to_string()))?
                    .to_owned(),
            );
            self.try_output(ctx, CH_RESPONSE, out_response)?;
        } else if out_responses.len() > 1 {
            let out_response = AgentData::new_array("response", out_responses);
            self.try_output(ctx, CH_RESPONSE, out_response)?;
        }

        Ok(())
    }
}

struct Prompt {
    message: rig::completion::Message,
    preamble: Option<String>,
    history: Vec<rig::completion::Message>,
}

fn data_to_prompts(data: AgentData) -> Result<Vec<Prompt>, AgentError> {
    let mut prompts = Vec::new();

    if data.is_array() {
        let arr = data
            .as_array()
            .ok_or_else(|| AgentError::InvalidValue("Array is empty".to_string()))?
            .to_owned();
        for item in arr {
            let preamble = preamble_from_value(&item);
            let history = history_from_value(&item);
            let user_message = value_to_user_message(item)?;
            prompts.push(Prompt {
                message: user_message,
                preamble,
                history,
            });
        }
        return Ok(prompts);
    }

    let preamble = preamble_from_value(&data.value);
    let history = history_from_value(&data.value);
    let user_message = value_to_user_message(data.value)?;

    prompts.push(Prompt {
        message: user_message,
        preamble,
        history,
    });

    Ok(prompts)
}

fn preamble_from_value(value: &AgentValue) -> Option<String> {
    if value.is_string() {
        return None;
    }

    if value.is_object() {
        return value.get_str("preamble").map(|s| s.to_string());
    }

    None
}

fn history_from_value(value: &AgentValue) -> Vec<rig::completion::Message> {
    if value.is_object() {
        if let Some(history) = value.get("history") {
            if history.is_array() {
                if let Some(arr) = history.as_array() {
                    let mut messages = Vec::new();
                    for item in arr.iter() {
                        let message = value_to_message(item.clone()).unwrap();
                        messages.push(message);
                    }
                    return messages;
                }
            }
        }
    }

    vec![]
}

fn value_to_user_message(value: AgentValue) -> Result<rig::completion::Message, AgentError> {
    if value.is_string() {
        let text = value
            .as_str()
            .ok_or_else(|| AgentError::InvalidValue("wrong string".to_string()))?;
        return Ok(rig::completion::Message::user(text));
    }

    if value.is_object() {
        let role = value.get_str("role").unwrap_or_default();
        if !(role.is_empty() || role == "user") {
            return Err(AgentError::InvalidValue("role is not user".to_string()));
        }

        let content = value.get_str("content").or_else(|| value.get_str("text"));

        // let mut images: Option<Vec<String>> = None;
        // if let Some(image) = value.get("image") {
        //     if image.is_image() {
        //         let image = image.as_image().context("wrong image")?.get_base64();
        //         images = Some(vec![image]);
        //     } else if image.is_string() {
        //         let image = image.as_str().context("wrong string")?;
        //         images = Some(vec![image.to_string()]);
        //     } else {
        //         bail!("invalid image property");
        //     }
        // } else if let Some(images_value) = value.get("images") {
        //     if images_value.is_array() {
        //         let arr = images_value.as_array().context("wrong array")?;
        //         let mut images_vec = Vec::new();
        //         for image in arr.iter() {
        //             if image.is_image() {
        //                 let image = image.as_image().context("wrong image")?;
        //                 images_vec.push(image.get_base64().to_string());
        //             } else if image.is_string() {
        //                 let image = image.as_str().context("wrong string")?;
        //                 images_vec.push(image.to_string());
        //             } else {
        //                 bail!("invalid images property");
        //             }
        //         }
        //         images = Some(images_vec);
        //     } else {
        //         bail!("invalid images property");
        //     }
        // }

        // if content.is_none() && images.is_none() {
        //     bail!("Both content and images are None");
        // }

        let mut items = Vec::new();
        if content.is_some() {
            items.push(rig::completion::message::UserContent::Text(
                rig::completion::message::Text {
                    text: content.unwrap().to_string(),
                },
            ));
        }
        // if images.is_some() {
        //     for image in images.unwrap() {
        //         items.push(rig::completion::message::UserContent::Image(
        //             rig::completion::message::Image {
        //                 data: image
        //                     .trim_start_matches("data:image/png;base64,")
        //                     .to_string(),
        //                 format: None,
        //                 media_type: None,
        //                 detail: None,
        //             },
        //         ));
        //     }
        // }

        return Ok(rig::completion::Message::User {
            content: OneOrMany::many(items)
                .map_err(|e| AgentError::InvalidValue(format!("OneOrMany error: {}", e)))?,
        });
    };

    Err(AgentError::InvalidValue(
        "Unsupported data type".to_string(),
    ))
}

fn value_to_message(value: AgentValue) -> Result<rig::completion::Message, AgentError> {
    if value.is_string() {
        let text = value
            .as_str()
            .ok_or_else(|| AgentError::InvalidValue("wrong string".to_string()))?;
        return Ok(rig::completion::Message::user(text));
    }

    if value.is_object() {
        let role = value.get_str("role").unwrap_or_default();

        let content = value.get_str("content").or_else(|| value.get_str("text"));

        // let mut images: Option<Vec<String>> = None;
        // if let Some(image) = value.get("image") {
        //     if image.is_image() {
        //         let image = image.as_image().context("wrong image")?.get_base64();
        //         images = Some(vec![image]);
        //     } else if image.is_string() {
        //         let image = image.as_str().context("wrong string")?;
        //         images = Some(vec![image.to_string()]);
        //     } else {
        //         bail!("invalid image property");
        //     }
        // } else if let Some(images_value) = value.get("images") {
        //     if images_value.is_array() {
        //         let arr = images_value.as_array().context("wrong array")?;
        //         let mut images_vec = Vec::new();
        //         for image in arr.iter() {
        //             if image.is_image() {
        //                 let image = image.as_image().context("wrong image")?;
        //                 images_vec.push(image.get_base64().to_string());
        //             } else if image.is_string() {
        //                 let image = image.as_str().context("wrong string")?;
        //                 images_vec.push(image.to_string());
        //             } else {
        //                 bail!("invalid images property");
        //             }
        //         }
        //         images = Some(images_vec);
        //     } else {
        //         bail!("invalid images property");
        //     }
        // }

        // if content.is_none() && images.is_none() {
        //     bail!("Both content and images are None");
        // }

        if role == "user" || role == "system" {
            // TODO: system is only available in Ollama
            let mut items = Vec::new();
            if content.is_some() {
                items.push(rig::completion::message::UserContent::Text(
                    rig::completion::message::Text {
                        text: content.unwrap().to_string(),
                    },
                ));
            }
            // if images.is_some() {
            //     for image in images.unwrap() {
            //         items.push(rig::completion::message::UserContent::Image(
            //             rig::completion::message::Image {
            //                 data: image
            //                     .trim_start_matches("data:image/png;base64,")
            //                     .to_string(),
            //                 format: None,
            //                 media_type: None,
            //                 detail: None,
            //             },
            //         ));
            //     }
            // }

            return Ok(rig::completion::Message::User {
                content: OneOrMany::many(items)
                    .map_err(|e| AgentError::InvalidValue(format!("OneOrMany error: {}", e)))?,
            });
        }

        if role == "assistant" {
            return Ok(rig::completion::Message::Assistant {
                id: None,
                content: OneOrMany::one(rig::completion::message::AssistantContent::Text(
                    rig::completion::message::Text {
                        text: content.unwrap().to_string(),
                    },
                )),
            });
        }
    };

    Err(AgentError::InvalidValue("Unsupported data type".into()))
}

// Rig Preamble Agent
pub struct RigPreambleAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for RigPreambleAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfig>,
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

    async fn process(&mut self, ctx: AgentContext, data: AgentData) -> Result<(), AgentError> {
        let preamble = self
            .config()
            .ok_or(AgentError::NoConfig)?
            .get_string_or_default(CONFIG_TEXT);

        if preamble.is_empty() {
            self.try_output(ctx, CH_MESSAGE, data)?;
            return Ok(());
        }

        let data = add_preamble_to_data(preamble, data)?;

        self.try_output(ctx, CH_MESSAGE, data)?;

        Ok(())
    }
}

fn add_preamble_to_data(preamble: String, data: AgentData) -> Result<AgentData, AgentError> {
    let value = add_preamble_to_value(preamble, data.value)?;

    if value.is_object() {
        let map = value
            .as_object()
            .ok_or_else(|| AgentError::InvalidValue("wrong object".to_string()))?
            .to_owned();
        return Ok(AgentData::new_custom_object("message", map));
    }

    if value.is_array() {
        let arr = value
            .as_array()
            .ok_or_else(|| AgentError::InvalidValue("wrong array".to_string()))?
            .to_owned();
        return Ok(AgentData::new_array("message", arr));
    }

    Err(AgentError::InvalidValue(
        "Unsupported data type".to_string(),
    ))
}

fn add_preamble_to_value(preamble: String, value: AgentValue) -> Result<AgentValue, AgentError> {
    if value.is_string() {
        let content = value
            .as_str()
            .ok_or_else(|| AgentError::InvalidValue("wrong string".to_string()))?;
        let mut out_value = AgentValueMap::new();
        out_value.insert("content".to_string(), AgentValue::new_string(content));
        out_value.insert("role".to_string(), AgentValue::new_string("user"));
        out_value.insert("preamble".to_string(), AgentValue::new_string(preamble));
        return Ok(AgentValue::new_object(out_value));
    }

    if value.is_object() {
        let mut out_value = value
            .as_object()
            .ok_or_else(|| AgentError::InvalidValue("wrong object value".to_string()))?
            .clone();
        out_value.insert("preamble".to_string(), AgentValue::new_string(preamble));
        return Ok(AgentValue::new_object(out_value));
    }

    if value.is_array() {
        let arr = value
            .as_array()
            .ok_or_else(|| AgentError::InvalidValue("wrong array".to_string()))?
            .to_owned();
        let mut out_value = Vec::new();
        for item in arr {
            let item = add_preamble_to_value(preamble.clone(), item)?;
            out_value.push(item);
        }
        return Ok(AgentValue::new_array(out_value));
    }

    return Err(AgentError::InvalidValue(
        "Unsupported value type".to_string(),
    ));
}

// Rig User Message with Image Agent
pub struct RigUserMessageWithImageAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for RigUserMessageWithImageAgent {
    fn new(
        akit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfig>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(akit, id, def_name, config),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    async fn process(&mut self, ctx: AgentContext, data: AgentData) -> Result<(), AgentError> {
        let text = self
            .config()
            .ok_or(AgentError::NoConfig)?
            .get_string_or_default(CONFIG_TEXT);

        let out_data = combine_text_and_image_data(text, data)?;

        self.try_output(ctx, CH_MESSAGE, out_data)?;

        Ok(())
    }
}

fn combine_text_and_image_data(text: String, data: AgentData) -> Result<AgentData, AgentError> {
    let value = combine_text_and_image_value(text, data.value)?;

    if value.is_object() {
        let map = value
            .as_object()
            .ok_or_else(|| AgentError::InvalidValue("wrong object".to_string()))?
            .to_owned();
        return Ok(AgentData::new_custom_object("message", map));
    }

    if value.is_array() {
        let arr = value
            .as_array()
            .ok_or_else(|| AgentError::InvalidValue("wrong array".to_string()))?
            .to_owned();
        return Ok(AgentData::new_array("message", arr));
    }

    Err(AgentError::InvalidValue(
        "Unsupported data type".to_string(),
    ))
}

fn combine_text_and_image_value(text: String, value: AgentValue) -> Result<AgentValue, AgentError> {
    // if value.is_image() || value.is_string() {
    if value.is_string() {
        let mut out_value = AgentValueMap::new();
        out_value.insert("images".to_string(), AgentValue::new_array(vec![value]));
        out_value.insert("role".to_string(), AgentValue::new_string("user"));
        out_value.insert("content".to_string(), AgentValue::new_string(text));
        return Ok(AgentValue::new_object(out_value));
    }

    // if value.is_object() {
    //     let mut out_value = value.as_object().context("wrong object value")?.clone();
    //     if let Some(images) = value.get("images") {
    //         if images.is_array() {
    //             let images = images.as_array().context("wrong array")?.clone();
    //             out_value.insert("images".to_string(), AgentValue::new_array(images));
    //         } else {
    //             bail!("images is not an array");
    //         }
    //     } else if let Some(image) = value.get("image") {
    //         if image.is_image() {
    //             out_value.insert(
    //                 "images".to_string(),
    //                 AgentValue::new_array(vec![image.clone()]),
    //             );
    //         } else {
    //             bail!("image is not an image");
    //         }
    //     } else {
    //         bail!("image or images are not set");
    //     }
    //     out_value.insert("role".to_string(), AgentValue::new_string("user"));
    //     out_value.insert("content".to_string(), AgentValue::new_string(text));
    //     return Ok(AgentValue::new_object(out_value));
    // }

    if value.is_array() {
        let arr = value
            .as_array()
            .ok_or_else(|| AgentError::InvalidValue("wrong array".to_string()))?
            .to_owned();
        let mut out_value = Vec::new();
        for item in arr {
            let item = combine_text_and_image_value(text.clone(), item)?;
            out_value.push(item);
        }
        return Ok(AgentValue::new_array(out_value));
    }

    Err(AgentError::InvalidValue(
        "Unsupported value type".to_string(),
    ))
}

static AGENT_KIND: &str = "agent";
static CATEGORY: &str = "Core/Rig";

static CH_IMAGE: &str = "image";
static CH_MEMORY: &str = "memory";
static CH_MESSAGE: &str = "message";
static CH_RESET: &str = "reset";
static CH_RESPONSE: &str = "response";

static CONFIG_MODEL: &str = "model";
// static CONFIG_OLLAMA_URL: &str = "ollama_url";
static CONFIG_TEXT: &str = "prompt";
static CONFIG_N: &str = "n";

const DEFAULT_CONFIG_MODEL: &str = "gemma3:4b";
// const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";
const DEFAULT_CONFIG_N: i64 = 10;

pub fn register_agents(askit: &ASKit) {
    askit.register_agent(
        AgentDefinition::new(AGENT_KIND, "rig_memory", Some(new_boxed::<RigMemoryAgent>))
            .with_title("Rig Memory")
            .with_description("Stores recent input data")
            .with_category(CATEGORY)
            .with_inputs(vec![CH_MESSAGE, CH_RESET])
            .with_outputs(vec![CH_MESSAGE, CH_MEMORY])
            .with_default_config(vec![(
                CONFIG_N.into(),
                AgentConfigEntry::new(AgentValue::new_integer(DEFAULT_CONFIG_N), "integer")
                    .with_title("Memory Size")
                    .with_description("-1 = unlimited"),
            )]),
    );

    askit.register_agent(
        AgentDefinition::new(AGENT_KIND, "rig_ollama", Some(new_boxed::<RigOllamaAgent>))
            // .use_native_thread()
            .with_title("Rig Ollama")
            .with_category(CATEGORY)
            .with_inputs(vec![CH_MESSAGE])
            .with_outputs(vec![CH_MESSAGE, CH_RESPONSE])
            // .with_global_config(vec![(
            //     CONFIG_OLLAMA_URL.into(),
            //     AgentConfigEntry::new(AgentValue::new_string(DEFAULT_OLLAMA_URL), "string")
            //         .with_title("Ollama URL"),
            // )])
            .with_default_config(vec![(
                CONFIG_MODEL.into(),
                AgentConfigEntry::new(AgentValue::new_string(DEFAULT_CONFIG_MODEL), "string")
                    .with_title("Chat Model"),
            )]),
    );

    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "rig_preamble",
            Some(new_boxed::<RigPreambleAgent>),
        )
        // .use_native_thread()
        .with_title("Rig Preamble")
        .with_category(CATEGORY)
        .with_inputs(vec![CH_MESSAGE])
        .with_outputs(vec![CH_MESSAGE])
        .with_default_config(vec![(
            CONFIG_TEXT.into(),
            AgentConfigEntry::new(AgentValue::new_string(""), "text"),
        )]),
    );

    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "rig_user_message_with_image",
            Some(new_boxed::<RigUserMessageWithImageAgent>),
        )
        // .use_native_thread()
        .with_title("Rig User Message with Image")
        .with_category(CATEGORY)
        .with_inputs(vec![CH_IMAGE])
        .with_outputs(vec![CH_MESSAGE])
        .with_default_config(vec![(
            CONFIG_TEXT.into(),
            AgentConfigEntry::new(AgentValue::new_string(""), "text"),
        )]),
    );
}
