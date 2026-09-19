use modular_agent_core::{
    AsModule, ModularAgent, ModuleContext, ModuleData, ModuleOutput, ModuleSpec, Result, Value,
    async_trait, modular_agent,
};

const CATEGORY: &str = "Std/UI";

const NOTE: &str = "note";
const PORT_SP: &str = " ";

#[modular_agent(
    kind = "UI",
    title = "Note",
    category = CATEGORY,
    custom_config(name = NOTE, type_="markdown", default="", hide_title),
    hint(color=2, width=240, height=160, free_size=true, background=true, bg_color="#fdf6b2", fg_color="#44403b"),
)]
struct NoteModule {
    data: ModuleData,
}

impl AsModule for NoteModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }
}

#[modular_agent(
    kind = "UI",
    title = "Router",
    hide_title,
    category = CATEGORY,
    inputs=[PORT_SP],
    outputs=[PORT_SP],
    hint(width=64, height=64, free_size=true, no_resize=true),
)]
struct RouterModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for RouterModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, port: String, value: Value) -> Result<()> {
        self.output(ctx, port, value).await
    }
}
