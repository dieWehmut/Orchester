//! Versioned, provider-neutral tool registration.
//!
//! A model only receives a [`ToolCatalog`].  A later tool call must bind to
//! the same registry generation and schema version before any backend can see
//! its arguments.  This keeps a refreshed tool implementation from silently
//! accepting a call advertised under an older contract.

use std::collections::BTreeMap;
use std::fmt;

use blake3::Hasher;
use orchester_modell::ToolDefinition;
use serde_json::Value;
use thiserror::Error;

const MAX_TOOL_NAME_BYTES: usize = 64;
const MAX_DESCRIPTION_BYTES: usize = 2 * 1024;
const MAX_SCHEMA_BYTES: usize = 64 * 1024;
const MAX_ARGUMENTS_BYTES: usize = 64 * 1024;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ToolRegistryError {
    #[error("tool definition is invalid")]
    InvalidDefinition,
    #[error("tool name is already registered")]
    DuplicateTool,
    #[error("tool is not registered")]
    UnknownTool,
    #[error("tool schema version does not match the registry")]
    SchemaVersionMismatch,
    #[error("tool call belongs to a different registry generation")]
    GenerationMismatch,
    #[error("tool arguments must be a JSON object")]
    InvalidArguments,
    #[error("tool arguments exceed the size limit")]
    ArgumentsTooLarge,
}

/// A validated tool definition with an explicit schema version.
pub struct ToolSchema {
    name: String,
    version: u16,
    definition: ToolDefinition,
    schema_bytes: Vec<u8>,
}

impl ToolSchema {
    pub fn new(
        name: impl Into<String>,
        version: u16,
        definition: ToolDefinition,
    ) -> Result<Self, ToolRegistryError> {
        let name = name.into();
        if version == 0
            || name.is_empty()
            || name.len() > MAX_TOOL_NAME_BYTES
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
            || definition.name != name
            || definition.description.is_empty()
            || definition.description.len() > MAX_DESCRIPTION_BYTES
            || contains_control(&definition.description)
        {
            return Err(ToolRegistryError::InvalidDefinition);
        }
        let schema_bytes = serde_json::to_vec(&definition.parameters)
            .map_err(|_| ToolRegistryError::InvalidDefinition)?;
        if schema_bytes.len() > MAX_SCHEMA_BYTES || !definition.parameters.is_object() {
            return Err(ToolRegistryError::InvalidDefinition);
        }
        Ok(Self {
            name,
            version,
            definition,
            schema_bytes,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version(&self) -> u16 {
        self.version
    }

    pub fn definition(&self) -> &ToolDefinition {
        &self.definition
    }
}

impl fmt::Debug for ToolSchema {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ToolSchema")
            .field("name", &self.name)
            .field("version", &self.version)
            .field("description_bytes", &self.definition.description.len())
            .field("schema_bytes", &self.schema_bytes.len())
            .finish()
    }
}

/// The exact tool list advertised to a model request.
#[derive(Clone)]
pub struct ToolCatalog {
    generation: String,
    definitions: Vec<ToolDefinition>,
    versions: BTreeMap<String, u16>,
}

impl ToolCatalog {
    pub fn generation(&self) -> &str {
        &self.generation
    }

    pub fn definitions(&self) -> &[ToolDefinition] {
        &self.definitions
    }

    pub fn schema_version(&self, name: &str) -> Option<u16> {
        self.versions.get(name).copied()
    }
}

impl fmt::Debug for ToolCatalog {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ToolCatalog")
            .field("generation", &self.generation)
            .field("tool_count", &self.definitions.len())
            .finish()
    }
}

/// Registry used to advertise and bind calls for one model request family.
pub struct ToolRegistry {
    generation: String,
    tools: BTreeMap<String, ToolSchema>,
}

impl ToolRegistry {
    pub fn new(schemas: Vec<ToolSchema>) -> Result<Self, ToolRegistryError> {
        let mut tools = BTreeMap::new();
        for schema in schemas {
            if tools.insert(schema.name.clone(), schema).is_some() {
                return Err(ToolRegistryError::DuplicateTool);
            }
        }
        let generation = generation_hash(&tools);
        Ok(Self { generation, tools })
    }

    pub fn generation(&self) -> &str {
        &self.generation
    }

    pub fn catalog(&self) -> ToolCatalog {
        ToolCatalog {
            generation: self.generation.clone(),
            definitions: self
                .tools
                .values()
                .map(|schema| schema.definition.clone())
                .collect(),
            versions: self
                .tools
                .values()
                .map(|schema| (schema.name.clone(), schema.version))
                .collect(),
        }
    }

