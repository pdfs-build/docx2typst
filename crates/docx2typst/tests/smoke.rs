use base64::Engine as _;
use docx2typst::{
    ConversionOptions, ConversionProfile, ValidationOptions, convert_bytes, inspect_bytes,
    validate_result,
};
use std::fs;
use std::io::{Cursor, Write};
use std::path::PathBuf;
use zip::write::SimpleFileOptions as FileOptions;

fn sample_docx() -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    let options = FileOptions::default();
    let mut zip = zip::ZipWriter::new(&mut buffer);

    zip.start_file("[Content_Types].xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
  <Override PartName="/word/fontTable.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.fontTable+xml"/>
  <Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/>
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
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    <w:p>
      <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
      <w:r><w:t>Quarterly Review</w:t></w:r>
    </w:p>
    <w:p>
      <w:r><w:t>See </w:t></w:r>
      <w:hyperlink r:id="rIdHyper">
        <w:r><w:t>the appendix</w:t></w:r>
      </w:hyperlink>
      <w:r><w:t> for details</w:t></w:r>
      <w:r><w:footnoteReference w:id="1"/></w:r>
    </w:p>
    <w:p>
      <w:r>
        <w:rPr><w:color w:val="222222"/></w:rPr>
        <w:t>https://example.com/raw</w:t>
      </w:r>
    </w:p>
    <w:sectPr>
      <w:pgSz w:w="12240" w:h="15840"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/>
    </w:sectPr>
  </w:body>
</w:document>"#
    )
    .unwrap();

    zip.start_file("word/_rels/document.xml.rels", options)
        .unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdHyper" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/appendix" TargetMode="External"/>
</Relationships>"#
    )
    .unwrap();

    zip.start_file("word/styles.xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:docDefaults>
    <w:rPrDefault><w:rPr><w:rFonts w:ascii="Libertinus Serif"/><w:sz w:val="22"/></w:rPr></w:rPrDefault>
  </w:docDefaults>
  <w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="Heading 1"/>
    <w:rPr><w:b/><w:sz w:val="32"/></w:rPr>
  </w:style>
</w:styles>"#
    )
    .unwrap();

    zip.start_file("word/fontTable.xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:fonts xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:font w:name="Libertinus Serif"/>
</w:fonts>"#
    )
    .unwrap();

    zip.start_file("word/footnotes.xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:footnote w:id="1">
    <w:p><w:r><w:t>Generated from DOCX.</w:t></w:r></w:p>
  </w:footnote>
</w:footnotes>"#
    )
    .unwrap();

    zip.finish().unwrap();
    buffer.into_inner()
}

fn tiny_png() -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAAAAAA6fptVAAAACklEQVR4nGNgAAAAAgABSK+kcQAAAABJRU5ErkJggg==")
        .unwrap()
}

fn fake_sfnt(fs_type: u16) -> Vec<u8> {
    let mut bytes = vec![
        0x00, 0x01, 0x00, 0x00, // scaler type
        0x00, 0x01, // numTables
        0x00, 0x10, // searchRange
        0x00, 0x00, // entrySelector
        0x00, 0x10, // rangeShift
        b'O', b'S', b'/', b'2', // tag
        0x00, 0x00, 0x00, 0x00, // checksum
        0x00, 0x00, 0x00, 0x1C, // offset = 28
        0x00, 0x00, 0x00, 0x0A, // length = 10
        0x00, 0x00, // version
        0x00, 0x00, // xAvgCharWidth
        0x00, 0x00, // usWeightClass
        0x00, 0x00, // usWidthClass
        0x00, 0x00, // fsType placeholder
    ];
    bytes[36] = (fs_type >> 8) as u8;
    bytes[37] = (fs_type & 0xFF) as u8;
    bytes
}

fn obfuscate_embedded_font(bytes: &[u8], key: &str) -> Vec<u8> {
    let normalized = key
        .chars()
        .filter(|ch| ch.is_ascii_hexdigit())
        .collect::<String>();
    let mut guid = (0..normalized.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&normalized[index..index + 2], 16).unwrap())
        .collect::<Vec<_>>();
    guid.reverse();

    let mut output = bytes.to_vec();
    for (index, byte) in output.iter_mut().take(32).enumerate() {
        *byte ^= guid[index % guid.len()];
    }
    output
}

fn embedded_font_docx(fs_type: u16) -> Vec<u8> {
    let font_key = "00112233445566778899AABBCCDDEEFF";
    let font_bytes = obfuscate_embedded_font(&fake_sfnt(fs_type), font_key);

    let mut buffer = Cursor::new(Vec::new());
    let options = FileOptions::default();
    let mut zip = zip::ZipWriter::new(&mut buffer);

    zip.start_file("[Content_Types].xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/fontTable.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.fontTable+xml"/>
  <Override PartName="/word/fonts/font1.odttf" ContentType="application/vnd.openxmlformats-officedocument.obfuscatedFont"/>
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
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r>
        <w:rPr><w:rFonts w:ascii="Embedded Sans"/></w:rPr>
        <w:t>Embedded font sample</w:t>
      </w:r>
    </w:p>
    <w:sectPr>
      <w:pgSz w:w="12240" w:h="15840"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/>
    </w:sectPr>
  </w:body>
</w:document>"#
    )
    .unwrap();

    zip.start_file("word/fontTable.xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:fonts xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
         xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:font w:name="Embedded Sans">
    <w:embedRegular r:id="rIdFont1" w:fontKey="{font_key}"/>
  </w:font>
</w:fonts>"#
    )
    .unwrap();

    zip.start_file("word/_rels/fontTable.xml.rels", options)
        .unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdFont1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/font" Target="fonts/font1.odttf"/>
</Relationships>"#
    )
    .unwrap();

    zip.start_file("word/fonts/font1.odttf", options).unwrap();
    zip.write_all(&font_bytes).unwrap();

    zip.finish().unwrap();
    buffer.into_inner()
}

fn fallback_docx() -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    let options = FileOptions::default();
    let mut zip = zip::ZipWriter::new(&mut buffer);

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
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
            xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
            xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
            xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"
            xmlns:v="urn:schemas-microsoft-com:vml"
            xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math">
  <w:body>
    <w:p>
      <w:r>
        <w:drawing>
          <wp:inline>
            <wp:extent cx="3657600" cy="1828800"/>
            <a:graphic>
              <a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart">
                <c:chart r:id="rIdChart1"/>
              </a:graphicData>
            </a:graphic>
          </wp:inline>
        </w:drawing>
      </w:r>
    </w:p>
    <w:p>
      <w:r>
        <w:pict>
          <v:shape>
            <v:textbox>
              <w:txbxContent>
                <w:p><w:r><w:t>Textbox fallback</w:t></w:r></w:p>
              </w:txbxContent>
            </v:textbox>
          </v:shape>
        </w:pict>
      </w:r>
    </w:p>
    <w:p>
      <m:oMathPara>
        <m:oMath>
          <m:r><m:t>x+y</m:t></m:r>
        </m:oMath>
      </m:oMathPara>
    </w:p>
    <w:sectPr>
      <w:pgSz w:w="12240" w:h="15840"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/>
    </w:sectPr>
  </w:body>
</w:document>"#
    )
    .unwrap();

    zip.finish().unwrap();
    buffer.into_inner()
}

fn temp_test_path(name: &str, extension: &str) -> PathBuf {
    let unique = format!(
        "docx2typst-{}-{}-{}.{}",
        name,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        extension
    );
    std::env::temp_dir().join(unique)
}

