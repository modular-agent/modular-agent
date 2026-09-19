#![cfg(feature = "image")]

use std::sync::Arc;

use modular_agent_core::photon_rs::{self, PhotonImage};
use modular_agent_core::{
    AsModule, Error, ModularAgent, Module, ModuleContext, ModuleData, ModuleOutput, ModuleSpec,
    Result, Value, async_trait, modular_agent,
};

const CATEGORY: &str = "Std/Image";

const PORT_PATH: &str = "path";
const PORT_IMAGE: &str = "image";
const PORT_IMAGE_FILENAME: &str = "image_filename";
const PORT_T: &str = "t";
const PORT_F: &str = "f";
const PORT_RESULT: &str = "result";

const CONFIG_ALMOST_BLACK_THRESHOLD: &str = "almost_black_threshold";
const CONFIG_BLANK_THRESHOLD: &str = "blank_threshold";
const CONFIG_SCALE: &str = "scale";
const CONFIG_HEIGHT: &str = "height";
const CONFIG_WIDTH: &str = "width";
const CONFIG_THRESHOLD: &str = "threshold";

// IsBlankImageModule
#[modular_agent(
    title = "IsBlank",
    category = CATEGORY,
    inputs = [PORT_IMAGE],
    outputs = [PORT_T, PORT_F],
    integer_config(name = CONFIG_ALMOST_BLACK_THRESHOLD, default = 20),
    integer_config(name = CONFIG_BLANK_THRESHOLD, default = 400)
)]
struct IsBlankImageModule {
    data: ModuleData,
}

impl IsBlankImageModule {
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
impl AsModule for IsBlankImageModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        let config = self.configs()?;

        if value.is_image() {
            let image = value
                .as_image()
                .ok_or_else(|| Error::InvalidValue("Expected image value".into()))?;

            let almost_black_threshold =
                config.get_integer_or_default(CONFIG_ALMOST_BLACK_THRESHOLD) as u8;
            let blank_threshold = config.get_integer_or_default(CONFIG_BLANK_THRESHOLD) as u32;

            let is_blank = self.is_blank(&image, almost_black_threshold, blank_threshold);
            if is_blank {
                self.output(ctx, PORT_T, value).await
            } else {
                self.output(ctx, PORT_F, value).await
            }
        } else {
            Err(Error::InvalidValue("Input value is not an image".into()))
        }
    }
}

// ResampleImageModule

#[modular_agent(
    title = "Resample Image",
    category = CATEGORY,
    inputs = [PORT_IMAGE],
    outputs = [PORT_IMAGE],
    integer_config(name = CONFIG_WIDTH, default = 512),
    integer_config(name = CONFIG_HEIGHT, default = 512)
)]
struct ResampleImageModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for ResampleImageModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        let config = self.configs()?;

        if value.is_image() {
            let image = value
                .as_image()
                .ok_or_else(|| Error::InvalidValue("Expected image value".into()))?;

            let width = config.get_integer_or_default(CONFIG_WIDTH) as usize;
            let height = config.get_integer_or_default(CONFIG_HEIGHT) as usize;

            let resampled_image = photon_rs::transform::resample(&*image, width, height);

            self.output(ctx, PORT_IMAGE, Value::image(resampled_image))
                .await
        } else {
            // Pass through non-image value
            self.output(ctx, PORT_IMAGE, value).await
        }
    }
}

// ResizeImageModule

#[modular_agent(
    title = "Resize Image",
    category = CATEGORY,
    inputs = [PORT_IMAGE],
    outputs = [PORT_IMAGE],
    integer_config(name = CONFIG_WIDTH, default = 512),
    integer_config(name = CONFIG_HEIGHT, default = 512)
)]
struct ResizeImageModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for ResizeImageModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        let config = self.configs()?;

        if value.is_image() {
            let image = value
                .as_image()
                .ok_or_else(|| Error::InvalidValue("Expected image value".into()))?;

            let width = config.get_integer_or_default(CONFIG_WIDTH) as u32;
            let height = config.get_integer_or_default(CONFIG_HEIGHT) as u32;

            let resized_image = photon_rs::transform::resize(
                &*image,
                width,
                height,
                photon_rs::transform::SamplingFilter::Nearest,
            );

            self.output(ctx, PORT_IMAGE, Value::image(resized_image))
                .await
        } else {
            // Pass through non-image value
            self.output(ctx, PORT_IMAGE, value).await
        }
    }
}

