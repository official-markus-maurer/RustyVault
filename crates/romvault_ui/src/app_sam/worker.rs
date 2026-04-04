use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use compress::structured_archive::ZipStructure;
use sevenz_rust::encoder_options::ZstandardOptions;
use sevenz_rust::{
    ArchiveEntry, ArchiveWriter, EncoderConfiguration, EncoderMethod, Password, SourceReader,
};
use trrntzip::{ProcessControl, StopMode, TorrentZip, TorrentZipRebuild, TrrntZipStatus};
use zip::read::ZipArchive;
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::RomVaultApp;

use super::{SamInterruptReader, SamJobRequest, SamSourceKind, SamWorkerEvent};

impl RomVaultApp {
    fn sam_7z_content_methods(
        output_kind: crate::dialogs::SamOutputKind,
    ) -> Option<Vec<EncoderConfiguration>> {
        match output_kind {
            crate::dialogs::SamOutputKind::SevenZipLzma => {
                Some(vec![EncoderConfiguration::new(EncoderMethod::LZMA)])
            }
            crate::dialogs::SamOutputKind::SevenZipZstd => Some(vec![EncoderConfiguration::from(
                ZstandardOptions::from_level(Self::SAM_7Z_ZSTD_LEVEL),
            )]),
            _ => None,
        }
    }

    fn sam_collect_stage_entries(
        base_dir: &Path,
        current_dir: &Path,
        entries: &mut Vec<(PathBuf, bool)>,
    ) -> Result<(), String> {
        let mut children: Vec<_> = fs::read_dir(current_dir)
            .map_err(|err| err.to_string())?
            .flatten()
            .map(|entry| entry.path())
            .collect();
        children.sort_by(|a, b| {
            let sa = a.to_string_lossy();
            let sb = b.to_string_lossy();
            let la = sa.to_ascii_lowercase();
            let lb = sb.to_ascii_lowercase();
            la.cmp(&lb).then(sa.cmp(&sb))
        });

        if current_dir != base_dir {
            entries.push((
                current_dir
                    .strip_prefix(base_dir)
                    .map_err(|err| err.to_string())?
                    .to_path_buf(),
                true,
            ));
        }

        for child in children {
            if child.is_dir() {
                Self::sam_collect_stage_entries(base_dir, &child, entries)?;
            } else {
                entries.push((
                    child
                        .strip_prefix(base_dir)
                        .map_err(|err| err.to_string())?
                        .to_path_buf(),
                    false,
                ));
            }
        }

        Ok(())
    }

