#![cfg(feature = "image")]

use std::sync::Arc;

use agent_stream_kit::photon_rs::{self, PhotonImage};
use agent_stream_kit::{
    ASKit, Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    askit_agent, async_trait,
};

const CATEGORY: &str = "Std/Image";

const PIN_FILENAME: &str = "filename";
const PIN_IMAGE: &str = "image";
const PIN_IMAGE_FILENAME: &str = "image_filename";
const PIN_BLANK: &str = "blank";
const PIN_NON_BLANK: &str = "non_blank";
const PIN_CHANGED: &str = "changed";
const PIN_UNCHANGED: &str = "unchanged";
const PIN_RESULT: &str = "result";

const CONFIG_ALMOST_BLACK_THRESHOLD: &str = "almost_black_threshold";
const CONFIG_BLANK_THRESHOLD: &str = "blank_threshold";
const CONFIG_SCALE: &str = "scale";
const CONFIG_HEIGHT: &str = "height";
const CONFIG_WIDTH: &str = "width";
const CONFIG_THRESHOLD: &str = "threshold";

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
    data: AgentData,
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
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
        })
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
                self.output(ctx, PIN_BLANK, value).await
            } else {
                self.output(ctx, PIN_NON_BLANK, value).await
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
    data: AgentData,
}

#[async_trait]
impl AsAgent for ResampleImageAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
        })
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

            self.output(ctx, PIN_IMAGE, AgentValue::image(resampled_image))
                .await
        } else {
            // Pass through non-image value
            self.output(ctx, PIN_IMAGE, value).await
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
    data: AgentData,
}

#[async_trait]
impl AsAgent for ResizeImageAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
        })
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

            self.output(ctx, PIN_IMAGE, AgentValue::image(resized_image))
                .await
        } else {
            // Pass through non-image value
            self.output(ctx, PIN_IMAGE, value).await
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
    data: AgentData,
}

#[async_trait]
impl AsAgent for ScaleImageAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
        })
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
                return self.output(ctx, PIN_IMAGE, value).await;
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
                self.output(ctx, PIN_IMAGE, AgentValue::image(resized_image))
                    .await
            } else {
                // scale > 1.0
                let width = ((image.get_width() as f64) * scale) as usize;
                let height = ((image.get_height() as f64) * scale) as usize;
                let resampled_image = photon_rs::transform::resample(&*image, width, height);
                self.output(ctx, PIN_IMAGE, AgentValue::image(resampled_image))
                    .await
            }
        } else {
            // Pass through non-image value
            self.output(ctx, PIN_IMAGE, value).await
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
    data: AgentData,
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
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
            last_image: None,
        })
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
                self.last_image = value.clone().into_image();
                self.output(ctx, PIN_CHANGED, value).await
            } else {
                self.output(ctx, PIN_UNCHANGED, value).await
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
    data: AgentData,
}

#[async_trait]
impl AsAgent for OpenImageAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
        })
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

        self.output(ctx, PIN_IMAGE, AgentValue::image(image)).await
    }
}

#[askit_agent(
    title = "Save Image",
    category = CATEGORY,
    inputs = [PIN_IMAGE_FILENAME],
    outputs = [PIN_RESULT]
)]
struct SaveImageAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for SaveImageAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
        })
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

        self.output(ctx, PIN_RESULT, AgentValue::unit()).await
    }
}