    pub fn replace(&mut self, schemas: Vec<ToolSchema>) -> Result<(), ToolRegistryError> {
        let replacement = Self::new(schemas)?;
        self.tools = replacement.tools;
        self.generation = replacement.generation;
        Ok(())
    }

    pub fn bind(
        &self,
        expected_generation: &str,
        name: &str,
        version: u16,
        arguments_json: &str,
    ) -> Result<BoundToolCall, ToolRegistryError> {
        if expected_generation != self.generation {
            return Err(ToolRegistryError::GenerationMismatch);
        }
        let schema = self.tools.get(name).ok_or(ToolRegistryError::UnknownTool)?;
        if schema.version != version {
            return Err(ToolRegistryError::SchemaVersionMismatch);
        }
        if arguments_json.len() > MAX_ARGUMENTS_BYTES {
            return Err(ToolRegistryError::ArgumentsTooLarge);
        }
        let arguments = serde_json::from_str::<Value>(arguments_json)
            .map_err(|_| ToolRegistryError::InvalidArguments)?;
        if !arguments.is_object() {
            return Err(ToolRegistryError::InvalidArguments);
        }
        let mut canonical = Vec::new();
        write_canonical_json(&arguments, &mut canonical);
        let mut hasher = Hasher::new();
        hasher.update(&canonical);
        let arguments_hash = hex(hasher.finalize().as_bytes());
        Ok(BoundToolCall {
            name: schema.name.clone(),
            version,
            generation: self.generation.clone(),
            arguments,
            arguments_hash,
        })
    }
}

impl fmt::Debug for ToolRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ToolRegistry")
            .field("generation", &self.generation)
            .field("tool_count", &self.tools.len())
            .finish()
    }
}

/// A structurally decoded call tied to the registry generation that accepted it.
#[derive(Clone)]
pub struct BoundToolCall {
    name: String,
    version: u16,
    generation: String,
    arguments: Value,
    arguments_hash: String,
}

impl BoundToolCall {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn schema_version(&self) -> u16 {
        self.version
    }

    pub fn registry_generation(&self) -> &str {
        &self.generation
    }

    pub fn arguments(&self) -> &Value {
        &self.arguments
    }

    pub fn arguments_hash(&self) -> &str {
        &self.arguments_hash
    }

    pub fn is_current(&self, registry: &ToolRegistry) -> bool {
        self.generation == registry.generation
            && registry
                .tools
                .get(&self.name)
                .is_some_and(|schema| schema.version == self.version)
    }
}

impl fmt::Debug for BoundToolCall {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BoundToolCall")
            .field("name", &self.name)
            .field("version", &self.version)
            .field("generation", &self.generation)
            .field("arguments_bytes", &self.arguments.to_string().len())
            .field("arguments_hash", &self.arguments_hash)
            .finish()
    }
}

fn generation_hash(tools: &BTreeMap<String, ToolSchema>) -> String {
    let mut hasher = Hasher::new();
    hasher.update(b"orchester-tool-registry-v1\0");
    for schema in tools.values() {
        hash_field(&mut hasher, schema.name.as_bytes());
        hasher.update(&schema.version.to_be_bytes());
        hash_field(&mut hasher, schema.definition.description.as_bytes());
        hash_field(&mut hasher, &schema.schema_bytes);
    }
    hex(hasher.finalize().as_bytes())
}

fn hash_field(hasher: &mut Hasher, value: &[u8]) {
    hasher.update(&(value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn write_canonical_json(value: &Value, output: &mut Vec<u8>) {
    match value {
        Value::Null => output.extend_from_slice(b"null"),
        Value::Bool(value) => output.extend_from_slice(if *value { b"true" } else { b"false" }),
        Value::Number(value) => output.extend_from_slice(value.to_string().as_bytes()),
        Value::String(value) => {
            output.extend_from_slice(serde_json::to_string(value).unwrap_or_default().as_bytes())
        }
        Value::Array(values) => {
            output.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                write_canonical_json(value, output);
            }
            output.push(b']');
        }
        Value::Object(values) => {
            output.push(b'{');
            let sorted = values.iter().collect::<BTreeMap<_, _>>();
            for (index, (key, value)) in sorted.into_iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                output.extend_from_slice(serde_json::to_string(key).unwrap_or_default().as_bytes());
                output.push(b':');
                write_canonical_json(value, output);
            }
            output.push(b'}');
        }
    }
}

fn contains_control(value: &str) -> bool {
    value.chars().any(|character| character.is_control())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
