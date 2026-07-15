use std::sync::OnceLock;

use regex::Regex;
use serde::Deserialize;

use super::PluginError;

const DESCRIPTION: &str = "Pure-data agent adapter plugin for Orchester";
const LICENSE: &str = "MIT OR Apache-2.0";
const REPOSITORY: &str = "https://github.com/dieWehmut/Orchester";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DescriptorWire {
    schema_version: u32,
    name: String,
    display_name: String,
    package_name: String,
    version: String,
    adapter_manifest: String,
    command: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PackageWire {
    name: String,
    version: String,
    description: String,
    license: String,
    repository: String,
    files: Vec<String>,
    publish_config: PublishConfigWire,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PublishConfigWire {
    access: String,
}

pub(super) struct ValidatedDescriptor {
    pub(super) name: String,
    pub(super) display_name: String,
    pub(super) package_name: String,
    pub(super) version: String,
    pub(super) command: String,
}

pub(super) fn validate(
    package_json: &str,
    descriptor_json: &str,
) -> Result<ValidatedDescriptor, PluginError> {
    let package: PackageWire =
        serde_json::from_str(package_json).map_err(|_| PluginError::InvalidPackage)?;
    validate_package(&package)?;
    let descriptor: DescriptorWire =
        serde_json::from_str(descriptor_json).map_err(|_| PluginError::InvalidDescriptor)?;
    validate_descriptor(&package, descriptor)
}

fn validate_package(package: &PackageWire) -> Result<(), PluginError> {
    if !package.name.starts_with("@orchester/")
        || !valid_name(&package.name["@orchester/".len()..])
        || !valid_version(&package.version)
        || package.description != DESCRIPTION
        || package.license != LICENSE
        || package.repository != REPOSITORY
        || package.files != ["orchester-plugin.json", "manifests"]
        || package.publish_config.access != "public"
    {
        return Err(PluginError::InvalidPackage);
    }
    Ok(())
}

fn validate_descriptor(
    package: &PackageWire,
    descriptor: DescriptorWire,
) -> Result<ValidatedDescriptor, PluginError> {
    let expected_name = package
        .name
        .strip_prefix("@orchester/")
        .ok_or(PluginError::InvalidDescriptor)?;
    let expected_manifest = format!("manifests/{expected_name}.toml");
    if descriptor.schema_version != 1
        || descriptor.name != expected_name
        || descriptor.package_name != package.name
        || descriptor.version != package.version
        || descriptor.adapter_manifest != expected_manifest
        || !valid_display_name(&descriptor.display_name)
        || !command_pattern().is_match(&descriptor.command)
    {
        return Err(PluginError::InvalidDescriptor);
    }
    Ok(ValidatedDescriptor {
        name: descriptor.name,
        display_name: descriptor.display_name,
        package_name: descriptor.package_name,
        version: descriptor.version,
        command: descriptor.command,
    })
}

fn valid_name(value: &str) -> bool {
    name_pattern().is_match(value)
}

fn valid_version(value: &str) -> bool {
    version_pattern().is_match(value)
}

fn valid_display_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && !value.chars().any(char::is_control)
        && !format_character_pattern().is_match(value)
}

fn name_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"^[a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?$").expect("static plugin name pattern")
    })
}

fn command_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$").expect("static plugin command pattern")
    })
}

fn version_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"^(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)(?:-[0-9A-Za-z.-]+)?$")
            .expect("static plugin version pattern")
    })
}

fn format_character_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"\p{Cf}").expect("static Unicode format pattern"))
}
