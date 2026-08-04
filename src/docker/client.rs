use std::{collections::HashMap, path::Path, time::Duration};

use anyhow::{Context, Result};
use bollard::{
    container::LogOutput,
    models::{
        ContainerInspectResponse, ContainerStatsResponse, ContainerSummary, EventMessage,
        ImageSummary, Network, Volume,
    },
    query_parameters::{
        EventsOptionsBuilder, ListContainersOptionsBuilder, ListImagesOptionsBuilder,
        ListNetworksOptionsBuilder, ListVolumesOptions, LogsOptionsBuilder, StatsOptionsBuilder,
    },
    Docker, API_DEFAULT_VERSION,
};
use futures_util::Stream;

#[derive(Clone)]
pub struct DockerClient {
    pub(crate) inner: Docker,
    pub socket: String,
}

impl DockerClient {
    pub fn connect(socket: impl Into<String>) -> Result<Self> {
        let socket = socket.into();
        if !Path::new(&socket).is_absolute() {
            anyhow::bail!("Docker socket must be an absolute Unix path");
        }
        let inner = Docker::connect_with_unix(&socket, 10, API_DEFAULT_VERSION)
            .context("create Docker Unix socket client")?;
        Ok(Self { inner, socket })
    }

    pub async fn ping(&self) -> Result<()> {
        self.inner.ping().await.context("ping Docker daemon")?;
        Ok(())
    }

    pub async fn containers(&self, all: bool) -> Result<Vec<ContainerSummary>> {
        let options = ListContainersOptionsBuilder::default().all(all).build();
        self.inner.list_containers(Some(options)).await.context("list Docker containers")
    }

    pub async fn stats(&self, id: &str) -> Result<Option<ContainerStatsResponse>> {
        let options = StatsOptionsBuilder::default().stream(false).one_shot(true).build();
        let mut stream = self.inner.stats(id, Some(options));
        match futures_util::StreamExt::next(&mut stream).await {
            Some(result) => Ok(Some(result.context("read container stats")?)),
            None => Ok(None),
        }
    }

    pub async fn inspect(&self, id: &str) -> Result<ContainerInspectResponse> {
        self.inner.inspect_container(id, None).await.context("inspect container")
    }
    pub async fn images(&self) -> Result<Vec<ImageSummary>> {
        let options = ListImagesOptionsBuilder::default().all(true).build();
        self.inner.list_images(Some(options)).await.context("list Docker images")
    }
    pub async fn volumes(&self) -> Result<Vec<Volume>> {
        Ok(self
            .inner
            .list_volumes(None::<ListVolumesOptions>)
            .await
            .context("list Docker volumes")?
            .volumes
            .unwrap_or_default())
    }
    pub async fn networks(&self) -> Result<Vec<Network>> {
        let options = ListNetworksOptionsBuilder::default().build();
        self.inner.list_networks(Some(options)).await.context("list Docker networks")
    }

    pub async fn action(&self, id: &str, action: crate::action::ContainerAction) -> Result<()> {
        match action {
            crate::action::ContainerAction::Start => self.inner.start_container(id, None).await?,
            crate::action::ContainerAction::Stop => self.inner.stop_container(id, None).await?,
            crate::action::ContainerAction::Restart => {
                self.inner.restart_container(id, None).await?
            }
            crate::action::ContainerAction::Pause => self.inner.pause_container(id).await?,
            crate::action::ContainerAction::Unpause => self.inner.unpause_container(id).await?,
        }
        Ok(())
    }

    pub fn logs(
        &self,
        id: &str,
        follow: bool,
    ) -> impl Stream<Item = Result<LogOutput, bollard::errors::Error>> + '_ {
        let options = LogsOptionsBuilder::default()
            .follow(follow)
            .stdout(true)
            .stderr(true)
            .timestamps(true)
            .tail("500")
            .build();
        self.inner.logs(id, Some(options))
    }

    pub fn events(&self) -> impl Stream<Item = Result<EventMessage, bollard::errors::Error>> + '_ {
        let mut filters = HashMap::new();
        filters.insert("type".to_owned(), vec!["container".to_owned()]);
        let options = EventsOptionsBuilder::default().filters(&filters).build();
        self.inner.events(Some(options))
    }

    pub fn timeout(&self) -> Duration {
        self.inner.timeout()
    }
}
