// SPDX-License-Identifier: GPL-3.0
use super::{
	sourcing,
	sourcing::{
		traits::{Source as _, *},
		GitHub::ReleaseArchive,
		Source,
	},
	target, Binary, Error,
};
use std::{iter::once, path::Path};
use strum::VariantArray as _;
use strum_macros::{EnumProperty, VariantArray};

/// A supported relay chain.
#[derive(Debug, EnumProperty, PartialEq, VariantArray)]
pub(super) enum RelayChain {
	/// Polkadot.
	#[strum(props(
		Repository = "https://github.com/r0gue-io/polkadot",
		Binary = "polkadot",
		TagFormat = "polkadot-{tag}",
		Fallback = "v1.12.0"
	))]
	Polkadot,
}

impl TryInto for &RelayChain {
	/// Attempt the conversion.
	///
	/// # Arguments
	/// * `tag` - If applicable, a tag used to determine a specific release.
	/// * `latest` - If applicable, some specifier used to determine the latest source.
	fn try_into(&self, tag: Option<String>, latest: Option<String>) -> Result<Source, Error> {
		Ok(match self {
			RelayChain::Polkadot => {
				// Source from GitHub release asset
				let repo = crate::GitHub::parse(self.repository())?;
				Source::GitHub(ReleaseArchive {
					owner: repo.org,
					repository: repo.name,
					tag,
					tag_format: self.tag_format().map(|t| t.into()),
					archive: format!("{}-{}.tar.gz", self.binary(), target()?),
					contents: once(self.binary()).chain(self.workers()).collect(),
					latest,
				})
			},
		})
	}
}

impl RelayChain {
	/// The additional worker binaries required for the relay chain.
	fn workers(&self) -> [&'static str; 2] {
		["polkadot-execute-worker", "polkadot-prepare-worker"]
	}
}

impl sourcing::traits::Source for RelayChain {}

/// Initialises the configuration required to launch the relay chain.
///
/// # Arguments
/// * `version` - The version of the relay chain binary to be used.
/// * `cache` - The cache to be used.
pub(super) async fn default(
	version: Option<&str>,
	cache: &Path,
) -> Result<super::RelayChain, Error> {
	from(RelayChain::Polkadot.binary(), version, cache).await
}

/// Initialises the configuration required to launch the relay chain using the specified command.
///
/// # Arguments
/// * `command` - The command specified.
/// * `version` - The version of the binary to be used.
/// * `cache` - The cache to be used.
pub(super) async fn from(
	command: &str,
	version: Option<&str>,
	cache: &Path,
) -> Result<super::RelayChain, Error> {
	for relay in RelayChain::VARIANTS
		.iter()
		.filter(|r| command.to_lowercase().ends_with(r.binary()))
	{
		let name = relay.binary();
		let releases = relay.releases().await?;
		let tag = Binary::resolve_version(name, version, &releases, cache);
		// Only set latest when caller has not explicitly specified a version to use
		let latest = version
			.is_none()
			.then(|| releases.iter().nth(0).map(|v| v.to_string()))
			.flatten();
		let binary = Binary::Source {
			name: name.to_string(),
			source: TryInto::try_into(&relay, tag, latest)?,
			cache: cache.to_path_buf(),
		};
		return Ok(super::RelayChain { binary, workers: relay.workers() });
	}
	return Err(Error::UnsupportedCommand(format!(
		"the relay chain command is unsupported: {command}",
	)));
}

#[cfg(test)]
mod tests {
	use super::*;
	use tempfile::tempdir;

	#[tokio::test]
	async fn default_works() -> anyhow::Result<()> {
		let expected = RelayChain::Polkadot;
		let version = "v1.12.0";
		let temp_dir = tempdir()?;
		let relay = default(Some(version), temp_dir.path()).await?;
		assert!(matches!(relay.binary, Binary::Source { name, source, cache }
			if name == expected.binary() && source == Source::GitHub(ReleaseArchive {
					owner: "r0gue-io".to_string(),
					repository: "polkadot".to_string(),
					tag: Some(version.to_string()),
					tag_format: Some("polkadot-{tag}".to_string()),
					archive: format!("{name}-{}.tar.gz", target()?),
					contents: vec!["polkadot", "polkadot-execute-worker", "polkadot-prepare-worker"],
					latest: relay.binary.latest().map(|l| l.to_string()),
				}) && cache == temp_dir.path()
		));
		assert_eq!(relay.workers, expected.workers());
		Ok(())
	}

	#[tokio::test]
	async fn from_handles_unsupported_command() -> anyhow::Result<()> {
		assert!(
			matches!(from("none", None, tempdir()?.path()).await, Err(Error::UnsupportedCommand(e))
			if e == "the relay chain command is unsupported: none")
		);
		Ok(())
	}

	#[tokio::test]
	async fn from_handles_local_command() -> anyhow::Result<()> {
		let expected = RelayChain::Polkadot;
		let version = "v1.12.0";
		let temp_dir = tempdir()?;
		let relay = from("./bin-v1.6.0/polkadot", Some(version), temp_dir.path()).await?;
		assert!(matches!(relay.binary, Binary::Source { name, source, cache }
			if name == expected.binary() && source == Source::GitHub(ReleaseArchive {
					owner: "r0gue-io".to_string(),
					repository: "polkadot".to_string(),
					tag: Some(version.to_string()),
					tag_format: Some("polkadot-{tag}".to_string()),
					archive: format!("{name}-{}.tar.gz", target()?),
					contents: vec!["polkadot", "polkadot-execute-worker", "polkadot-prepare-worker"],
					latest: relay.binary.latest().map(|l| l.to_string()),
				}) && cache == temp_dir.path()
		));
		assert_eq!(relay.workers, expected.workers());
		Ok(())
	}
}
