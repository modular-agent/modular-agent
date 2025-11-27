use std::sync::Arc;

use agent_stream_kit::{
    ASKit, Agent, AgentConfigs, AgentContext, AgentError, AgentOutput, AgentValue, AsAgent,
    AsAgentData, async_trait,
};
use askit_macros::askit_agent;

#[cfg(feature = "image")]
use photon_rs::PhotonImage;

static CATEGORY: &str = "Std/Image";

static PIN_FILENAME: &str = "filename";
static PIN_IMAGE: &str = "image";
static PIN_IMAGE_FILENAME: &str = "image_filename";
static PIN_BLANK: &str = "blank";
static PIN_NON_BLANK: &str = "non_blank";
static PIN_CHANGED: &str = "changed";
static PIN_UNCHANGED: &str = "unchanged";
static PIN_RESULT: &str = "result";

static CONFIG_ALMOST_BLACK_THRESHOLD: &str = "almost_black_threshold";
static CONFIG_BLANK_THRESHOLD: &str = "blank_threshold";
static CONFIG_SCALE: &str = "scale";
static CONFIG_HEIGHT: &str = "height";
static CONFIG_WIDTH: &str = "width";
static CONFIG_THRESHOLD: &str = "threshold";

// IsBlankImageAgent
#[askit_agent(
    title = "isBlank",
    category = CATEGORY,
    inputs = [PIN_IMAGE],
    outputs = [PIN_BLANK, PIN_NON_BLANK],
    integer_config(name = CONFIG_ALMOST_BLACK_THRESHOLD, default = 20),
    integer_config(name = CONFIG_BLANK_THRESHOLD, default = 400)
)]
struct IsBlankImageAgent {
    data: AsAgentData,
}

impl IsBlankImageAgent {
    fn is_blank(
        &self,
        image: &PhotonImage,
        almost_black_threshold: u8,
        blank_threshold: u32,
    ) -> bool {
        let mut count = 0;
        for pixel in image.get_raw_pixels() {
            if pixel >= almost_black_threshold {
                count += 1;
            }
            if count >= blank_threshold {
                return false;
            }
        }
        true
    }
}

#[async_trait]
impl AsAgent for IsBlankImageAgent {
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
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let config = self.configs()?;

        if value.is_image() {
            let image = value
                .as_image()
                .ok_or_else(|| AgentError::InvalidValue("Expected image value".into()))?;

            let almost_black_threshold =
                config.get_integer_or_default(CONFIG_ALMOST_BLACK_THRESHOLD) as u8;
            let blank_threshold = config.get_integer_or_default(CONFIG_BLANK_THRESHOLD) as u32;

            let is_blank = self.is_blank(&image, almost_black_threshold, blank_threshold);
            if is_blank {
                self.try_output(ctx, PIN_BLANK, value)
            } else {
                self.try_output(ctx, PIN_NON_BLANK, value)
            }
        } else {
            Err(AgentError::InvalidValue(
                "Input value is not an image".into(),
            ))
        }
    }
}

// ResampleImageAgent

#[askit_agent(
    title = "Resize Image",
    category = CATEGORY,
    inputs = [PIN_IMAGE],
    outputs = [PIN_IMAGE],
    integer_config(name = CONFIG_WIDTH, default = 512),
    integer_config(name = CONFIG_HEIGHT, default = 512)
)]
struct ResampleImageAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for ResampleImageAgent {
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
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let config = self.configs()?;

        if value.is_image() {
            let image = value
                .as_image()
                .ok_or_else(|| AgentError::InvalidValue("Expected image value".into()))?;

            let width = config.get_integer_or_default(CONFIG_WIDTH) as usize;
            let height = config.get_integer_or_default(CONFIG_HEIGHT) as usize;

            let resampled_image = photon_rs::transform::resample(&*image, width, height);

            self.try_output(ctx, PIN_IMAGE, AgentValue::image(resampled_image))
        } else {
            // Pass through non-image value
            self.try_output(ctx, PIN_IMAGE, value)
        }
    }
}

// ResizeImageAgent

#[askit_agent(
    title = "Resize Image",
    category = CATEGORY,
    inputs = [PIN_IMAGE],
    outputs = [PIN_IMAGE],
    integer_config(name = CONFIG_WIDTH, default = 512),
    integer_config(name = CONFIG_HEIGHT, default = 512)
)]
struct ResizeImageAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for ResizeImageAgent {
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
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let config = self.configs()?;

        if value.is_image() {
            let image = value
                .as_image()
                .ok_or_else(|| AgentError::InvalidValue("Expected image value".into()))?;

            let width = config.get_integer_or_default(CONFIG_WIDTH) as u32;
            let height = config.get_integer_or_default(CONFIG_HEIGHT) as u32;

            let resized_image = photon_rs::transform::resize(
                &*image,
                width,
                height,
                photon_rs::transform::SamplingFilter::Nearest,
            );

            self.try_output(ctx, PIN_IMAGE, AgentValue::image(resized_image))
        } else {
            // Pass through non-image value
            self.try_output(ctx, PIN_IMAGE, value)
        }
    }
}

// ScaleImageAgent