fn letterhead_docx() -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    let options = FileOptions::default();
    let mut zip = zip::ZipWriter::new(&mut buffer);

    zip.start_file("[Content_Types].xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/>
  <Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/>
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
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    <w:p/>
    <w:sectPr>
      <w:headerReference w:type="default" r:id="rIdHeader"/>
      <w:footerReference w:type="default" r:id="rIdFooter"/>
      <w:pgSz w:w="12240" w:h="15840"/>
      <w:pgMar w:top="720" w:right="720" w:bottom="720" w:left="720"/>
    </w:sectPr>
  </w:body>
</w:document>"#
    )
    .unwrap();

    zip.start_file("word/_rels/document.xml.rels", options)
        .unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/>
  <Relationship Id="rIdFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/>
</Relationships>"#
    )
    .unwrap();

    zip.start_file("word/header1.xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
       xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
       xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
  <w:p>
    <w:r>
      <w:drawing>
        <wp:anchor behindDoc="1">
          <wp:positionH relativeFrom="column"><wp:posOffset>-182880</wp:posOffset></wp:positionH>
          <wp:positionV relativeFrom="paragraph"><wp:posOffset>-91440</wp:posOffset></wp:positionV>
          <wp:extent cx="1828800" cy="457200"/>
          <wp:docPr id="1" name="Header Image"/>
          <a:graphic>
            <a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">
              <pic:pic>
                <pic:blipFill>
                  <a:blip r:embed="rId1"/>
                  <a:stretch><a:fillRect/></a:stretch>
                </pic:blipFill>
              </pic:pic>
            </a:graphicData>
          </a:graphic>
        </wp:anchor>
      </w:drawing>
    </w:r>
  </w:p>
</w:hdr>"#
    )
    .unwrap();

    zip.start_file("word/footer1.xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
       xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
       xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
  <w:p>
    <w:r>
      <w:drawing>
        <wp:anchor behindDoc="1">
          <wp:positionH relativeFrom="column"><wp:posOffset>-182880</wp:posOffset></wp:positionH>
          <wp:positionV relativeFrom="paragraph"><wp:posOffset>-91440</wp:posOffset></wp:positionV>
          <wp:extent cx="1828800" cy="457200"/>
          <wp:docPr id="2" name="Footer Image"/>
          <a:graphic>
            <a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">
              <pic:pic>
                <pic:blipFill>
                  <a:blip r:embed="rId1"/>
                  <a:stretch><a:fillRect/></a:stretch>
                </pic:blipFill>
              </pic:pic>
            </a:graphicData>
          </a:graphic>
        </wp:anchor>
      </w:drawing>
    </w:r>
  </w:p>
</w:ftr>"#
    )
    .unwrap();

    zip.start_file("word/_rels/header1.xml.rels", options)
        .unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>
</Relationships>"#
    )
    .unwrap();

    zip.start_file("word/_rels/footer1.xml.rels", options)
        .unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image2.png"/>
</Relationships>"#
    )
    .unwrap();

    zip.start_file("word/media/image1.png", options).unwrap();
    zip.write_all(&tiny_png()).unwrap();
    zip.start_file("word/media/image2.png", options).unwrap();
    zip.write_all(&tiny_png()).unwrap();

    zip.finish().unwrap();
    buffer.into_inner()
}

fn fidelity_docx() -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    let options = FileOptions::default();
    let mut zip = zip::ZipWriter::new(&mut buffer);

    zip.start_file("[Content_Types].xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
  <Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/>
  <Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/>
  <Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/>
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
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    <w:p/>
    <w:p>
      <w:r><w:t>Header-safe body start</w:t></w:r>
    </w:p>
    <w:tbl>
      <w:tblPr>
        <w:tblLook w:firstRow="1" w:lastRow="0" w:firstColumn="1" w:lastColumn="0" w:noHBand="0" w:noVBand="1"/>
      </w:tblPr>
      <w:tblGrid>
        <w:gridCol w:w="2400"/>
        <w:gridCol w:w="2400"/>
        <w:gridCol w:w="2400"/>
      </w:tblGrid>
      <w:tr>
        <w:trPr><w:trHeight w:val="480"/></w:trPr>
        <w:tc>
          <w:tcPr><w:tcW w:w="2400" w:type="dxa"/><w:vMerge w:val="restart"/></w:tcPr>
          <w:p><w:r><w:t>Merged label</w:t></w:r></w:p>
        </w:tc>
        <w:tc>
          <w:tcPr><w:tcW w:w="4800" w:type="dxa"/><w:gridSpan w:val="2"/><w:shd w:val="clear" w:color="auto" w:fill="E7E6E6"/></w:tcPr>
          <w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:t>Spanned header</w:t></w:r></w:p>
        </w:tc>
      </w:tr>
      <w:tr>
        <w:trPr><w:trHeight w:val="480"/></w:trPr>
        <w:tc>
          <w:tcPr><w:tcW w:w="2400" w:type="dxa"/><w:vMerge/></w:tcPr>
          <w:p/>
        </w:tc>
        <w:tc>
          <w:tcPr><w:tcW w:w="2400" w:type="dxa"/></w:tcPr>
          <w:p>
            <w:pPr>
              <w:pStyle w:val="ListParagraph"/>
              <w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr>
            </w:pPr>
            <w:r><w:t>Bullet one</w:t></w:r>
          </w:p>
          <w:p>
            <w:pPr>
              <w:pStyle w:val="ListParagraph"/>
              <w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr>
            </w:pPr>
            <w:r><w:t>Bullet two</w:t></w:r>
          </w:p>
        </w:tc>
        <w:tc>
          <w:tcPr><w:tcW w:w="2400" w:type="dxa"/></w:tcPr>
          <w:p>
            <w:pPr><w:rPr><w:b/></w:rPr></w:pPr>
            <w:r><w:t>BoldWord</w:t></w:r>
            <w:r><w:rPr><w:b w:val="0"/></w:rPr><w:t> PlainWord</w:t></w:r>
          </w:p>
        </w:tc>
      </w:tr>
    </w:tbl>
    <w:p>
      <w:hyperlink r:id="rIdHyper">
        <w:r><w:t>htt</w:t></w:r>
        <w:r><w:t>ps://split.example.com</w:t></w:r>
      </w:hyperlink>
    </w:p>
    <w:p>
      <w:r><w:br w:type="page"/></w:r>
    </w:p>
    <w:p>
      <w:r><w:t>Second page body</w:t></w:r>
    </w:p>
    <w:sectPr>
      <w:headerReference w:type="default" r:id="rIdHeader"/>
      <w:footerReference w:type="default" r:id="rIdFooter"/>
      <w:pgSz w:w="12240" w:h="15840"/>
      <w:pgMar w:top="720" w:right="720" w:bottom="720" w:left="720" w:header="708" w:footer="708"/>
    </w:sectPr>
  </w:body>
</w:document>"#
    )
    .unwrap();

    zip.start_file("word/_rels/document.xml.rels", options)
        .unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/>
  <Relationship Id="rIdFooter" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/>
  <Relationship Id="rIdHyper" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://split.example.com" TargetMode="External"/>
