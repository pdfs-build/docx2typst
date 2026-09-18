use roxmltree::Document as XmlDocument;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Cursor, Read, Seek};
use std::path::{Path, PathBuf};
use zip::ZipArchive;

pub type Result<T> = std::result::Result<T, OpcError>;

#[derive(Debug, thiserror::Error)]
pub enum OpcError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("xml error in {part}: {message}")]
    Xml { part: String, message: String },
    #[error("missing OPC part: {0}")]
    MissingPart(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Part {
    pub path: String,
    pub checksum_sha256: String,
    pub content_type: Option<String>,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Relationship {
    pub id: String,
    pub kind: String,
    pub target: String,
    pub target_mode: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Relationships(pub Vec<Relationship>);

impl Relationships {
    pub fn find(&self, id: &str) -> Option<&Relationship> {
        self.0.iter().find(|rel| rel.id == id)
    }
}

#[derive(Debug, Clone, Default)]
pub struct OpcPackage {
    parts: BTreeMap<String, Part>,
    content_types: BTreeMap<String, String>,
}

impl OpcPackage {
    pub fn open_path(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::open(path)?;
        Self::from_reader(file)
    }

    pub fn open_bytes(bytes: Vec<u8>) -> Result<Self> {
        Self::from_reader(Cursor::new(bytes))
    }

    pub fn from_reader<R: Read + Seek>(reader: R) -> Result<Self> {
        let mut archive = ZipArchive::new(reader)?;
        let mut raw_parts = BTreeMap::new();
        for index in 0..archive.len() {
            let mut file = archive.by_index(index)?;
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            let normalized = normalize_part_name(file.name());
            let checksum_sha256 = hex_sha256(&bytes);
            raw_parts.insert(
                normalized.clone(),
                Part {
                    path: normalized,
                    checksum_sha256,
                    content_type: None,
                    bytes,
                },
            );
        }

        let mut package = Self {
            parts: raw_parts,
            content_types: BTreeMap::new(),
        };
        package.load_content_types()?;
        Ok(package)
    }

    pub fn part_names(&self) -> Vec<String> {
        self.parts.keys().cloned().collect()
    }

    pub fn part(&self, path: &str) -> Option<&Part> {
        self.parts.get(&normalize_part_name(path))
    }

    pub fn require_part(&self, path: &str) -> Result<&Part> {
        self.part(path)
            .ok_or_else(|| OpcError::MissingPart(normalize_part_name(path)))
    }

    pub fn part_bytes(&self, path: &str) -> Result<&[u8]> {
        Ok(&self.require_part(path)?.bytes)
    }

    pub fn part_string(&self, path: &str) -> Result<String> {
        let bytes = self.part_bytes(path)?;
        Ok(String::from_utf8_lossy(bytes).into_owned())
    }

    pub fn xml(&self, path: &str) -> Result<XmlDocument<'static>> {
        let xml = self.part_string(path)?;
        let leaked: &'static str = Box::leak(xml.into_boxed_str());
        XmlDocument::parse(leaked).map_err(|error| OpcError::Xml {
            part: normalize_part_name(path),
            message: error.to_string(),
        })
    }

    pub fn content_type(&self, path: &str) -> Option<&str> {
        self.content_types
            .get(&normalize_part_name(path))
            .map(String::as_str)
            .or_else(|| {
                Path::new(path)
                    .extension()
                    .and_then(|ext| {
                        self.content_types
                            .get(&format!(".{}", ext.to_string_lossy()))
                    })
                    .map(String::as_str)
            })
    }

    pub fn relationships_for(&self, source_part: Option<&str>) -> Result<Relationships> {
        let rels_path = match source_part {
            None => "_rels/.rels".to_string(),
            Some(path) => relationships_part_for(path),
        };

        let Some(part) = self.part(&rels_path) else {
            return Ok(Relationships::default());
        };
        let xml = String::from_utf8_lossy(&part.bytes);
        let doc = XmlDocument::parse(&xml).map_err(|error| OpcError::Xml {
            part: rels_path.clone(),
            message: error.to_string(),
        })?;

        let relationships = doc
            .descendants()
            .filter(|node| node.is_element() && node.tag_name().name() == "Relationship")
            .map(|node| Relationship {
                id: node.attribute("Id").unwrap_or_default().to_string(),
                kind: node.attribute("Type").unwrap_or_default().to_string(),
                target: node.attribute("Target").unwrap_or_default().to_string(),
                target_mode: node.attribute("TargetMode").map(str::to_string),
            })
            .collect();

        Ok(Relationships(relationships))
    }

    pub fn resolve_relationship_target(&self, source_part: &str, rel: &Relationship) -> String {
        if rel.target_mode.as_deref() == Some("External") {
            return rel.target.clone();
        }

        let base = Path::new(source_part)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        normalize_path(base.join(&rel.target))
    }

    fn load_content_types(&mut self) -> Result<()> {
        let Some(bytes) = self
            .parts
            .get("[Content_Types].xml")
            .map(|part| part.bytes.clone())
        else {
            return Ok(());
        };

        let xml = String::from_utf8_lossy(&bytes);
        let doc = XmlDocument::parse(&xml).map_err(|error| OpcError::Xml {
            part: "[Content_Types].xml".to_string(),
            message: error.to_string(),
        })?;

        for node in doc.descendants().filter(|node| node.is_element()) {
            match node.tag_name().name() {
                "Default" => {
                    if let (Some(ext), Some(content_type)) =
                        (node.attribute("Extension"), node.attribute("ContentType"))
                    {
                        self.content_types
                            .insert(format!(".{}", ext), content_type.to_string());
                    }
                }
                "Override" => {
                    if let (Some(name), Some(content_type)) =
                        (node.attribute("PartName"), node.attribute("ContentType"))
                    {
                        let normalized = normalize_part_name(name.trim_start_matches('/'));
                        self.content_types
                            .insert(normalized.clone(), content_type.to_string());
                        if let Some(part) = self.parts.get_mut(&normalized) {
                            part.content_type = Some(content_type.to_string());
                        }
                    }
                }
                _ => {}
            }
        }

        let extension_defaults = self.content_types.clone();
        for (path, part) in &mut self.parts {
            if part.content_type.is_none() {
                part.content_type = extension_defaults.get(path).cloned().or_else(|| {
                    Path::new(path)
                        .extension()
                        .and_then(|ext| {
                            extension_defaults.get(&format!(".{}", ext.to_string_lossy()))
                        })
                        .cloned()
                });
            }
        }
        Ok(())
    }
}

pub fn relationships_part_for(source_part: &str) -> String {
    let path = normalize_part_name(source_part);
    let source = Path::new(&path);
    let file_name = source.file_name().unwrap_or_default().to_string_lossy();
    let parent = source.parent().map(Path::to_path_buf).unwrap_or_default();
    normalize_path(parent.join("_rels").join(format!("{file_name}.rels")))
}

pub fn normalize_part_name(path: impl AsRef<str>) -> String {
    normalize_path(PathBuf::from(path.as_ref().trim_start_matches('/')))
}

fn normalize_path(path: PathBuf) -> String {
    let mut out = Vec::new();
    for component in path.components() {
        use std::path::Component;
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = out.pop();
            }
            Component::Normal(part) => out.push(part.to_string_lossy().into_owned()),
            Component::RootDir | Component::Prefix(_) => {}
        }
    }
    out.join("/")
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions as FileOptions;

    fn minimal_docx() -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buffer);
            let options = FileOptions::default();
            zip.start_file("[Content_Types].xml", options).unwrap();
            write!(
                zip,
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#
            )
            .unwrap();
            zip.start_file("_rels/.rels", options).unwrap();
            write!(
                zip,
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#
            )
            .unwrap();
            zip.start_file("word/document.xml", options).unwrap();
            write!(zip, r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body/></w:document>"#).unwrap();
            zip.finish().unwrap();
        }
        buffer.into_inner()
    }

    #[test]
    fn opens_minimal_package() {
        let package = OpcPackage::open_bytes(minimal_docx()).unwrap();
        assert!(package.part("word/document.xml").is_some());
        assert_eq!(
            package.content_type("word/document.xml"),
            Some(
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"
            )
        );
        let rels = package.relationships_for(None).unwrap();
        assert_eq!(rels.0.len(), 1);
        assert_eq!(
            package.resolve_relationship_target("", rels.find("rId1").unwrap()),
            "word/document.xml"
        );
    }
}
