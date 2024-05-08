pub mod container;
mod globals;
pub mod image;
mod install;
pub mod kube;
mod network;
mod pod;
mod volume;

use std::{
    fmt::{self, Display, Formatter},
    iter,
    path::PathBuf,
    str::FromStr,
};

use clap::ValueEnum;
use serde::{Serialize, Serializer};
use thiserror::Error;

pub use self::{
    container::Container,
    globals::Globals,
    image::Image,
    install::Install,
    kube::Kube,
    network::{IpRange, Network},
    pod::Pod,
    volume::Volume,
};
use crate::cli::{service::Service, unit::Unit};

#[derive(Debug, Clone, PartialEq)]
pub struct File {
    pub name: String,
    pub unit: Option<Unit>,
    pub resource: Resource,
    pub globals: Globals,
    pub service: Option<Service>,
    pub install: Option<Install>,
}

impl Display for File {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        if let Some(unit) = &self.unit {
            writeln!(f, "{unit}")?;
        }

        write!(f, "{}{}", self.resource, self.globals)?;

        if let Some(service) = &self.service {
            write!(f, "\n{service}")?;
        }

        if let Some(install) = &self.install {
            write!(f, "\n{install}")?;
        }

        Ok(())
    }
}

impl File {
    /// Returns the corresponding service file name generated by Quadlet
    pub fn service_name(&self) -> String {
        self.resource.name_to_service(&self.name)
    }
}

impl HostPaths for File {
    fn host_paths(&mut self) -> impl Iterator<Item = &mut PathBuf> {
        self.resource.host_paths().chain(self.globals.host_paths())
    }
}

impl Downgrade for File {
    fn downgrade(&mut self, version: PodmanVersion) -> Result<(), DowngradeError> {
        self.resource.downgrade(version)?;
        self.globals.downgrade(version)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Resource {
    Container(Box<Container>),
    Pod(Pod),
    Kube(Kube),
    Network(Network),
    Volume(Volume),
    Image(Image),
}

impl Display for Resource {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::Container(container) => container.fmt(f),
            Self::Pod(pod) => pod.fmt(f),
            Self::Kube(kube) => kube.fmt(f),
            Self::Network(network) => network.fmt(f),
            Self::Volume(volume) => volume.fmt(f),
            Self::Image(image) => image.fmt(f),
        }
    }
}

impl From<Container> for Resource {
    fn from(value: Container) -> Self {
        Self::Container(Box::new(value))
    }
}

impl From<Box<Container>> for Resource {
    fn from(value: Box<Container>) -> Self {
        Self::Container(value)
    }
}

impl From<Pod> for Resource {
    fn from(value: Pod) -> Self {
        Self::Pod(value)
    }
}

impl From<Kube> for Resource {
    fn from(value: Kube) -> Self {
        Self::Kube(value)
    }
}

impl From<Network> for Resource {
    fn from(value: Network) -> Self {
        Self::Network(value)
    }
}

impl From<Volume> for Resource {
    fn from(value: Volume) -> Self {
        Self::Volume(value)
    }
}

impl From<Image> for Resource {
    fn from(value: Image) -> Self {
        Self::Image(value)
    }
}

impl Resource {
    /// The extension that should be used for the generated file.
    pub fn extension(&self) -> &'static str {
        ResourceKind::from(self).as_str()
    }

    /// Takes a file name (no extension) and returns the corresponding service file name
    /// generated by Quadlet.
    pub fn name_to_service(&self, name: &str) -> String {
        let mut service = match self {
            Self::Container(_) | Self::Kube(_) => String::from(name),
            Self::Pod(_) => format!("{name}-pod"),
            Self::Network(_) => format!("{name}-network"),
            Self::Volume(_) => format!("{name}-volume"),
            Self::Image(_) => format!("{name}-image"),
        };
        service.push_str(".service");
        service
    }
}