</Relationships>"#
    )
    .unwrap();

    zip.start_file("word/styles.xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:docDefaults>
    <w:pPrDefault><w:pPr><w:spacing w:after="160" w:line="278" w:lineRule="auto"/></w:pPr></w:pPrDefault>
    <w:rPrDefault><w:rPr><w:rFonts w:ascii="Libertinus Serif"/><w:sz w:val="22"/></w:rPr></w:rPrDefault>
  </w:docDefaults>
  <w:style w:type="paragraph" w:default="1" w:styleId="Normal">
    <w:name w:val="Normal"/>
    <w:pPr><w:spacing w:after="160" w:line="278" w:lineRule="auto"/></w:pPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="ListParagraph">
    <w:name w:val="List Paragraph"/>
    <w:basedOn w:val="Normal"/>
    <w:pPr><w:ind w:left="720"/></w:pPr>
  </w:style>
</w:styles>"#
    )
    .unwrap();

    zip.start_file("word/numbering.xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="1">
    <w:lvl w:ilvl="0">
      <w:numFmt w:val="bullet"/>
      <w:lvlText w:val="•"/>
      <w:lvlJc w:val="left"/>
      <w:pPr><w:ind w:left="720" w:hanging="360"/></w:pPr>
    </w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num>
</w:numbering>"#
    )
    .unwrap();

    zip.start_file("word/header1.xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
       xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
       xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
  <w:p>
    <w:r>
      <w:drawing>
        <wp:anchor behindDoc="1">
          <wp:positionH relativeFrom="column"><wp:posOffset>0</wp:posOffset></wp:positionH>
          <wp:positionV relativeFrom="paragraph"><wp:posOffset>0</wp:posOffset></wp:positionV>
          <wp:extent cx="5486400" cy="914400"/>
          <wp:docPr id="1" name="Header Image"/>
          <a:graphic>
            <a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">
              <pic:pic>
                <pic:blipFill>
                  <a:blip r:embed="rId1"/>
                  <a:stretch><a:fillRect/></a:stretch>
                </pic:blipFill>
              </pic:pic>
            </a:graphicData>
          </a:graphic>
        </wp:anchor>
      </w:drawing>
    </w:r>
  </w:p>
</w:hdr>"#
    )
    .unwrap();

    zip.start_file("word/footer1.xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
       xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
       xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
  <w:p>
    <w:r>
      <w:drawing>
        <wp:anchor behindDoc="1">
          <wp:positionH relativeFrom="column"><wp:posOffset>0</wp:posOffset></wp:positionH>
          <wp:positionV relativeFrom="paragraph"><wp:posOffset>0</wp:posOffset></wp:positionV>
          <wp:extent cx="5486400" cy="457200"/>
          <wp:docPr id="2" name="Footer Image"/>
          <a:graphic>
            <a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">
              <pic:pic>
                <pic:blipFill>
                  <a:blip r:embed="rId1"/>
                  <a:stretch><a:fillRect/></a:stretch>
                </pic:blipFill>
              </pic:pic>
            </a:graphicData>
          </a:graphic>
        </wp:anchor>
      </w:drawing>
    </w:r>
  </w:p>
</w:ftr>"#
    )
    .unwrap();

    zip.start_file("word/_rels/header1.xml.rels", options)
        .unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>
</Relationships>"#
    )
    .unwrap();

    zip.start_file("word/_rels/footer1.xml.rels", options)
        .unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image2.png"/>
</Relationships>"#
    )
    .unwrap();

    zip.start_file("word/media/image1.png", options).unwrap();
    zip.write_all(&tiny_png()).unwrap();
    zip.start_file("word/media/image2.png", options).unwrap();
    zip.write_all(&tiny_png()).unwrap();

    zip.finish().unwrap();
    buffer.into_inner()
}

fn placeholder_docx() -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    let options = FileOptions::default();
    let mut zip = zip::ZipWriter::new(&mut buffer);

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
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r>
        <w:rPr><w:color w:val="auto"/></w:rPr>
        <w:t>&lt;Client Company Name&gt;</w:t>
      </w:r>
    </w:p>
    <w:p>
      <w:r>
        <w:rPr><w:color w:val="auto"/></w:rPr>
        <w:t>$ -</w:t>
      </w:r>
    </w:p>
    <w:p>
      <w:r>
        <w:t>Contract No. _______</w:t>
      </w:r>
    </w:p>
    <w:sectPr>
      <w:pgSz w:w="12240" w:h="15840"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/>
    </w:sectPr>
  </w:body>
</w:document>"#
    )
    .unwrap();

    zip.finish().unwrap();
    buffer.into_inner()
}

fn styled_table_docx() -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    let options = FileOptions::default();
    let mut zip = zip::ZipWriter::new(&mut buffer);

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
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:tbl>
      <w:tblPr>
        <w:tblW w:w="2000" w:type="dxa"/>
        <w:tblLayout w:type="fixed"/>
        <w:tblCellMar>
          <w:top w:w="0" w:type="dxa"/>
          <w:left w:w="40" w:type="dxa"/>
          <w:bottom w:w="0" w:type="dxa"/>
          <w:right w:w="40" w:type="dxa"/>
        </w:tblCellMar>
      </w:tblPr>
      <w:tblGrid>
        <w:gridCol w:w="1000"/>
        <w:gridCol w:w="1000"/>
      </w:tblGrid>
      <w:tr>
        <w:trPr><w:trHeight w:val="320" w:hRule="atLeast"/></w:trPr>
        <w:tc>
          <w:tcPr>
            <w:tcW w:w="1000" w:type="dxa"/>
            <w:tcBorders>
              <w:bottom w:val="single" w:sz="6" w:space="0" w:color="BFBFBF"/>
            </w:tcBorders>
            <w:tcMar>
              <w:top w:w="100" w:type="dxa"/>
              <w:left w:w="100" w:type="dxa"/>
              <w:bottom w:w="100" w:type="dxa"/>
              <w:right w:w="100" w:type="dxa"/>
            </w:tcMar>
            <w:shd w:fill="F3F3F3" w:val="clear"/>
            <w:vAlign w:val="center"/>
          </w:tcPr>
          <w:p><w:r><w:t>Left</w:t></w:r></w:p>
        </w:tc>
        <w:tc>
          <w:tcPr>
            <w:tcW w:w="1000" w:type="dxa"/>
            <w:tcBorders>
              <w:right w:val="single" w:sz="8" w:space="0" w:color="999999"/>
            </w:tcBorders>
            <w:vAlign w:val="bottom"/>
          </w:tcPr>
          <w:p><w:r><w:t>Right</w:t></w:r></w:p>
        </w:tc>
      </w:tr>
    </w:tbl>
    <w:sectPr>
      <w:pgSz w:w="12240" w:h="15840"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/>
    </w:sectPr>
  </w:body>
</w:document>"#
    )
    .unwrap();

    zip.finish().unwrap();
    buffer.into_inner()
}

fn table_layout_docx() -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    let options = FileOptions::default();
    let mut zip = zip::ZipWriter::new(&mut buffer);

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
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:tbl>
      <w:tblPr>
        <w:tblW w:w="3600" w:type="dxa"/>
        <w:tblInd w:w="240" w:type="dxa"/>
        <w:jc w:val="center"/>
        <w:tblLayout w:type="fixed"/>
        <w:tblCellSpacing w:w="60" w:type="dxa"/>
      </w:tblPr>
      <w:tr>
        <w:trPr><w:trHeight w:val="480" w:hRule="exact"/></w:trPr>
        <w:tc>
          <w:tcPr><w:tcW w:w="1200" w:type="dxa"/></w:tcPr>
          <w:p><w:r><w:t>Left</w:t></w:r></w:p>
        </w:tc>
        <w:tc>
          <w:tcPr><w:tcW w:w="2400" w:type="dxa"/></w:tcPr>
          <w:p><w:r><w:t>Right</w:t></w:r></w:p>
        </w:tc>
      </w:tr>
      <w:tr>
        <w:trPr><w:trHeight w:val="720" w:hRule="atLeast"/></w:trPr>
        <w:tc>
          <w:tcPr><w:tcW w:w="1200" w:type="dxa"/></w:tcPr>
          <w:p><w:r><w:t>Second</w:t></w:r></w:p>
        </w:tc>
        <w:tc>
          <w:tcPr><w:tcW w:w="2400" w:type="dxa"/></w:tcPr>
          <w:p><w:r><w:t>Row</w:t></w:r></w:p>
        </w:tc>
      </w:tr>
    </w:tbl>
    <w:sectPr>
      <w:pgSz w:w="12240" w:h="15840"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/>
    </w:sectPr>
  </w:body>