// ScaleImageModule

#[modular_agent(
    title = "Scale Image",
    category = CATEGORY,
    inputs = [PORT_IMAGE],
    outputs = [PORT_IMAGE],
    number_config(name = CONFIG_SCALE, default = 1.0)
)]
struct ScaleImageModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for ScaleImageModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        let config = self.configs()?;

        if value.is_image() {
            let image = value
                .as_image()
                .ok_or_else(|| Error::InvalidValue("Expected image value".into()))?;

            let scale = config.get_number_or_default(CONFIG_SCALE);

            if scale <= 0.0 {
                return Err(Error::InvalidValue(
                    "Scale factor must be greater than 0".into(),
                ));
            }

            if scale == 1.0 {
                // No scaling needed, pass through the original image
                return self.output(ctx, PORT_IMAGE, value).await;
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
                self.output(ctx, PORT_IMAGE, Value::image(resized_image))
                    .await
            } else {
                // scale > 1.0
                let width = ((image.get_width() as f64) * scale) as usize;
                let height = ((image.get_height() as f64) * scale) as usize;
                let resampled_image = photon_rs::transform::resample(&*image, width, height);
                self.output(ctx, PORT_IMAGE, Value::image(resampled_image))
                    .await
            }
        } else {
            // Pass through non-image value
            self.output(ctx, PORT_IMAGE, value).await
        }
    }
}

// IsChangedImageModule
#[modular_agent(
    title = "IsChanged",
    category = CATEGORY,
    inputs = [PORT_IMAGE],
    outputs = [PORT_T, PORT_F],
    number_config(name = CONFIG_THRESHOLD, default = 0.01)
)]
struct IsChangedImageModule {
    data: ModuleData,
    last_image: Option<Arc<PhotonImage>>,
}

impl IsChangedImageModule {
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
impl AsModule for IsChangedImageModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
            last_image: None,
        })
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        let config = self.configs()?;

        if value.is_image() {
            let image = value
                .as_image()
                .ok_or_else(|| Error::InvalidValue("Expected image value".into()))?;

            let threshold = config.get_number_or_default(CONFIG_THRESHOLD) as f32;

            let is_changed = if let Some(last_image) = &self.last_image {
                self.images_are_different(&last_image, &image, threshold)
            } else {
                true
            };

            if is_changed {
                self.last_image = value.clone().into_image();
                self.output(ctx, PORT_T, value).await
            } else {
                self.output(ctx, PORT_F, value).await
            }
        } else {
            Err(Error::InvalidValue("Input value is not an image".into()))
        }
    }
}

// native

#[modular_agent(
    title = "Open Image",
    category = CATEGORY,
    inputs = [PORT_PATH],
    outputs = [PORT_IMAGE]
)]
struct OpenImageModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for OpenImageModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        let path = value
            .as_str()
            .ok_or_else(|| Error::InvalidValue("Expected path string".into()))?;
        let img_path = std::path::Path::new(path);

        let image = photon_rs::native::open_image(img_path)
            .map_err(|e| Error::InvalidValue(format!("Failed to open image {}: {}", path, e)))?;

        self.output(ctx, PORT_IMAGE, Value::image(image)).await
    }
}

#[modular_agent(
    title = "Save Image",
    category = CATEGORY,
    inputs = [PORT_IMAGE_FILENAME],
    outputs = [PORT_RESULT]
)]
struct SaveImageModule {
    data: ModuleData,
}

#[async_trait]
impl AsModule for SaveImageModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, value: Value) -> Result<()> {
        let Some(image) = value.get_image("image") else {
            return Err(Error::InvalidValue(
                "Expected image value under 'image' key".into(),
            ));
        };

        let Some(filename) = value.get_str("filename") else {
            return Err(Error::InvalidValue(
                "Expected filename string under 'filename' key".into(),
            ));
        };

        photon_rs::native::save_image((*image).clone(), std::path::Path::new(filename)).map_err(
            |e| Error::InvalidValue(format!("Failed to save image {}: {}", filename, e)),
        )?;

        self.output(ctx, PORT_RESULT, Value::unit()).await
    }
}