impl HostPaths for Resource {
    fn host_paths(&mut self) -> impl Iterator<Item = &mut PathBuf> {
        match self {
            Self::Container(container) => ResourceIter::Container(container.host_paths()),
            Self::Pod(pod) => ResourceIter::Pod(pod.host_paths()),
            Self::Kube(kube) => ResourceIter::Kube(kube.host_paths()),
            Self::Network(_) => ResourceIter::Network(iter::empty()),
            Self::Volume(volume) => ResourceIter::Volume(volume.host_paths()),
            Self::Image(image) => ResourceIter::Image(image.host_paths()),
        }
    }
}

/// [`Iterator`] for all [`Resource`] types.
enum ResourceIter<C, P, K, N, V, I> {
    Container(C),
    Pod(P),
    Kube(K),
    Network(N),
    Volume(V),
    Image(I),
}

impl<C, P, K, N, V, I, Item> Iterator for ResourceIter<C, P, K, N, V, I>
where
    C: Iterator<Item = Item>,
    P: Iterator<Item = Item>,
    K: Iterator<Item = Item>,
    N: Iterator<Item = Item>,
    V: Iterator<Item = Item>,
    I: Iterator<Item = Item>,
{
    type Item = Item;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Container(iter) => iter.next(),
            Self::Pod(iter) => iter.next(),
            Self::Kube(iter) => iter.next(),
            Self::Network(iter) => iter.next(),
            Self::Volume(iter) => iter.next(),
            Self::Image(iter) => iter.next(),
        }
    }
}

impl Downgrade for Resource {
    fn downgrade(&mut self, version: PodmanVersion) -> Result<(), DowngradeError> {
        match self {
            Self::Container(container) => container.downgrade(version),
            Self::Pod(pod) => pod.downgrade(version),
            Self::Kube(kube) => kube.downgrade(version),
            Self::Network(network) => network.downgrade(version),
            Self::Volume(volume) => volume.downgrade(version),
            Self::Image(image) => image.downgrade(version),
        }
    }
}

/// Quadlet [`Resource`] kinds
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind {
    Container,
    Pod,
    Kube,
    Network,
    Volume,
    Image,
}

impl ResourceKind {
    /// Resource kind as a lowercase static string slice.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Container => "container",
            Self::Pod => "pod",
            Self::Kube => "kube",
            Self::Network => "network",
            Self::Volume => "volume",
            Self::Image => "image",
        }
    }
}

impl Display for ResourceKind {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&Resource> for ResourceKind {
    fn from(value: &Resource) -> Self {
        match value {
            Resource::Container(_) => Self::Container,
            Resource::Pod(_) => Self::Pod,
            Resource::Kube(_) => Self::Kube,
            Resource::Network(_) => Self::Network,
            Resource::Volume(_) => Self::Volume,
            Resource::Image(_) => Self::Image,
        }
    }
}

/// Trait for types which have varying levels of compatibility with different [`PodmanVersion`]s.
pub trait Downgrade {
    /// Downgrade Podman compatibility to `version`.
    ///
    /// This is a one-way transformation, calling downgrade a second time with a higher version
    /// will not increase the Quadlet options used.
    ///
    /// # Errors
    ///
    /// Returns an error if the given [`PodmanVersion`] does not support a used Quadlet option or
    /// the type of Quadlet file.
    fn downgrade(&mut self, version: PodmanVersion) -> Result<(), DowngradeError>;
}

/// Versions of Podman since Quadlet was added.
///
/// Each version added new features to Quadlet.
#[non_exhaustive]
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PodmanVersion {
    /// Podman v4.4
    #[value(name = "4.4", aliases = ["4.4.0", "4.4.1", "4.4.2", "4.4.3", "4.4.4"])]
    V4_4,

    /// Podman v4.5
    #[value(name = "4.5", aliases = ["4.5.0", "4.5.1"])]
    V4_5,

    /// Podman v4.6
    #[value(name = "4.6", aliases = ["4.6.0", "4.6.1", "4.6.2"])]
    V4_6,

    /// Podman v4.7
    #[value(name = "4.7", aliases = ["4.7.0", "4.7.1", "4.7.2"])]
    V4_7,

    /// Podman v4.8 and v4.9
    #[value(
        name = "4.8",
        aliases = ["4.8.0", "4.8.1", "4.8.2", "4.8.3", "4.9", "4.9.0", "4.9.1", "4.9.2", "4.9.3", "4.9.4"]
    )]
    V4_8,

    /// Podman v5.0
    #[value(name = "5.0", aliases = ["latest", "5.0.0", "5.0.1", "5.0.2"])]
    V5_0,
}