</w:document>"#
    )
    .unwrap();

    zip.finish().unwrap();
    buffer.into_inner()
}

fn table_percent_width_docx() -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    let options = FileOptions::default();
    let mut zip = zip::ZipWriter::new(&mut buffer);

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
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:tbl>
      <w:tblPr>
        <w:tblW w:w="4000" w:type="pct"/>
      </w:tblPr>
      <w:tr>
        <w:tc><w:p><w:r><w:t>A</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>B</w:t></w:r></w:p></w:tc>
      </w:tr>
    </w:tbl>
    <w:sectPr>
      <w:pgSz w:w="12240" w:h="15840"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/>
    </w:sectPr>
  </w:body>
</w:document>"#
    )
    .unwrap();

    zip.finish().unwrap();
    buffer.into_inner()
}

fn table_style_border_docx() -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    let options = FileOptions::default();
    let mut zip = zip::ZipWriter::new(&mut buffer);

    zip.start_file("[Content_Types].xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
  <Override PartName="/word/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>
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
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:tbl>
      <w:tblPr>
        <w:tblStyle w:val="CustomStyledTable"/>
        <w:tblW w:w="6000" w:type="dxa"/>
        <w:tblLook w:firstRow="1" w:lastRow="1" w:firstColumn="1" w:lastColumn="1" w:noHBand="0" w:noVBand="0"/>
      </w:tblPr>
      <w:tblGrid>
        <w:gridCol w:w="2000"/>
        <w:gridCol w:w="2000"/>
        <w:gridCol w:w="2000"/>
      </w:tblGrid>
      <w:tr>
        <w:trPr>
          <w:cnfStyle w:firstRow="1"/>
        </w:trPr>
        <w:tc>
          <w:tcPr><w:cnfStyle w:firstColumn="1"/></w:tcPr>
          <w:p><w:r><w:t>Header 1</w:t></w:r></w:p>
        </w:tc>
        <w:tc>
          <w:p>
            <w:r><w:t>Header 2</w:t></w:r>
            <w:r><w:rPr><w:b w:val="0"/></w:rPr><w:t xml:space="preserve"> plain</w:t></w:r>
          </w:p>
        </w:tc>
        <w:tc>
          <w:tcPr><w:cnfStyle w:lastColumn="1"/></w:tcPr>
          <w:p><w:r><w:t>Header 3</w:t></w:r></w:p>
        </w:tc>
      </w:tr>
      <w:tr>
        <w:tc>
          <w:tcPr><w:cnfStyle w:firstColumn="1"/></w:tcPr>
          <w:p><w:r><w:t>Row 2 Col 1</w:t></w:r></w:p>
        </w:tc>
        <w:tc>
          <w:tcPr>
            <w:cnfStyle w:evenHBand="1" w:evenVBand="1"/>
            <w:tcBorders>
              <w:right w:val="single" w:sz="8" w:color="FF0000"/>
            </w:tcBorders>
          </w:tcPr>
          <w:p><w:r><w:t>Row 2 Col 2</w:t></w:r></w:p>
        </w:tc>
        <w:tc>
          <w:tcPr><w:cnfStyle w:lastColumn="1"/></w:tcPr>
          <w:p><w:r><w:t>Row 2 Col 3</w:t></w:r></w:p>
        </w:tc>
      </w:tr>
      <w:tr>
        <w:trPr>
          <w:cnfStyle w:lastRow="1"/>
        </w:trPr>
        <w:tc>
          <w:tcPr><w:cnfStyle w:firstColumn="1"/></w:tcPr>
          <w:p><w:r><w:t>Footer 1</w:t></w:r></w:p>
        </w:tc>
        <w:tc>
          <w:p><w:r><w:t>Footer 2</w:t></w:r></w:p>
        </w:tc>
        <w:tc>
          <w:tcPr><w:cnfStyle w:lastColumn="1"/></w:tcPr>
          <w:p><w:r><w:t>Footer 3</w:t></w:r></w:p>
        </w:tc>
      </w:tr>
    </w:tbl>
    <w:sectPr>
      <w:pgSz w:w="12240" w:h="15840"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/>
    </w:sectPr>
  </w:body>
</w:document>"#
    )
    .unwrap();

    zip.start_file("word/styles.xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="table" w:default="1" w:styleId="TableNormal">
    <w:name w:val="Normal Table"/>
    <w:tblPr>
      <w:tblCellMar>
        <w:top w:w="0" w:type="dxa"/>
        <w:left w:w="108" w:type="dxa"/>
        <w:bottom w:w="0" w:type="dxa"/>
        <w:right w:w="108" w:type="dxa"/>
      </w:tblCellMar>
    </w:tblPr>
  </w:style>
  <w:style w:type="table" w:styleId="CustomStyledTable">
    <w:name w:val="Custom Styled Table"/>
    <w:basedOn w:val="TableNormal"/>
    <w:tblPr>
      <w:tblStyleRowBandSize w:val="1"/>
      <w:tblStyleColBandSize w:val="1"/>
      <w:tblBorders>
        <w:top w:val="single" w:sz="4" w:themeColor="accent1" w:themeTint="66"/>
        <w:left w:val="single" w:sz="4" w:themeColor="accent1" w:themeTint="66"/>
        <w:bottom w:val="single" w:sz="4" w:themeColor="accent1" w:themeTint="66"/>
        <w:right w:val="single" w:sz="4" w:themeColor="accent1" w:themeTint="66"/>
        <w:insideH w:val="single" w:sz="4" w:themeColor="accent1" w:themeTint="66"/>
        <w:insideV w:val="single" w:sz="4" w:themeColor="accent1" w:themeTint="66"/>
      </w:tblBorders>
    </w:tblPr>
    <w:tblStylePr w:type="firstRow">
      <w:rPr><w:b/></w:rPr>
      <w:tcPr>
        <w:tcBorders>
          <w:bottom w:val="single" w:sz="12" w:themeColor="accent1" w:themeTint="99"/>
        </w:tcBorders>
      </w:tcPr>
    </w:tblStylePr>
    <w:tblStylePr w:type="lastRow">
      <w:rPr><w:b/></w:rPr>
      <w:tcPr>
        <w:tcBorders>
          <w:top w:val="single" w:sz="8" w:color="C00000"/>
        </w:tcBorders>
      </w:tcPr>
    </w:tblStylePr>
    <w:tblStylePr w:type="firstCol">
      <w:rPr><w:b/></w:rPr>
      <w:tcPr>
        <w:tcBorders>
          <w:left w:val="single" w:sz="10" w:color="00AA00"/>
        </w:tcBorders>
      </w:tcPr>
    </w:tblStylePr>
    <w:tblStylePr w:type="lastCol">
      <w:rPr><w:b/></w:rPr>
      <w:tcPr>
        <w:tcBorders>
          <w:right w:val="single" w:sz="10" w:color="AA00AA"/>
        </w:tcBorders>
      </w:tcPr>
    </w:tblStylePr>
    <w:tblStylePr w:type="band1Horz">
      <w:tcPr>
        <w:tcBorders>
          <w:bottom w:val="single" w:sz="5" w:color="2255FF"/>
        </w:tcBorders>
      </w:tcPr>
    </w:tblStylePr>
    <w:tblStylePr w:type="band2Horz">
      <w:tcPr>
        <w:tcBorders>
          <w:bottom w:val="single" w:sz="7" w:color="0044AA"/>
        </w:tcBorders>
      </w:tcPr>
    </w:tblStylePr>
    <w:tblStylePr w:type="band1Vert">
      <w:tcPr>
        <w:tcBorders>
          <w:right w:val="single" w:sz="5" w:color="FF8800"/>
        </w:tcBorders>
      </w:tcPr>
    </w:tblStylePr>
    <w:tblStylePr w:type="band2Vert">
      <w:tcPr>
        <w:tcBorders>
          <w:right w:val="single" w:sz="6" w:color="8844FF"/>
        </w:tcBorders>
      </w:tcPr>
    </w:tblStylePr>
  </w:style>
</w:styles>"#
    )
    .unwrap();

    zip.start_file("word/theme/theme1.xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Test Theme">
  <a:themeElements>
    <a:clrScheme name="Test">
      <a:dk1><a:srgbClr val="000000"/></a:dk1>
      <a:lt1><a:srgbClr val="FFFFFF"/></a:lt1>
      <a:dk2><a:srgbClr val="44546A"/></a:dk2>
      <a:lt2><a:srgbClr val="E7E6E6"/></a:lt2>
      <a:accent1><a:srgbClr val="4472C4"/></a:accent1>
      <a:accent2><a:srgbClr val="ED7D31"/></a:accent2>
      <a:accent3><a:srgbClr val="A5A5A5"/></a:accent3>
      <a:accent4><a:srgbClr val="FFC000"/></a:accent4>
      <a:accent5><a:srgbClr val="5B9BD5"/></a:accent5>
      <a:accent6><a:srgbClr val="70AD47"/></a:accent6>
      <a:hlink><a:srgbClr val="0563C1"/></a:hlink>
      <a:folHlink><a:srgbClr val="954F72"/></a:folHlink>
    </a:clrScheme>
  </a:themeElements>
</a:theme>"#
    )
    .unwrap();

    zip.finish().unwrap();
    buffer.into_inner()
}