#[askit_agent(
    title = "Scale Image",
    category = CATEGORY,
    inputs = [PIN_IMAGE],
    outputs = [PIN_IMAGE],
    number_config(name = CONFIG_SCALE, default = 1.0)
)]
struct ScaleImageAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for ScaleImageAgent {
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
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let config = self.configs()?;

        if value.is_image() {
            let image = value
                .as_image()
                .ok_or_else(|| AgentError::InvalidValue("Expected image value".into()))?;

            let scale = config.get_number_or_default(CONFIG_SCALE);

            if scale <= 0.0 {
                return Err(AgentError::InvalidValue(
                    "Scale factor must be greater than 0".into(),
                ));
            }

            if scale == 1.0 {
                // No scaling needed, pass through the original image
                return self.try_output(ctx, PIN_IMAGE, value);
            }

            if scale < 1.0 {
                let width = ((image.get_width() as f64) * scale) as u32;
                let height = ((image.get_height() as f64) * scale) as u32;

                let resized_image = photon_rs::transform::resize(
                    &*image,
                    width,
                    height,
                    photon_rs::transform::SamplingFilter::Nearest,
                );
                self.try_output(ctx, PIN_IMAGE, AgentValue::image(resized_image))
            } else {
                // scale > 1.0
                let width = ((image.get_width() as f64) * scale) as usize;
                let height = ((image.get_height() as f64) * scale) as usize;
                let resampled_image = photon_rs::transform::resample(&*image, width, height);
                self.try_output(ctx, PIN_IMAGE, AgentValue::image(resampled_image))
            }
        } else {
            // Pass through non-image value
            self.try_output(ctx, PIN_IMAGE, value)
        }
    }
}

// IsChangedImageAgent
#[askit_agent(
    title = "isChanged",
    category = CATEGORY,
    inputs = [PIN_IMAGE],
    outputs = [PIN_CHANGED, PIN_UNCHANGED],
    number_config(name = CONFIG_THRESHOLD, default = 0.01)
)]
struct IsChangedImageAgent {
    data: AsAgentData,
    last_image: Option<Arc<PhotonImage>>,
}

impl IsChangedImageAgent {
    fn images_are_different(&self, img1: &PhotonImage, img2: &PhotonImage, threshold: f32) -> bool {
        let pixels1 = img1.get_raw_pixels();
        let pixels2 = img2.get_raw_pixels();

        if pixels1.len() != pixels2.len() {
            return true;
        }

        let diff_threshold = (threshold * pixels1.len() as f32) as usize;
        let mut diff_count = 0;
        for (p1, p2) in pixels1.iter().zip(pixels2.iter()) {
            if p1 != p2 {
                diff_count += 1;
            }
            if diff_count > diff_threshold {
                return true;
            }
        }

        false
    }
}

#[async_trait]
impl AsAgent for IsChangedImageAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
            last_image: None,
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
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let config = self.configs()?;

        if value.is_image() {
            let image = value
                .as_image()
                .ok_or_else(|| AgentError::InvalidValue("Expected image value".into()))?;

            let threshold = config.get_number_or_default(CONFIG_THRESHOLD) as f32;

            let is_changed = if let Some(last_image) = &self.last_image {
                self.images_are_different(&last_image, &image, threshold)
            } else {
                true
            };

            if is_changed {
                self.last_image = Some(image.clone());
                self.try_output(ctx, PIN_CHANGED, value)
            } else {
                self.try_output(ctx, PIN_UNCHANGED, value)
            }
        } else {
            Err(AgentError::InvalidValue(
                "Input value is not an image".into(),
            ))
        }
    }
}

// native

#[askit_agent(
    title = "Open Image",
    category = CATEGORY,
    inputs = [PIN_FILENAME],
    outputs = [PIN_IMAGE]
)]
struct OpenImageAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for OpenImageAgent {
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
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let filename = value
            .as_str()
            .ok_or_else(|| AgentError::InvalidValue("Expected filename string".into()))?;
        let img_path = std::path::Path::new(filename);

        let image = photon_rs::native::open_image(img_path).map_err(|e| {
            AgentError::InvalidValue(format!("Failed to open image {}: {}", filename, e))
        })?;

        self.try_output(ctx, PIN_IMAGE, AgentValue::image(image))
    }
}

#[askit_agent(
    title = "Save Image",
    category = CATEGORY,
    inputs = [PIN_IMAGE_FILENAME],
    outputs = [PIN_RESULT]
)]
struct SaveImageAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for SaveImageAgent {
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
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let Some(image) = value.get_image("image") else {
            return Err(AgentError::InvalidValue(
                "Expected image value under 'image' key".into(),
            ));
        };

        let Some(filename) = value.get_str("filename") else {
            return Err(AgentError::InvalidValue(
                "Expected filename string under 'filename' key".into(),
            ));
        };

        photon_rs::native::save_image((*image).clone(), std::path::Path::new(filename)).map_err(
            |e| AgentError::InvalidValue(format!("Failed to save image {}: {}", filename, e)),
        )?;

        self.try_output(ctx, PIN_RESULT, AgentValue::unit())
    }
}