    fn sam_source_kind(path: &Path) -> Option<SamSourceKind> {
        if path.is_dir() {
            Some(SamSourceKind::Directory)
        } else if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
        {
            Some(SamSourceKind::Zip)
        } else if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("7z"))
        {
            Some(SamSourceKind::SevenZip)
        } else {
            None
        }
    }

    fn sam_input_allows_source(
        input_kind: crate::dialogs::SamInputKind,
        source_kind: SamSourceKind,
    ) -> bool {
        match input_kind {
            crate::dialogs::SamInputKind::Directory => source_kind == SamSourceKind::Directory,
            crate::dialogs::SamInputKind::Zip => source_kind == SamSourceKind::Zip,
            crate::dialogs::SamInputKind::SevenZip => source_kind == SamSourceKind::SevenZip,
            crate::dialogs::SamInputKind::Mixed => true,
        }
    }

    pub(crate) fn collect_sam_work_items(
        source: &Path,
        recurse: bool,
        input_kind: crate::dialogs::SamInputKind,
        items: &mut Vec<PathBuf>,
        seen: &mut HashSet<PathBuf>,
    ) {
        if let Some(source_kind) = Self::sam_source_kind(source) {
            if Self::sam_input_allows_source(input_kind, source_kind) {
                let canonical = source
                    .canonicalize()
                    .unwrap_or_else(|_| source.to_path_buf());
                if seen.insert(canonical) {
                    items.push(source.to_path_buf());
                }
            }
        }

        if !source.is_dir() || !recurse {
            return;
        }

        let Ok(entries) = fs::read_dir(source) else {
            return;
        };

        for entry in entries.flatten() {
            Self::collect_sam_work_items(&entry.path(), true, input_kind, items, seen);
        }
    }

    pub(crate) fn sam_output_path(
        output_root: &Path,
        source_path: &Path,
        output_kind: crate::dialogs::SamOutputKind,
    ) -> Option<PathBuf> {
        let extension = Self::sam_output_extension(output_kind)?;
        let stem = if source_path.is_dir() {
            source_path.file_name()?.to_string_lossy().to_string()
        } else {
            source_path.file_stem()?.to_string_lossy().to_string()
        };
        Some(output_root.join(format!("{}.{}", stem, extension)))
    }

    pub(crate) fn sam_output_root_for_source(
        source_path: &Path,
        output_directory: &str,
        use_origin_output: bool,
    ) -> Option<PathBuf> {
        if use_origin_output {
            source_path.parent().map(Path::to_path_buf)
        } else if output_directory.trim().is_empty() {
            None
        } else {
            Some(PathBuf::from(output_directory))
        }
    }

    fn sam_archive_temp_path(output_path: &Path) -> PathBuf {
        let file_name = output_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        output_path
            .parent()
            .unwrap_or(Path::new(""))
            .join(format!("__{}.samtmp", file_name))
    }

    fn sam_stage_dir(output_path: &Path) -> PathBuf {
        let file_name = output_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        output_path
            .parent()
            .unwrap_or(Path::new(""))
            .join(format!("__{}.samtmp.dir", file_name))
    }

    fn sam_normalize_archive_entry_name(relative_path: &Path) -> String {
        relative_path.to_string_lossy().replace('\\', "/")
    }

    fn sam_deterministic_7z_entry(relative_path: &Path, is_dir: bool) -> ArchiveEntry {
        let entry_name = Self::sam_normalize_archive_entry_name(relative_path);
        if is_dir {
            ArchiveEntry::new_directory(&entry_name)
        } else {
            ArchiveEntry::new_file(&entry_name)
        }
    }

    fn sam_hard_stop_requested(control: &ProcessControl) -> bool {
        control.is_hard_stop_requested()
    }

    fn sam_copy_stream<R: Read, W: Write>(
        reader: &mut R,
        writer: &mut W,
        control: &ProcessControl,
    ) -> Result<(), String> {
        let mut buffer = [0u8; 64 * 1024];
        loop {
            if Self::sam_hard_stop_requested(control) {
                return Err("USER_ABORTED_HARD".to_string());
            }
            let read = reader.read(&mut buffer).map_err(|err| err.to_string())?;
            if read == 0 {
                return Ok(());
            }
            writer
                .write_all(&buffer[..read])
                .map_err(|err| err.to_string())?;
        }
    }

    fn sam_collect_directory_entries(
        base_dir: &Path,
        current_dir: &Path,
        entries: &mut Vec<(PathBuf, bool)>,
    ) -> Result<(), String> {
        let mut children: Vec<_> = fs::read_dir(current_dir)
            .map_err(|err| err.to_string())?
            .flatten()
            .map(|entry| entry.path())
            .collect();
        children.sort_by(|a, b| {
            let sa = a.to_string_lossy();
            let sb = b.to_string_lossy();
            let la = sa.to_ascii_lowercase();
            let lb = sb.to_ascii_lowercase();
            la.cmp(&lb).then(sa.cmp(&sb))
        });

        if current_dir != base_dir {
            entries.push((
                current_dir
                    .strip_prefix(base_dir)
                    .map_err(|err| err.to_string())?
                    .to_path_buf(),
                true,
            ));
        }

        for child in children {
            if child.is_dir() {
                Self::sam_collect_directory_entries(base_dir, &child, entries)?;
            } else {
                entries.push((
                    child
                        .strip_prefix(base_dir)
                        .map_err(|err| err.to_string())?
                        .to_path_buf(),
                    false,
                ));
            }
        }

        Ok(())
    }

    fn sam_write_zip_from_directory(
        source_dir: &Path,
        output_path: &Path,
        compression: CompressionMethod,
        control: &ProcessControl,
    ) -> Result<(), String> {
        let file = File::create(output_path).map_err(|err| err.to_string())?;
        let mut writer = ZipWriter::new(file);
        let options = FileOptions::<()>::default().compression_method(compression);
        let mut entries = Vec::new();
        Self::sam_collect_directory_entries(source_dir, source_dir, &mut entries)?;

        for (relative_path, is_dir) in entries {
            if Self::sam_hard_stop_requested(control) {
                return Err("USER_ABORTED_HARD".to_string());
            }
            let name = relative_path.to_string_lossy().replace('\\', "/");
            if is_dir {
                writer
                    .add_directory(format!("{}/", name.trim_end_matches('/')), options)
                    .map_err(|err| err.to_string())?;
            } else {
                writer
                    .start_file(&name, options)
                    .map_err(|err| err.to_string())?;
                let mut file =
                    File::open(source_dir.join(&relative_path)).map_err(|err| err.to_string())?;
                Self::sam_copy_stream(&mut file, &mut writer, control)?;
            }
        }

        writer.finish().map_err(|err| err.to_string())?;
        Ok(())
    }

    fn sam_extract_zip_to_directory(
        source_path: &Path,
        stage_dir: &Path,
        control: &ProcessControl,
    ) -> Result<(), String> {
        let file = File::open(source_path).map_err(|err| err.to_string())?;
        let mut archive = ZipArchive::new(file).map_err(|err| err.to_string())?;

        for idx in 0..archive.len() {
            if Self::sam_hard_stop_requested(control) {
                return Err("USER_ABORTED_HARD".to_string());
            }
            let mut entry = archive.by_index(idx).map_err(|err| err.to_string())?;
            let out_path = stage_dir.join(entry.mangled_name());
            if entry.is_dir() {
                fs::create_dir_all(&out_path).map_err(|err| err.to_string())?;
            } else {
                if let Some(parent) = out_path.parent() {
                    fs::create_dir_all(parent).map_err(|err| err.to_string())?;
                }
                let mut output = File::create(&out_path).map_err(|err| err.to_string())?;
                Self::sam_copy_stream(&mut entry, &mut output, control)?;
            }
        }

        Ok(())
    }

    fn sam_extract_7z_to_directory(
        source_path: &Path,
        stage_dir: &Path,
        control: &ProcessControl,
    ) -> Result<(), String> {
        sevenz_rust::decompress_file_with_extract_fn(
            source_path,
            stage_dir,
            |entry, reader, dest| {
                if control.is_hard_stop_requested() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Interrupted,
                        "USER_ABORTED_HARD",
                    )
                    .into());
                }
                let out_path = dest.to_path_buf();
                if entry.name().ends_with('/') {
                    fs::create_dir_all(&out_path)?;
                } else {
                    if let Some(parent) = out_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    let mut output = File::create(&out_path)?;
                    let mut buffer = [0u8; 64 * 1024];
                    loop {
                        if control.is_hard_stop_requested() {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::Interrupted,
                                "USER_ABORTED_HARD",
                            )
                            .into());
                        }
                        let read = reader.read(&mut buffer)?;
                        if read == 0 {
                            break;
                        }
                        output.write_all(&buffer[..read])?;
                    }
                }
                Ok(true)
            },
        )
        .map_err(|err| {
            if control.is_hard_stop_requested() {
                "USER_ABORTED_HARD".to_string()
            } else {
                err.to_string()
            }
        })?;
        Ok(())
    }

    fn sam_prepare_source_directory(
        source_path: &Path,
        source_kind: SamSourceKind,
        stage_dir: &Path,
        control: &ProcessControl,
    ) -> Result<Option<PathBuf>, String> {
        match source_kind {
            SamSourceKind::Directory => Ok(None),
            SamSourceKind::Zip => {
                fs::create_dir_all(stage_dir).map_err(|err| err.to_string())?;
                Self::sam_extract_zip_to_directory(source_path, stage_dir, control)?;
                Ok(Some(stage_dir.to_path_buf()))
            }
            SamSourceKind::SevenZip => {
                fs::create_dir_all(stage_dir).map_err(|err| err.to_string())?;
                Self::sam_extract_7z_to_directory(source_path, stage_dir, control)?;
                Ok(Some(stage_dir.to_path_buf()))
            }
        }
    }

    fn sam_verify_zip_output(output_path: &Path) -> Result<(), String> {
        let file = File::open(output_path).map_err(|err| err.to_string())?;
        let archive = ZipArchive::new(file).map_err(|err| err.to_string())?;
        let _ = archive.len();
        Ok(())
    }

    pub(crate) fn sam_verify_7z_output(output_path: &Path) -> Result<(), String> {
        let mut file = File::open(output_path).map_err(|err| err.to_string())?;
        let password = Password::empty();
        sevenz_rust::Archive::read(&mut file, &password).map_err(|err| err.to_string())?;
        Ok(())
    }

    pub(crate) fn sam_process_7z_item(
        source_path: &Path,
        source_kind: SamSourceKind,
        output_path: &Path,
        output_kind: crate::dialogs::SamOutputKind,
        verify_output: bool,
        control: &ProcessControl,
    ) -> Result<String, String> {
        let temp_archive = Self::sam_archive_temp_path(output_path);
        let stage_dir = Self::sam_stage_dir(output_path);
        let _ = fs::remove_file(&temp_archive);
        let _ = fs::remove_dir_all(&stage_dir);

        let result = (|| -> Result<String, String> {
            let prepared_dir =
                Self::sam_prepare_source_directory(source_path, source_kind, &stage_dir, control)?;
            let source_dir = prepared_dir.as_deref().unwrap_or(source_path);
            let mut entries = Vec::new();
            Self::sam_collect_stage_entries(source_dir, source_dir, &mut entries)?;

            let Some(content_methods) = Self::sam_7z_content_methods(output_kind) else {
                return Err("The selected 7z output type is not available.".to_string());
            };
            let mut writer: ArchiveWriter<File> =
                ArchiveWriter::create(&temp_archive).map_err(|err| err.to_string())?;
            writer.set_content_methods(content_methods);
            let mut solid_entries = Vec::new();
            let mut solid_readers: Vec<SourceReader<SamInterruptReader<File>>> = Vec::new();
            for (relative_path, is_dir) in entries {
                if control.is_hard_stop_requested() {
                    return Err("USER_ABORTED_HARD".to_string());
                }

                let disk_path = source_dir.join(&relative_path);
                let entry = Self::sam_deterministic_7z_entry(&relative_path, is_dir);
                if is_dir {
                    writer
                        .push_archive_entry::<&[u8]>(entry, None)
                        .map_err(|err: sevenz_rust::Error| err.to_string())?;
                } else {
                    let file = File::open(&disk_path).map_err(|err| err.to_string())?;
                    let reader = SourceReader::new(SamInterruptReader {
                        inner: file,
                        control: control.clone(),
                    });
                    solid_entries.push(entry);
                    solid_readers.push(reader);
                }
            }
            if !solid_entries.is_empty() {
                writer
                    .push_archive_entries(solid_entries, solid_readers)
                    .map_err(|err: sevenz_rust::Error| {
                        if control.is_hard_stop_requested() {
                            "USER_ABORTED_HARD".to_string()
                        } else {
                            err.to_string()
                        }
                    })?;
            }
            writer.finish().map_err(|err| err.to_string())?;

            if verify_output {
                Self::sam_verify_7z_output(&temp_archive)?;
            }

            if output_path.exists() {
                let _ = fs::remove_file(output_path);
            }
            fs::rename(&temp_archive, output_path).map_err(|err| err.to_string())?;
            Ok(match output_kind {
                crate::dialogs::SamOutputKind::SevenZipLzma => "SEVENZIP_LZMA_CREATED".to_string(),
                crate::dialogs::SamOutputKind::SevenZipZstd => "SEVENZIP_ZSTD_CREATED".to_string(),
                _ => unreachable!(),
            })
        })();

        if result.is_err() {
            let _ = fs::remove_file(&temp_archive);
        }
        let _ = fs::remove_dir_all(&stage_dir);
        result
    }

    fn sam_process_zip_family_item(
        source_path: &Path,
        source_kind: SamSourceKind,
        output_path: &Path,
        output_kind: crate::dialogs::SamOutputKind,
        verify_output: bool,
        control: &ProcessControl,
    ) -> Result<String, String> {
        let temp_archive = Self::sam_archive_temp_path(output_path);
        let stage_dir = Self::sam_stage_dir(output_path);
        let _ = fs::remove_file(&temp_archive);
        let _ = fs::remove_dir_all(&stage_dir);

        let result = (|| -> Result<String, String> {
            let prepared_dir =
                Self::sam_prepare_source_directory(source_path, source_kind, &stage_dir, control)?;
            let source_dir = prepared_dir.as_deref().unwrap_or(source_path);
            let compression = match output_kind {
                crate::dialogs::SamOutputKind::ZipZstd => CompressionMethod::Zstd,
                _ => CompressionMethod::Deflated,
            };

            Self::sam_write_zip_from_directory(source_dir, &temp_archive, compression, control)?;

            if output_kind == crate::dialogs::SamOutputKind::TorrentZip {
                let mut sam = TorrentZip::new();
                sam.force_rezip = true;
                sam.check_only = false;
                sam.out_zip_type = ZipStructure::ZipTrrnt;
                let status =
                    sam.process_with_control(&temp_archive.to_string_lossy(), Some(control));
                if status == TrrntZipStatus::USER_ABORTED_HARD {
                    return Err("USER_ABORTED_HARD".to_string());
                }
                if status != TrrntZipStatus::VALID_TRRNTZIP {
                    return Err(format!("{:?}", status));
                }
                if verify_output {
                    let verify_status = sam.process(&temp_archive.to_string_lossy());
                    if verify_status != TrrntZipStatus::VALID_TRRNTZIP {
                        return Err(format!(
                            "SAM verification reported {:?} for {}",
                            verify_status,
                            temp_archive.to_string_lossy()
                        ));
                    }
                }
            } else if verify_output {
                Self::sam_verify_zip_output(&temp_archive)?;
            }

            if output_path.exists() {
                let _ = fs::remove_file(output_path);
            }
            fs::rename(&temp_archive, output_path).map_err(|err| err.to_string())?;
            Ok(match output_kind {
                crate::dialogs::SamOutputKind::TorrentZip => "VALID_TRRNTZIP".to_string(),
                crate::dialogs::SamOutputKind::Zip => "ZIP_CREATED".to_string(),
                crate::dialogs::SamOutputKind::ZipZstd => "ZIP_ZSTD_CREATED".to_string(),
                _ => unreachable!(),
            })
        })();

        if result.is_err() {
            let _ = fs::remove_file(&temp_archive);
        }
        if result.is_err() || output_path.exists() {
            let _ = fs::remove_dir_all(&stage_dir);
        }

        result
    }

    fn cleanup_samtmp_for_request(request: &SamJobRequest) -> usize {
        let mut visited = HashSet::new();
        let mut removed = 0;

        if !request.use_origin_output && !request.output_directory.trim().is_empty() {
            let output_dir = PathBuf::from(&request.output_directory);
            if visited.insert(output_dir.clone()) {
                removed += TorrentZipRebuild::cleanup_samtmp_files(&output_dir, true);
            }
        }

        for source in &request.sources {
            let path = PathBuf::from(source);
            let cleanup_root = if request.use_origin_output {
                path.parent().map(Path::to_path_buf).unwrap_or(path.clone())
            } else if path.is_dir() {
                path.clone()
            } else {
                path.parent().map(Path::to_path_buf).unwrap_or(path.clone())
            };
            if visited.insert(cleanup_root.clone()) {
                removed += TorrentZipRebuild::cleanup_samtmp_files(&cleanup_root, true);
            }
        }

        removed
    }

    pub(crate) fn run_sam_job(
        request: SamJobRequest,
        control: ProcessControl,
        tx: Sender<SamWorkerEvent>,
    ) {
        let mut work_items = Vec::new();
        let mut seen = HashSet::new();
        for source in &request.sources {
            Self::collect_sam_work_items(
                Path::new(source),
                request.recurse_subdirs,
                request.input_kind,
                &mut work_items,
                &mut seen,
            );
        }

        let _ = tx.send(SamWorkerEvent::Started {
            total_items: work_items.len(),
        });

        if !Self::sam_output_kind_supported(request.output_kind) {
            let _ = tx.send(SamWorkerEvent::Finished {
                status: Self::sam_output_kind_support_message(request.output_kind)
                    .unwrap_or("The selected SAM output type is not available.")
                    .to_string(),
            });
            return;
        }

        for (idx, source_path) in work_items.iter().enumerate() {
            if control.stop_mode() != StopMode::Running {
                break;
            }

            let item_label = source_path.to_string_lossy().to_string();
            let _ = tx.send(SamWorkerEvent::ItemStarted {
                item: item_label.clone(),
                index: idx + 1,
                total: work_items.len(),
            });

            let Some(source_kind) = Self::sam_source_kind(source_path) else {
                let _ = tx.send(SamWorkerEvent::Log(format!(
                    "SAM skipped unsupported source {}",
                    item_label
                )));
                continue;
            };

            let Some(output_root) = Self::sam_output_root_for_source(
                source_path,
                &request.output_directory,
                request.use_origin_output,
            ) else {
                let _ = tx.send(SamWorkerEvent::Log(format!(
                    "SAM skipped {} because no usable output location could be resolved.",
                    item_label
                )));
                continue;
            };
            let _ = fs::create_dir_all(&output_root);

            let Some(output_path) =
                Self::sam_output_path(&output_root, source_path, request.output_kind)
            else {
                let _ = tx.send(SamWorkerEvent::Log(format!(
                    "SAM skipped {} because the selected output type is not available.",
                    item_label
                )));
                continue;
            };

            if output_path.exists() && !request.rebuild_existing {
                let _ = tx.send(SamWorkerEvent::Log(format!(
                    "SAM skipped {} because {} already exists.",
                    item_label,
                    output_path.to_string_lossy()
                )));
                continue;
            }

            let result = match request.output_kind {
                crate::dialogs::SamOutputKind::TorrentZip
                | crate::dialogs::SamOutputKind::Zip
                | crate::dialogs::SamOutputKind::ZipZstd => Self::sam_process_zip_family_item(
                    source_path,
                    source_kind,
                    &output_path,
                    request.output_kind,
                    request.verify_output,
                    &control,
                ),
                crate::dialogs::SamOutputKind::SevenZipLzma
                | crate::dialogs::SamOutputKind::SevenZipZstd => Self::sam_process_7z_item(
                    source_path,
                    source_kind,
                    &output_path,
                    request.output_kind,
                    request.verify_output,
                    &control,
                ),
            };

            match result {
                Ok(status) => {
                    if request.remove_source {
                        if source_kind == SamSourceKind::Directory {
                            let _ = fs::remove_dir_all(source_path);
                        } else if source_path != &output_path {
                            let _ = fs::remove_file(source_path);
                        }
                    }
                    let _ = tx.send(SamWorkerEvent::ItemFinished {
                        item: item_label,
                        status,
                    });
                }
                Err(status) => {
                    let _ = tx.send(SamWorkerEvent::ItemFinished {
                        item: item_label.clone(),
                        status: status.clone(),
                    });
                    if status == "USER_ABORTED_HARD" {
                        let removed = Self::cleanup_samtmp_for_request(&request);
                        let _ = tx.send(SamWorkerEvent::Log(format!(
                            "SAM hard stop removed {} .samtmp file(s).",
                            removed
                        )));
                        break;
                    }
                    let _ = tx.send(SamWorkerEvent::Log(format!("SAM {}", status)));
                }
            }
        }

        let finish_status = match control.stop_mode() {
            StopMode::HardStop => {
                let removed = Self::cleanup_samtmp_for_request(&request);
                format!("SAM hard stopped. Removed {} .samtmp file(s).", removed)
            }
            StopMode::SoftStop => "SAM soft stopped after the current conversion.".to_string(),
            StopMode::Running => "SAM completed.".to_string(),
        };
        let _ = tx.send(SamWorkerEvent::Finished {
            status: finish_status,
        });
    }
}