fn heading_pagebreak_docx() -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    let options = FileOptions::default();
    let mut zip = zip::ZipWriter::new(&mut buffer);

    zip.start_file("[Content_Types].xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
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
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
      <w:r><w:br w:type="page"/></w:r>
      <w:r><w:t>Abstract</w:t></w:r>
    </w:p>
    <w:p><w:r><w:t>Body text.</w:t></w:r></w:p>
    <w:sectPr>
      <w:pgSz w:w="12240" w:h="15840"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/>
    </w:sectPr>
  </w:body>
</w:document>"#
    )
    .unwrap();

    zip.start_file("word/styles.xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="Heading 1"/>
    <w:rPr><w:b/></w:rPr>
  </w:style>
</w:styles>"#
    )
    .unwrap();

    zip.finish().unwrap();
    buffer.into_inner()
}

fn header_field_docx() -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    let options = FileOptions::default();
    let mut zip = zip::ZipWriter::new(&mut buffer);

    zip.start_file("[Content_Types].xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/>
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
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    <w:p><w:r><w:t>Body text.</w:t></w:r></w:p>
    <w:sectPr>
      <w:headerReference w:type="default" r:id="rIdHeader"/>
      <w:pgSz w:w="12240" w:h="15840"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720"/>
    </w:sectPr>
  </w:body>
</w:document>"#
    )
    .unwrap();

    zip.start_file("word/_rels/document.xml.rels", options)
        .unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/>
</Relationships>"#
    )
    .unwrap();

    zip.start_file("word/header1.xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:p>
    <w:sdt>
      <w:sdtContent>
        <w:r><w:t xml:space="preserve">HEADER TITLE </w:t></w:r>
        <w:r><w:fldChar w:fldCharType="begin"/></w:r>
        <w:r><w:instrText xml:space="preserve"> PAGE   \* MERGEFORMAT </w:instrText></w:r>
        <w:r><w:fldChar w:fldCharType="separate"/></w:r>
        <w:r><w:t>19</w:t></w:r>
        <w:r><w:fldChar w:fldCharType="end"/></w:r>
      </w:sdtContent>
    </w:sdt>
  </w:p>
</w:hdr>"#
    )
    .unwrap();

    zip.finish().unwrap();
    buffer.into_inner()
}

fn last_rendered_pagebreak_docx() -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    let options = FileOptions::default();
    let mut zip = zip::ZipWriter::new(&mut buffer);

    zip.start_file("[Content_Types].xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
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
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:br w:type="page"/></w:r></w:p>
    <w:p>
      <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
      <w:r><w:lastRenderedPageBreak/></w:r>
      <w:r><w:t>Abstract</w:t></w:r>
    </w:p>
    <w:p><w:r><w:t>Body text.</w:t></w:r></w:p>
    <w:sectPr>
      <w:pgSz w:w="12240" w:h="15840"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/>
    </w:sectPr>
  </w:body>
</w:document>"#
    )
    .unwrap();

    zip.start_file("word/styles.xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="Heading 1"/>
    <w:rPr><w:b/></w:rPr>
  </w:style>
</w:styles>"#
    )
    .unwrap();

    zip.finish().unwrap();
    buffer.into_inner()
}

fn email_docx() -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    let options = FileOptions::default();
    let mut zip = zip::ZipWriter::new(&mut buffer);

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
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>Email: example@institution.edu</w:t></w:r></w:p>
    <w:sectPr>
      <w:pgSz w:w="12240" w:h="15840"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/>
    </w:sectPr>
  </w:body>
</w:document>"#
    )
    .unwrap();

    zip.finish().unwrap();
    buffer.into_inner()
}

fn list_pagebreak_docx() -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    let options = FileOptions::default();
    let mut zip = zip::ZipWriter::new(&mut buffer);

    zip.start_file("[Content_Types].xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/>
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
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr>
        <w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr>
      </w:pPr>
      <w:r><w:t>Before break</w:t></w:r>
      <w:r><w:br w:type="page"/></w:r>
      <w:r><w:t>After break</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr>
        <w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr>
      </w:pPr>
      <w:r><w:t>Second item</w:t></w:r>
    </w:p>
    <w:sectPr>
      <w:pgSz w:w="12240" w:h="15840"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/>
    </w:sectPr>
  </w:body>
</w:document>"#
    )
    .unwrap();

    zip.start_file("word/numbering.xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="1">
    <w:lvl w:ilvl="0">
      <w:numFmt w:val="decimal"/>
      <w:lvlText w:val="%1."/>
    </w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num>
</w:numbering>"#
    )
    .unwrap();

    zip.finish().unwrap();
    buffer.into_inner()
}

fn nested_list_docx() -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    let options = FileOptions::default();
    let mut zip = zip::ZipWriter::new(&mut buffer);

    zip.start_file("[Content_Types].xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/>
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
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr>
      <w:r><w:t>Parent 1</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="1"/></w:numPr></w:pPr>
      <w:r><w:t>Child a</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="1"/></w:numPr></w:pPr>
      <w:r><w:t>Child b</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr>
      <w:r><w:t>Parent 2</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="2"/></w:numPr></w:pPr>
      <w:r><w:t>Bullet 1</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="2"/></w:numPr></w:pPr>
      <w:r><w:t>Bullet child</w:t></w:r>
    </w:p>
    <w:sectPr>
      <w:pgSz w:w="12240" w:h="15840"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/>
    </w:sectPr>
  </w:body>
</w:document>"#
    )
    .unwrap();

    zip.start_file("word/numbering.xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="1">
    <w:lvl w:ilvl="0">
      <w:numFmt w:val="decimal"/>
      <w:lvlText w:val="%1."/>
    </w:lvl>
    <w:lvl w:ilvl="1">
      <w:numFmt w:val="lowerLetter"/>
      <w:lvlText w:val="%2."/>
    </w:lvl>
  </w:abstractNum>
  <w:abstractNum w:abstractNumId="2">
    <w:lvl w:ilvl="0">
      <w:numFmt w:val="bullet"/>
      <w:lvlText w:val="•"/>
    </w:lvl>
    <w:lvl w:ilvl="1">
      <w:numFmt w:val="bullet"/>
      <w:lvlText w:val="◦"/>
    </w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num>
  <w:num w:numId="2"><w:abstractNumId w:val="2"/></w:num>
</w:numbering>"#
    )
    .unwrap();

    zip.finish().unwrap();
    buffer.into_inner()
}

