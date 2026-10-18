mod fixtures;

use std::path::{Path, PathBuf};

use ofdbox_core::{
    AnnotationsView, AttachmentsView, CacheOptions, CustomTagsView, DocVersionView, DocumentView,
    ExtensionsView, OfdError, OfdPackage, OfdRootView, PageAnnotView, PageContentView, ReadOptions,
    ResView, SignatureView, SignaturesView,
};

fn visit(dir: &Path, ofd_files: &mut Vec<PathBuf>, root_xml_files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == ".git") {
                continue;
            }
            visit(&path, ofd_files, root_xml_files);
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("ofd"))
        {
            ofd_files.push(path);
        } else if path.file_name().is_some_and(|name| name == "OFD.xml") {
            root_xml_files.push(path);
        }
    }
}

#[test]
fn batch_samples_never_panic() {
    let Some(dir) = fixtures::testdata_dir() else {
        return;
    };

    let mut ofd_files = Vec::new();
    let mut root_xml_files = Vec::new();
    visit(&dir, &mut ofd_files, &mut root_xml_files);
    assert!(!ofd_files.is_empty(), "no .ofd files under fixture dir");

    let mut ofd_ok = 0usize;
    let mut ofd_writer_ok = 0usize;
    let mut document_ok = 0usize;
    let mut document_writer_ok = 0usize;
    let mut page_ok = 0usize;
    let mut page_writer_ok = 0usize;
    let mut res_ok = 0usize;
    let mut res_writer_ok = 0usize;
    let mut annotations_ok = 0usize;
    let mut annotations_writer_ok = 0usize;
    let mut page_annot_ok = 0usize;
    let mut page_annot_writer_ok = 0usize;
    let mut annot_count = 0usize;
    let mut signatures_ok = 0usize;
    let mut signatures_writer_ok = 0usize;
    let mut signature_ok = 0usize;
    let mut signature_writer_ok = 0usize;
    let mut reference_count = 0usize;
    let mut version_ok = 0usize;
    let mut version_writer_ok = 0usize;
    let mut version_file_count = 0usize;
    let mut attachments_ok = 0usize;
    let mut attachments_writer_ok = 0usize;
    let mut attachment_count = 0usize;
    let mut custom_tags_ok = 0usize;
    let mut custom_tags_writer_ok = 0usize;
    let mut custom_tag_count = 0usize;
    let mut extensions_ok = 0usize;
    let mut extensions_writer_ok = 0usize;
    let mut extension_count = 0usize;
    for path in ofd_files {
        let bytes = std::fs::read(&path).expect("read sample");
        let mut package = match OfdPackage::from_bytes_with_options(
            bytes,
            ReadOptions::LENIENT,
            CacheOptions::DISABLED,
        ) {
            Ok(package) => package,
            Err(error) => {
                assert!(matches!(
                    error,
                    OfdError::Zip(_) | OfdError::Xml(_) | OfdError::InvalidStructure(_)
                ));
                continue;
            }
        };

        let view = match package.parse_root() {
            Ok(view) => view,
            Err(error) => {
                assert!(matches!(
                    error,
                    OfdError::Xml(_) | OfdError::InvalidStructure(_) | OfdError::MissingEntry(_)
                ));
                continue;
            }
        };
        ofd_ok += 1;

        if let Ok(xml) = view.to_xml() {
            ofd_writer_ok += 1;
            OfdRootView::from_xml(xml.as_bytes(), ReadOptions::STRICT)
                .expect("strict writer output must pass strict parsing");
        }

        if view.doc_bodies().any(|body| body.signatures().is_some()) {
            match package.parse_signatures() {
                Ok(signatures) => {
                    signatures_ok += 1;
                    if let Ok(xml) = signatures.to_xml() {
                        signatures_writer_ok += 1;
                        SignaturesView::from_xml(xml.as_bytes(), ReadOptions::STRICT)
                            .expect("Signatures strict writer output must pass strict parsing");
                    }
                    let base_locs: Vec<String> = signatures
                        .signatures()
                        .filter_map(|entry| entry.base_loc().map(str::to_owned))
                        .collect();
                    for base_loc in base_locs {
                        match package.parse_signature(&base_loc) {
                            Ok(signature) => {
                                signature_ok += 1;
                                if let Some(info) = signature.signed_info()
                                    && let Some(references) = info.references()
                                {
                                    reference_count += references.references().count();
                                }
                                if let Ok(xml) = signature.to_xml() {
                                    signature_writer_ok += 1;
                                    SignatureView::from_xml(xml.as_bytes(), ReadOptions::STRICT)
                                        .expect("Signature strict writer output must pass strict parsing");
                                }
                            }
                            Err(error) => {
                                assert!(matches!(
                                    error,
                                    OfdError::Xml(_)
                                        | OfdError::InvalidStructure(_)
                                        | OfdError::MissingEntry(_)
                                        | OfdError::Conformance(_)
                                ));
                            }
                        }
                    }
                }
                Err(error) => {
                    assert!(matches!(
                        error,
                        OfdError::Xml(_)
                            | OfdError::InvalidStructure(_)
                            | OfdError::MissingEntry(_)
                            | OfdError::Conformance(_)
                    ));
                }
            }
        }

        let mut version_locs = Vec::new();
        for body in view.doc_bodies() {
            let Some(versions) = body.versions() else {
                continue;
            };
            for version in versions.versions() {
                if let Some(loc) = version.base_loc() {
                    version_locs.push(loc.to_owned());
                }
            }
        }
        for loc in version_locs {
            match package.parse_doc_version(&loc) {
                Ok(version) => {
                    version_ok += 1;
                    version_file_count += version.files().count();
                    if let Ok(xml) = version.to_xml() {
                        version_writer_ok += 1;
                        DocVersionView::from_xml(xml.as_bytes(), ReadOptions::STRICT)
                            .expect("DocVersion strict writer output must pass strict parsing");
                    }
                }
                Err(error) => {
                    assert!(matches!(
                        error,
                        OfdError::Xml(_)
                            | OfdError::InvalidStructure(_)
                            | OfdError::MissingEntry(_)
                            | OfdError::Conformance(_)
                    ));
                }
            }
        }

        match package.parse_document() {
            Ok(document) => {
                document_ok += 1;
                if let Ok(xml) = document.to_xml() {
                    document_writer_ok += 1;
                    DocumentView::from_xml(xml.as_bytes(), ReadOptions::STRICT)
                        .expect("Document strict writer output must pass strict parsing");
                }

                if document.annotations().is_some() {
                    match package.parse_annotations() {
                        Ok(annotations) => {
                            annotations_ok += 1;
                            if let Ok(xml) = annotations.to_xml() {
                                annotations_writer_ok += 1;
                                AnnotationsView::from_xml(xml.as_bytes(), ReadOptions::STRICT)
                                    .expect(
                                        "Annotations strict writer output must pass strict parsing",
                                    );
                            }
                            let annot_locs: Vec<String> = annotations
                                .pages()
                                .filter_map(|page| page.file_loc().map(str::to_owned))
                                .collect();
                            for loc in annot_locs {
                                match package.parse_page_annot(&loc) {
                                    Ok(page_annot) => {
                                        page_annot_ok += 1;
                                        annot_count += page_annot.annots().count();
                                        if let Ok(xml) = page_annot.to_xml() {
                                            page_annot_writer_ok += 1;
                                            PageAnnotView::from_xml(
                                                xml.as_bytes(),
                                                ReadOptions::STRICT,
                                            )
                                            .expect("PageAnnot strict writer output must pass strict parsing");
                                        }
                                    }
                                    Err(error) => {
                                        assert!(matches!(
                                            error,
                                            OfdError::Xml(_)
                                                | OfdError::InvalidStructure(_)
                                                | OfdError::MissingEntry(_)
                                                | OfdError::Conformance(_)
                                        ));
                                    }
                                }
                            }
                        }
                        Err(error) => {
                            assert!(matches!(
                                error,
                                OfdError::Xml(_)
                                    | OfdError::InvalidStructure(_)
                                    | OfdError::MissingEntry(_)
                                    | OfdError::Conformance(_)
                            ));
                        }
                    }
                }

                if document.attachments().is_some() {
                    match package.parse_attachments() {
                        Ok(attachments) => {
                            attachments_ok += 1;
                            attachment_count += attachments.attachments().count();
                            if let Ok(xml) = attachments.to_xml() {
                                attachments_writer_ok += 1;
                                AttachmentsView::from_xml(xml.as_bytes(), ReadOptions::STRICT)
                                    .expect(
                                        "Attachments strict writer output must pass strict parsing",
                                    );
                            }
                        }
                        Err(error) => {
                            assert!(matches!(
                                error,
                                OfdError::Xml(_)
                                    | OfdError::InvalidStructure(_)
                                    | OfdError::MissingEntry(_)
                                    | OfdError::Conformance(_)
                            ));
                        }
                    }
                }

                if document.custom_tags().is_some() {
                    match package.parse_custom_tags() {
                        Ok(custom_tags) => {
                            custom_tags_ok += 1;
                            custom_tag_count += custom_tags.custom_tags().count();
                            if let Ok(xml) = custom_tags.to_xml() {
                                custom_tags_writer_ok += 1;
                                CustomTagsView::from_xml(xml.as_bytes(), ReadOptions::STRICT)
                                    .expect(
                                        "CustomTags strict writer output must pass strict parsing",
                                    );
                            }
                        }
                        Err(error) => {
                            assert!(matches!(
                                error,
                                OfdError::Xml(_)
                                    | OfdError::InvalidStructure(_)
                                    | OfdError::MissingEntry(_)
                                    | OfdError::Conformance(_)
                            ));
                        }
                    }
                }

                if document.extensions().is_some() {
                    match package.parse_extensions() {
                        Ok(extensions) => {
                            extensions_ok += 1;
                            extension_count += extensions.extensions().count();
                            if let Ok(xml) = extensions.to_xml() {
                                extensions_writer_ok += 1;
                                ExtensionsView::from_xml(xml.as_bytes(), ReadOptions::STRICT)
                                    .expect(
                                        "Extensions strict writer output must pass strict parsing",
                                    );
                            }
                        }
                        Err(error) => {
                            assert!(matches!(
                                error,
                                OfdError::Xml(_)
                                    | OfdError::InvalidStructure(_)
                                    | OfdError::MissingEntry(_)
                                    | OfdError::Conformance(_)
                            ));
                        }
                    }
                }

                let pages: Vec<_> = document
                    .pages()
                    .map(|pages| pages.pages().take(3).collect())
                    .unwrap_or_default();
                let res_locs: Vec<String> = document
                    .common_data()
                    .map(|common| {
                        common
                            .public_res()
                            .chain(common.document_res())
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default();
                for loc in res_locs {
                    match package.parse_res_from_document(&loc) {
                        Ok(res) => {
                            res_ok += 1;
                            if let Ok(xml) = res.to_xml() {
                                res_writer_ok += 1;
                                ResView::from_xml(xml.as_bytes(), ReadOptions::STRICT)
                                    .expect("Res strict writer output must pass strict parsing");
                            }
                        }
                        Err(error) => {
                            assert!(matches!(
                                error,
                                OfdError::Xml(_)
                                    | OfdError::InvalidStructure(_)
                                    | OfdError::MissingEntry(_)
                                    | OfdError::Conformance(_)
                            ));
                        }
                    }
                }

                for page in pages {
                    match package.parse_page_for(&page) {
                        Ok(content) => {
                            page_ok += 1;
                            if let Ok(xml) = content.to_xml() {
                                page_writer_ok += 1;
                                PageContentView::from_xml(xml.as_bytes(), ReadOptions::STRICT)
                                    .expect("Page strict writer output must pass strict parsing");
                            }
                        }
                        Err(error) => {
                            assert!(matches!(
                                error,
                                OfdError::Xml(_)
                                    | OfdError::InvalidStructure(_)
                                    | OfdError::MissingEntry(_)
                                    | OfdError::Conformance(_)
                            ));
                        }
                    }
                }
            }
            Err(error) => {
                assert!(matches!(
                    error,
                    OfdError::Xml(_)
                        | OfdError::InvalidStructure(_)
                        | OfdError::MissingEntry(_)
                        | OfdError::Conformance(_)
                ));
            }
        }
    }

    let mut xml_ok = 0usize;
    let mut xml_writer_ok = 0usize;
    for path in root_xml_files {
        let bytes = std::fs::read(&path).expect("read OFD.xml");
        match OfdRootView::from_xml(&bytes, ReadOptions::LENIENT) {
            Ok(view) => {
                xml_ok += 1;
                if let Ok(xml) = view.to_xml() {
                    xml_writer_ok += 1;
                    OfdRootView::from_xml(xml.as_bytes(), ReadOptions::STRICT)
                        .expect("strict writer output must pass strict parsing");
                }
            }
            Err(error) => {
                assert!(matches!(
                    error,
                    OfdError::Xml(_) | OfdError::InvalidStructure(_)
                ));
            }
        }
    }

    eprintln!(
        "批量统计: ofd_ok={ofd_ok}, ofd_writer_ok={ofd_writer_ok}, document_ok={document_ok}, document_writer_ok={document_writer_ok}, page_ok={page_ok}, page_writer_ok={page_writer_ok}, res_ok={res_ok}, res_writer_ok={res_writer_ok}, annotations_ok={annotations_ok}, annotations_writer_ok={annotations_writer_ok}, page_annot_ok={page_annot_ok}, page_annot_writer_ok={page_annot_writer_ok}, annot_count={annot_count}, signatures_ok={signatures_ok}, signatures_writer_ok={signatures_writer_ok}, signature_ok={signature_ok}, signature_writer_ok={signature_writer_ok}, reference_count={reference_count}, version_ok={version_ok}, version_writer_ok={version_writer_ok}, version_file_count={version_file_count}, attachments_ok={attachments_ok}, attachments_writer_ok={attachments_writer_ok}, attachment_count={attachment_count}, custom_tags_ok={custom_tags_ok}, custom_tags_writer_ok={custom_tags_writer_ok}, custom_tag_count={custom_tag_count}, extensions_ok={extensions_ok}, extensions_writer_ok={extensions_writer_ok}, extension_count={extension_count}, xml_ok={xml_ok}, xml_writer_ok={xml_writer_ok}"
    );
    assert!(ofd_ok > 0, "no OFD package parsed successfully");
    assert!(ofd_writer_ok > 0, "no OFD package passed strict writer");
    assert!(document_ok > 0, "no Document.xml parsed successfully");
    assert!(
        document_writer_ok > 0,
        "no Document.xml passed strict writer"
    );
    assert!(res_ok > 0, "no Res file parsed successfully");
    assert!(res_writer_ok > 0, "no Res file passed strict writer");
    assert!(
        annotations_ok > 0,
        "no annotation index parsed successfully"
    );
    assert!(
        annotations_writer_ok > 0,
        "no annotation index passed strict writer"
    );
    assert!(page_annot_ok > 0, "no page annotation file parsed");
    assert!(signatures_ok > 0, "no signature index parsed successfully");
    assert!(
        signature_ok > 0,
        "no signature description parsed successfully"
    );
    assert!(attachments_ok > 0, "no attachment list parsed successfully");
    assert!(
        attachments_writer_ok > 0,
        "no attachment list passed strict writer"
    );
    assert!(custom_tags_ok > 0, "no custom tag list parsed successfully");
    assert!(
        custom_tags_writer_ok > 0,
        "no custom tag list passed strict writer"
    );
    assert!(extensions_ok > 0, "no extension list parsed successfully");
    assert!(
        extensions_writer_ok > 0,
        "no extension list passed strict writer"
    );
    assert!(xml_ok > 0, "no extracted OFD.xml parsed successfully");
    assert!(
        xml_writer_ok > 0,
        "no extracted OFD.xml passed strict writer"
    );
}
