//! The browser entry points, built for wasm32 with wasm-bindgen.
//! Images come in as the bytes of a png file, enum options as the names the command line takes.

use crate::generator::{self, BodyType, Generated, Input, Species, SpriteOutputType};
use clap::ValueEnum;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Output {
    description: String,
    content: String,
}

#[wasm_bindgen]
impl Output {
    /// what the generator has to say about the result, like the size of the sprite
    #[wasm_bindgen(getter)]
    pub fn description(&self) -> String {
        self.description.clone()
    }

    /// the directives or the item json
    #[wasm_bindgen(getter)]
    pub fn content(&self) -> String {
        self.content.clone()
    }
}

impl From<Generated> for Output {
    fn from(generated: Generated) -> Self {
        Output {
            description: generated.description,
            content: generated.content,
        }
    }
}

fn error(error: anyhow::Error) -> JsError {
    JsError::new(&format!("{error:#}"))
}

fn option<T: ValueEnum>(name: &str) -> Result<T, JsError> {
    T::from_str(name, true).map_err(|message| JsError::new(&format!("{name}: {message}")))
}

fn input(name: &str, data: Vec<u8>) -> Result<Input, JsError> {
    // the message reads "the female chest is not an image"
    Input::from_bytes(format!("the {name}"), data).map_err(error)
}

/// an optional image, where an empty array means that none was given
fn maybe(name: &str, data: Vec<u8>) -> Result<Option<Input>, JsError> {
    if data.is_empty() {
        Ok(None)
    } else {
        input(name, data).map(Some)
    }
}

/// `output_type` is raw, hat, object or floating-object
#[wasm_bindgen]
pub fn sprite(image: Vec<u8>, output_type: &str) -> Result<Output, JsError> {
    let output_type = option::<SpriteOutputType>(output_type)?;
    generator::sprite(input("image", image)?, output_type)
        .map(Output::from)
        .map_err(error)
}

#[wasm_bindgen]
pub fn shirt(
    chest_female: Vec<u8>,
    chest_male: Vec<u8>,
    front_sleeve: Vec<u8>,
    back_sleeve: Vec<u8>,
) -> Result<Output, JsError> {
    generator::shirt(
        maybe("female chest", chest_female)?,
        maybe("male chest", chest_male)?,
        maybe("front sleeve", front_sleeve)?,
        maybe("back sleeve", back_sleeve)?,
    )
    .map(Output::from)
    .map_err(error)
}

#[wasm_bindgen]
pub fn pants(pants: Vec<u8>, hide_body: bool) -> Result<Output, JsError> {
    generator::pants(input("pants", pants)?, hide_body)
        .map(Output::from)
        .map_err(error)
}

#[wasm_bindgen]
pub fn back(back: Vec<u8>) -> Result<Output, JsError> {
    generator::back(input("back", back)?)
        .map(Output::from)
        .map_err(error)
}

/// `body_type` is male or female, `species` is one of the command line's
#[wasm_bindgen]
pub fn humanoid(
    body_type: &str,
    species: &str,
    body: Vec<u8>,
    head: Vec<u8>,
    front_arm: Vec<u8>,
    back_arm: Vec<u8>,
) -> Result<Output, JsError> {
    generator::humanoid(
        option::<BodyType>(body_type)?,
        option::<Species>(species)?,
        maybe("body", body)?,
        maybe("head", head)?,
        maybe("front arm", front_arm)?,
        maybe("back arm", back_arm)?,
    )
    .map(Output::from)
    .map_err(error)
}

/// the emote directives of a 387x344 sheet, not the legacy ones
#[wasm_bindgen]
pub fn emote(sheet: Vec<u8>, species: &str) -> Result<String, JsError> {
    crate::emote::directives(&sheet, option::<Species>(species)?).map_err(error)
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    #[wasm_bindgen(js_name = error)]
    fn console_error(message: &str);
}

/// a panic is a bare "unreachable" trap in the browser without this
#[wasm_bindgen(start)]
fn start() {
    std::panic::set_hook(Box::new(|info| console_error(&info.to_string())));
}
