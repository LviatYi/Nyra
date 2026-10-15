use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    ecs::error::BevyError,
    prelude::*,
};
use serde::de::DeserializeOwned;
use std::marker::PhantomData;

#[derive(TypePath)]
pub(crate) struct JsonConfigLoader<T>(PhantomData<T>);

impl<T> Default for JsonConfigLoader<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: Asset + DeserializeOwned> AssetLoader for JsonConfigLoader<T> {
    type Asset = T;
    type Settings = ();
    type Error = BevyError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn extensions(&self) -> &[&str] {
        &["json"]
    }
}