impl PodmanVersion {
    /// Latest supported version of Podman with regards to Quadlet.
    pub const LATEST: Self = Self::V5_0;

    /// Podman version as a static string slice.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V4_4 => "4.4",
            Self::V4_5 => "4.5",
            Self::V4_6 => "4.6",
            Self::V4_7 => "4.7",
            Self::V4_8 => "4.8",
            Self::V5_0 => "5.0",
        }
    }
}

impl Default for PodmanVersion {
    fn default() -> Self {
        Self::LATEST
    }
}

impl Display for PodmanVersion {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Error returned when [downgrading](Downgrade::downgrade()) a Quadlet file fails.
#[derive(Error, Debug)]
pub enum DowngradeError {
    /// Unsupported Quadlet option used
    #[error(
        "Quadlet option `{quadlet_option}={value}` was not \
        supported until Podman v{supported_version}"
    )]
    Option {
        quadlet_option: &'static str,
        value: String,
        supported_version: PodmanVersion,
    },
    /// Unsupported Quadlet kind
    #[error("`.{kind}` Quadlet files were not supported until Podman v{supported_version}")]
    Kind {
        kind: ResourceKind,
        supported_version: PodmanVersion,
    },
}

/// Valid values for the `AutoUpdate=` Quadlet option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoUpdate {
    Registry,
    Local,
}

impl AutoUpdate {
    /// Podman-specific label for `podman auto-update`.
    ///
    /// See <https://docs.podman.io/en/stable/markdown/podman-auto-update.1.html>
    const LABEL_KEY: &'static str = "io.containers.autoupdate";

    /// Extracts all valid values of the `io.containers.autoupdate` label from `labels`,
    /// the last value of which is parsed into an [`AutoUpdate`].
    ///
    /// Returns `None` if no valid `io.containers.autoupdate` label is found.
    ///
    /// `io.containers.autoupdate` labels with invalid values are retained in `labels`.
    pub fn extract_from_labels(labels: &mut Vec<String>) -> Option<Self> {
        let mut auto_update = None;
        labels.retain(|label| {
            label
                .strip_prefix(Self::LABEL_KEY)
                .and_then(|label| label.strip_prefix('='))
                .and_then(|value| value.parse().ok())
                .map_or(true, |value| {
                    auto_update = Some(value);
                    false
                })
        });

        auto_update
    }
}

impl AsRef<str> for AutoUpdate {
    fn as_ref(&self) -> &str {
        match self {
            Self::Registry => "registry",
            Self::Local => "local",
        }
    }
}

impl Display for AutoUpdate {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.write_str(self.as_ref())
    }
}

impl Serialize for AutoUpdate {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_ref())
    }
}

impl FromStr for AutoUpdate {
    type Err = ParseAutoUpdateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "registry" => Ok(Self::Registry),
            "local" => Ok(Self::Local),
            s => Err(ParseAutoUpdateError(s.into())),
        }
    }
}

/// Error returned when attempting to parse an invalid [`AutoUpdate`] variant,
/// see [`AutoUpdate::from_str()`].
#[derive(Debug, Error)]
#[error("unknown auto update variant `{0}`, must be `registry` or `local`")]
pub struct ParseAutoUpdateError(String);

/// Trait for types which contain paths on the host.
pub trait HostPaths {
    /// Retrieve an [`Iterator`] over mutable references to all [`PathBuf`]s that represent paths
    /// on the host.
    fn host_paths(&mut self) -> impl Iterator<Item = &mut PathBuf>;
}