fn themed_character_style_docx() -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    let options = FileOptions::default();
    let mut zip = zip::ZipWriter::new(&mut buffer);

    zip.start_file("[Content_Types].xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
  <Override PartName="/word/fontTable.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.fontTable+xml"/>
  <Override PartName="/word/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>
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
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:pStyle w:val="BodyStyle"/></w:pPr>
      <w:r>
        <w:rPr>
          <w:rStyle w:val="AccentChar"/>
          <w:b w:val="0"/>
        </w:rPr>
        <w:t>Themed Accent</w:t>
      </w:r>
    </w:p>
    <w:sectPr>
      <w:pgSz w:w="12240" w:h="15840"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/>
    </w:sectPr>
  </w:body>
</w:document>"#
    )
    .unwrap();

    zip.start_file("word/styles.xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:docDefaults>
    <w:rPrDefault>
      <w:rPr>
        <w:rFonts w:asciiTheme="minorHAnsi"/>
        <w:sz w:val="22"/>
      </w:rPr>
    </w:rPrDefault>
  </w:docDefaults>
  <w:style w:type="paragraph" w:styleId="BodyStyle">
    <w:name w:val="Body Style"/>
    <w:rPr>
      <w:b/>
      <w:i/>
    </w:rPr>
  </w:style>
  <w:style w:type="character" w:styleId="AccentChar">
    <w:name w:val="Accent Char"/>
    <w:rPr>
      <w:u/>
      <w:color w:val="4472C4"/>
      <w:rFonts w:asciiTheme="majorHAnsi"/>
    </w:rPr>
  </w:style>
</w:styles>"#
    )
    .unwrap();

    zip.start_file("word/fontTable.xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:fonts xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:font w:name="Theme Major"/>
  <w:font w:name="Theme Minor"/>
</w:fonts>"#
    )
    .unwrap();

    zip.start_file("word/theme/theme1.xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Office Theme">
  <a:themeElements>
    <a:clrScheme name="Office">
      <a:dk1><a:srgbClr val="000000"/></a:dk1>
      <a:lt1><a:srgbClr val="FFFFFF"/></a:lt1>
    </a:clrScheme>
    <a:fontScheme name="Office">
      <a:majorFont>
        <a:latin typeface="Theme Major"/>
        <a:ea typeface=""/>
        <a:cs typeface="Theme Major CS"/>
      </a:majorFont>
      <a:minorFont>
        <a:latin typeface="Theme Minor"/>
        <a:ea typeface=""/>
        <a:cs typeface="Theme Minor CS"/>
      </a:minorFont>
    </a:fontScheme>
  </a:themeElements>
</a:theme>"#
    )
    .unwrap();

    zip.finish().unwrap();
    buffer.into_inner()
}

#[test]
fn converts_docx_to_typst_and_validates() {
    let result = convert_bytes(sample_docx(), &ConversionOptions::default()).unwrap();
    assert!(result.typst.contains("Quarterly Review"));
    assert!(
        result
            .typst
            .contains("#link(\"https://example.com/appendix\")")
    );

    let report = validate_result(
        &result,
        &ValidationOptions {
            emit_pdf: false,
            ..ValidationOptions::default()
        },
    )
    .unwrap();

    assert!(
        report.syntax_ok,
        "syntax diagnostics: {:?}",
        report.diagnostics
    );
    assert!(
        report.compile_ok,
        "compile diagnostics: {:?}",
        report.diagnostics
    );
}

#[test]
fn validation_can_emit_pdf_bytes() {
    let result = convert_bytes(sample_docx(), &ConversionOptions::default()).unwrap();
    let report = validate_result(&result, &ValidationOptions::default()).unwrap();

    assert!(
        report.compile_ok,
        "compile diagnostics: {:?}",
        report.diagnostics
    );
    assert!(report.pdf_bytes.starts_with(b"%PDF"));

    let report_json = report.report_json().unwrap();
    assert!(!report_json.contains("pdf_bytes"));
}

#[test]
fn converts_header_footer_images_into_typst_assets() {
    let result = convert_bytes(letterhead_docx(), &ConversionOptions::default()).unwrap();
    assert!(result.typst.contains("assets/asset-1.png"));
    assert!(result.typst.contains("assets/asset-2.png"));
    assert!(result.typst.contains("background: ["));
    assert!(result.typst.contains("#place(top + left"));

    let report = validate_result(
        &result,
        &ValidationOptions {
            emit_pdf: false,
            ..ValidationOptions::default()
        },
    )
    .unwrap();

    assert!(
        report.compile_ok,
        "compile diagnostics: {:?}",
        report.diagnostics
    );
}

#[test]
fn preserves_native_layout_features_in_typst_output() {
    let result = convert_bytes(fidelity_docx(), &ConversionOptions::default()).unwrap();

    assert!(result.typst.contains("margin: (top: 1.4917in"));
    assert!(result.typst.contains("table.cell(rowspan: 2)"));
    assert!(result.typst.contains("table.cell(colspan: 2"));
    assert!(result.typst.contains("#list("));
    assert!(result.typst.contains("#strong[BoldWord]"));
    assert!(result.typst.contains(" PlainWord"));
    assert!(!result.typst.contains("#strong[ PlainWord]"));
    assert!(result.typst.contains("https:\\/\\/split.example.com"));
    assert!(result.typst.contains("#pagebreak()"));

    let report = validate_result(
        &result,
        &ValidationOptions {
            emit_pdf: false,
            ..ValidationOptions::default()
        },
    )
    .unwrap();

    assert!(
        report.compile_ok,
        "compile diagnostics: {:?}",
        report.diagnostics
    );
    assert_eq!(report.page_count, Some(2));
}

#[test]
fn escapes_word_placeholders_and_auto_colors() {
    let result = convert_bytes(placeholder_docx(), &ConversionOptions::default()).unwrap();

    assert!(result.typst.contains("text(fill: black)"));
    assert!(result.typst.contains("\\<Client Company Name\\>"));
    assert!(result.typst.contains("\\$ -"));
    assert!(result.typst.contains("Contract No. \\_\\_\\_\\_\\_\\_\\_"));

    let report = validate_result(
        &result,
        &ValidationOptions {
            emit_pdf: false,
            ..ValidationOptions::default()
        },
    )
    .unwrap();

    assert!(
        report.syntax_ok,
        "syntax diagnostics: {:?}",
        report.diagnostics
    );
    assert!(
        report.compile_ok,
        "compile diagnostics: {:?}",
        report.diagnostics
    );
}

#[test]
fn preserves_table_borders_and_cell_margins() {
    let result = convert_bytes(styled_table_docx(), &ConversionOptions::default()).unwrap();

    assert!(result.typst.contains("#table(\n  stroke: none,"));
    assert!(
        result
            .typst
            .contains("inset: (top: 0in, right: 0.0278in, bottom: 0in, left: 0.0278in)")
    );
    assert!(
        result
            .typst
            .contains("stroke: (bottom: 0.75pt + rgb(\"#BFBFBF\"))")
    );
    assert!(
        result
            .typst
            .contains("stroke: (right: 1pt + rgb(\"#999999\"))")
    );
    assert!(
        result
            .typst
            .contains("inset: (top: 0.0694in, right: 0.0694in, bottom: 0.0694in, left: 0.0694in)")
    );

    let report = validate_result(
        &result,
        &ValidationOptions {
            emit_pdf: false,
            ..ValidationOptions::default()
        },
    )
    .unwrap();

    assert!(
        report.compile_ok,
        "compile diagnostics: {:?}",
        report.diagnostics
    );
}

#[test]
fn emits_table_layout_wrappers_and_exact_row_heights() {
    let result = convert_bytes(table_layout_docx(), &ConversionOptions::default()).unwrap();

    assert!(result.typst.contains("#align(center)["));
    assert!(result.typst.contains("#pad(left: 0.1667in)["));
    assert!(
        result
            .typst
            .contains("#block(width: 2.5in, breakable: true)[#table(")
    );
    assert!(result.typst.contains("gutter: 0.0417in"));
    assert!(result.typst.contains("columns: (1200fr, 2400fr)"));
    assert!(result.typst.contains("rows: (0.3333in, auto)"));

    let report = validate_result(
        &result,
        &ValidationOptions {
            emit_pdf: false,
            ..ValidationOptions::default()
        },
    )
    .unwrap();

    assert!(
        report.compile_ok,
        "compile diagnostics: {:?}",
        report.diagnostics
    );
}

#[test]
fn emits_percent_based_table_widths() {
    let result = convert_bytes(table_percent_width_docx(), &ConversionOptions::default()).unwrap();

    assert!(
        result
            .typst
            .contains("#block(width: 80%, breakable: true)[#table(")
    );

    let report = validate_result(
        &result,
        &ValidationOptions {
            emit_pdf: false,
            ..ValidationOptions::default()
        },
    )
    .unwrap();

    assert!(
        report.compile_ok,
        "compile diagnostics: {:?}",
        report.diagnostics
    );
}

#[test]
fn resolves_table_style_borders_and_direct_overrides() {
    let result = convert_bytes(table_style_border_docx(), &ConversionOptions::default()).unwrap();

    assert!(result.typst.contains("#table(\n  stroke: none,"));
    assert!(result.typst.contains("columns: (2000fr, 2000fr, 2000fr)"));
    assert!(
        !result
            .typst
            .contains("inset: (top: 0in, right: 0.075in, bottom: 0in, left: 0.075in)")
    );
    assert!(result.typst.contains("top: 0.5pt + rgb(\"#B4C6E7\")"));
    assert!(result.typst.contains("bottom: 1.5pt + rgb(\"#8EAADB\")"));
    assert!(result.typst.contains("top: 1pt + rgb(\"#C00000\")"));
    assert!(result.typst.contains("left: 1.25pt + rgb(\"#00AA00\")"));
    assert!(result.typst.contains("right: 1.25pt + rgb(\"#AA00AA\")"));
    assert!(result.typst.contains("bottom: 0.875pt + rgb(\"#0044AA\")"));
    assert!(result.typst.contains("right: 1pt + rgb(\"#FF0000\")"));
    assert!(result.typst.contains("#strong[Header 1]"));
    assert!(result.typst.contains("#strong[Header 2]"));
    assert!(result.typst.contains("#strong[Footer 1]"));
    assert!(result.typst.contains("#strong[Row 2 Col 1]"));
    assert!(result.typst.contains("#strong[Header 3]"));
    assert!(result.typst.contains(" plain"));
    assert!(!result.typst.contains("#strong[ plain]"));

    let report = validate_result(
        &result,
        &ValidationOptions {
            emit_pdf: false,
            ..ValidationOptions::default()
        },
    )
    .unwrap();

    assert!(
        report.compile_ok,
        "compile diagnostics: {:?}",
        report.diagnostics
    );
}

#[test]
fn hoists_pagebreaks_out_of_headings() {
    let result = convert_bytes(heading_pagebreak_docx(), &ConversionOptions::default()).unwrap();

    assert!(result.typst.contains("#pagebreak()"));
    assert!(result.typst.contains("Abstract"));
    assert!(!result.typst.contains("= #pagebreak()"));

    let report = validate_result(
        &result,
        &ValidationOptions {
            emit_pdf: false,
            ..ValidationOptions::default()
        },
    )
    .unwrap();

    assert!(
        report.compile_ok,
        "compile diagnostics: {:?}",
        report.diagnostics
    );
}

#[test]
fn renders_header_content_controls_and_page_fields() {
    let result = convert_bytes(header_field_docx(), &ConversionOptions::default()).unwrap();

    assert!(result.typst.contains("header: ["));
    assert!(result.typst.contains("HEADER TITLE"));
    assert!(result.typst.contains("counter(page).display()"));
    assert!(!result.typst.contains("[19]"));

    let report = validate_result(
        &result,
        &ValidationOptions {
            emit_pdf: false,
            ..ValidationOptions::default()
        },
    )
    .unwrap();

    assert!(
        report.compile_ok,
        "compile diagnostics: {:?}",
        report.diagnostics
    );
}

#[test]
fn escapes_email_addresses_as_plain_text() {
    let result = convert_bytes(email_docx(), &ConversionOptions::default()).unwrap();

    assert!(result.typst.contains("example\\@institution.edu"));

    let report = validate_result(
        &result,
        &ValidationOptions {
            emit_pdf: false,
            ..ValidationOptions::default()
        },
    )
    .unwrap();

    assert!(
        report.compile_ok,
        "compile diagnostics: {:?}",
        report.diagnostics
    );
}

#[test]
fn ignores_last_rendered_pagebreaks() {
    let result = convert_bytes(
        last_rendered_pagebreak_docx(),
        &ConversionOptions::default(),
    )
    .unwrap();

    assert_eq!(result.typst.matches("#pagebreak()").count(), 1);
    assert!(result.typst.contains("Abstract"));

    let report = validate_result(
        &result,
        &ValidationOptions {
            emit_pdf: false,
            ..ValidationOptions::default()
        },
    )
    .unwrap();

    assert!(
        report.compile_ok,
        "compile diagnostics: {:?}",
        report.diagnostics
    );
}

#[test]
fn downgrades_pagebreaks_inside_list_items() {
    let result = convert_bytes(list_pagebreak_docx(), &ConversionOptions::default()).unwrap();

    assert!(result.typst.contains("#colbreak()"));
    assert!(!result.typst.contains("#pagebreak()"));

    let report = validate_result(
        &result,
        &ValidationOptions {
            emit_pdf: false,
            ..ValidationOptions::default()
        },
    )
    .unwrap();

    assert!(
        report.compile_ok,
        "compile diagnostics: {:?}",
        report.diagnostics
    );
}

#[test]
fn emits_real_nested_lists_instead_of_flat_padded_runs() {
    let result = convert_bytes(nested_list_docx(), &ConversionOptions::default()).unwrap();

    assert!(result.typst.contains("#enum("));
    assert!(result.typst.contains("numbering: \"1.\""));
    assert!(result.typst.contains("numbering: \"a.\""));
    assert!(result.typst.contains("#list("));
    assert!(result.typst.contains("Parent 1"));
    assert!(result.typst.contains("Child a"));
    assert!(result.typst.contains("Bullet child"));
    assert!(!result.typst.contains("#pad(left: 2em)["));

    let report = validate_result(
        &result,
        &ValidationOptions {
            emit_pdf: false,
            ..ValidationOptions::default()
        },
    )
    .unwrap();

    assert!(
        report.compile_ok,
        "compile diagnostics: {:?}",
        report.diagnostics
    );
}

#[test]
fn resolves_theme_fonts_and_character_style_precedence() {
    let result =
        convert_bytes(themed_character_style_docx(), &ConversionOptions::default()).unwrap();

    assert!(result.typst.contains("#emph["));
    assert!(result.typst.contains("#underline["));
    assert!(result.typst.contains("text(font: \"Theme Major\")"));
    assert!(result.typst.contains("text(fill: rgb(\"#4472C4\"))"));
    assert!(!result.typst.contains("#strong[Themed Accent]"));

    let inspect = inspect_bytes(themed_character_style_docx()).unwrap();
    assert_eq!(
        inspect.theme_fonts.get("majorHAnsi").map(String::as_str),
        Some("Theme Major")
    );
    assert_eq!(
        inspect.theme_fonts.get("minorHAnsi").map(String::as_str),
        Some("Theme Minor")
    );
}

#[test]
fn records_font_substitutions_and_search_paths() {
    let mut profile = ConversionProfile::default();
    profile
        .fonts
        .substitutions
        .insert("theme major".to_string(), "Resolved Major".to_string());
    profile.fonts.embedded_paths.insert(
        "Theme Minor".to_string(),
        PathBuf::from("/tmp/docx2typst-fonts/ThemeMinor-Regular.ttf"),
    );

    let options = ConversionOptions {
        profile: Some(profile),
        ..ConversionOptions::default()
    };

    let result = convert_bytes(themed_character_style_docx(), &options).unwrap();

    assert!(
        result
            .font_search_paths
            .contains(&PathBuf::from("/tmp/docx2typst-fonts"))
    );
    assert!(result.font_decisions.iter().any(|decision| {
        decision.requested == "Theme Major"
            && decision.resolved == "Resolved Major"
            && decision.reason == "profile substitution"
            && decision.location.is_some()
    }));
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "FONT_SUBSTITUTION"
            && diagnostic.location.is_some()
            && diagnostic
                .context
                .get("requested")
                .is_some_and(|value| value == "Theme Major")
            && diagnostic
                .context
                .get("resolved")
                .is_some_and(|value| value == "Resolved Major")
    }));
}

#[test]
fn extracts_usable_embedded_fonts_from_docx_packages() {
    let result = convert_bytes(embedded_font_docx(0), &ConversionOptions::default()).unwrap();

    assert_eq!(result.embedded_fonts.len(), 1);
    let font = &result.embedded_fonts[0];
    assert_eq!(font.family, "Embedded Sans");
    assert_eq!(font.variant, "regular");
    assert!(font.usable);
    assert!(font.obfuscated);
    assert_eq!(
        font.license.rights,
        docx2typst::FontEmbeddingRights::Installable
    );
    assert_eq!(font.bytes, fake_sfnt(0));
    assert!(
        font.emitted_path
            .as_deref()
            .is_some_and(|path| path.starts_with("fonts/font-1-embedded-sans-regular"))
    );
    assert!(result.font_decisions.iter().any(|decision| {
        decision.requested == "Embedded Sans"
            && decision.resolved == "Embedded Sans"
            && decision.reason == "document embedded font"
            && decision
                .location
                .as_ref()
                .is_some_and(|location| location.part == "word/document.xml")
    }));
}

#[test]
fn reports_restricted_embedded_font_licenses() {
    let result = convert_bytes(embedded_font_docx(0x0002), &ConversionOptions::default()).unwrap();

    assert_eq!(result.embedded_fonts.len(), 1);
    let font = &result.embedded_fonts[0];
    assert!(!font.usable);
    assert!(font.bytes.is_empty());
    assert_eq!(
        font.license.rights,
        docx2typst::FontEmbeddingRights::Restricted
    );
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "DOCX_EMBEDDED_FONT_LICENSE"
            && diagnostic.location.is_some()
            && diagnostic
                .context
                .get("family")
                .is_some_and(|value| value == "Embedded Sans")
    }));
}

#[test]
fn renders_svg_fallback_assets_for_unsupported_drawings_and_equations() {
    let result = convert_bytes(fallback_docx(), &ConversionOptions::default()).unwrap();

    assert_eq!(result.fallbacks.len(), 3);
    assert!(
        result
            .fallback_inventory
            .iter()
            .any(|entry| entry == "drawing:chart")
    );
    assert!(
        result
            .fallback_inventory
            .iter()
            .any(|entry| entry == "drawing:text_box")
    );
    assert!(
        result
            .fallback_inventory
            .iter()
            .any(|entry| entry == "equation:omml_equation")
    );
    assert!(
        result
            .typst
            .contains("#docx-fallback(\"drawing\", \"assets/asset-fallback-1.svg\"")
    );
    assert!(
        result
            .typst
            .contains("#docx-fallback(\"equation\", \"assets/asset-fallback-3.svg\"")
    );
    assert!(result.assets.iter().any(|asset| {
        asset.id == "asset-fallback-1" && String::from_utf8_lossy(&asset.bytes).contains("CHART")
    }));
    assert!(result.assets.iter().any(|asset| {
        asset.id == "asset-fallback-2"
            && String::from_utf8_lossy(&asset.bytes).contains("Textbox fallback")
    }));
    assert!(
        result
            .diagnostics
            .iter()
            .filter(|diagnostic| { diagnostic.code == "DOCX_FALLBACK_RENDERED_ASSET" })
            .count()
            >= 3
    );

    let report = validate_result(
        &result,
        &ValidationOptions {
            emit_pdf: false,
            ..ValidationOptions::default()
        },
    )
    .unwrap();

    assert!(
        report.compile_ok,
        "compile diagnostics: {:?}",
        report.diagnostics
    );
}

#[test]
fn validates_against_reference_pdf_when_requested() {
    let result = convert_bytes(sample_docx(), &ConversionOptions::default()).unwrap();
    let baseline = validate_result(&result, &ValidationOptions::default()).unwrap();
    let reference_path = temp_test_path("reference-match", "pdf");
    fs::write(&reference_path, &baseline.pdf_bytes).unwrap();

    let report = validate_result(
        &result,
        &ValidationOptions {
            emit_pdf: false,
            reference_path: Some(reference_path.clone()),
            ..ValidationOptions::default()
        },
    )
    .unwrap();

    let reference = report.reference.as_ref().expect("reference comparison");
    assert!(
        report.compile_ok,
        "compile diagnostics: {:?}",
        report.diagnostics
    );
    assert_eq!(reference.reference_page_count, report.page_count);
    assert_eq!(reference.page_count_delta, Some(0));
    assert_eq!(reference.text_similarity_percent, Some(100));
    assert_eq!(reference.within_threshold, Some(true));
    assert!(
        !report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.starts_with("REFERENCE_"))
    );

    let _ = fs::remove_file(reference_path);
}

#[test]
fn reports_reference_pdf_mismatches() {
    let sample_result = convert_bytes(sample_docx(), &ConversionOptions::default()).unwrap();
    let sample_report = validate_result(&sample_result, &ValidationOptions::default()).unwrap();
    let reference_path = temp_test_path("reference-mismatch", "pdf");
    fs::write(&reference_path, &sample_report.pdf_bytes).unwrap();

    let fidelity_result = convert_bytes(fidelity_docx(), &ConversionOptions::default()).unwrap();
    let report = validate_result(
        &fidelity_result,
        &ValidationOptions {
            emit_pdf: false,
            reference_path: Some(reference_path.clone()),
            reference_text_similarity_threshold_percent: Some(95),
            ..ValidationOptions::default()
        },
    )
    .unwrap();

    let reference = report.reference.as_ref().expect("reference comparison");
    assert!(
        report.compile_ok,
        "compile diagnostics: {:?}",
        report.diagnostics
    );
    assert_eq!(reference.page_count_delta, Some(1));
    assert_eq!(reference.page_count_match, false);
    assert_eq!(reference.within_threshold, Some(false));
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "REFERENCE_PAGE_COUNT_MISMATCH" })
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "REFERENCE_TEXT_BELOW_THRESHOLD" })
    );

    let _ = fs::remove_file(reference_path);
}
