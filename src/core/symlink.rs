//! Symlink provider and link status entry point.

use std::{
    collections::BTreeMap,
    error, fmt, fs, io,
    path::{Path, PathBuf},
};

#[cfg(windows)]
use win_symlinks_client::{
    create_symlink_via_broker, CreateSymlinkOptions as BrokerCreateSymlinkOptions, ErrorCode,
    TargetKind, WinSymlinksError,
};

pub type SymlinkResult<T> = std::result::Result<T, SymlinkError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    File,
    Directory,
}

impl fmt::Display for LinkKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinkKind::File => write!(formatter, "file"),
            LinkKind::Directory => write!(formatter, "directory"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymlinkBackend {
    Std,
    WindowsBroker,
    WindowsStdFallback,
    ExternalLn,
    Mock,
}

impl fmt::Display for SymlinkBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SymlinkBackend::Std => write!(formatter, "std"),
            SymlinkBackend::WindowsBroker => write!(formatter, "windows-broker"),
            SymlinkBackend::WindowsStdFallback => write!(formatter, "windows-std-fallback"),
            SymlinkBackend::ExternalLn => write!(formatter, "external-ln"),
            SymlinkBackend::Mock => write!(formatter, "mock"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkStatus {
    Missing,
    CorrectSymlink { target: PathBuf },
    WrongSymlinkTarget { actual: PathBuf, expected: PathBuf },
    BrokenSymlink { target: PathBuf },
    ExistingRealFile,
    ExistingRealDirectory,
    UnsupportedFileType { path: PathBuf },
}

impl LinkStatus {
    pub fn is_existing_real_path(&self) -> bool {
        matches!(
            self,
            LinkStatus::ExistingRealFile | LinkStatus::ExistingRealDirectory
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymlinkErrorKind {
    SourceNotFound,
    LinkParentNotFound,
    LinkAlreadyExists,
    WrongSymlinkTarget,
    ExistingRealFile,
    ExistingRealDirectory,
    PermissionDenied,
    BrokerUnavailable,
    BrokerProtocolError,
    UnsupportedPlatform,
    UnsupportedLinkKind,
    Io,
}

impl fmt::Display for SymlinkErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SymlinkErrorKind::SourceNotFound => write!(formatter, "source_not_found"),
            SymlinkErrorKind::LinkParentNotFound => write!(formatter, "link_parent_not_found"),
            SymlinkErrorKind::LinkAlreadyExists => write!(formatter, "link_already_exists"),
            SymlinkErrorKind::WrongSymlinkTarget => write!(formatter, "wrong_symlink_target"),
            SymlinkErrorKind::ExistingRealFile => write!(formatter, "existing_real_file"),
            SymlinkErrorKind::ExistingRealDirectory => write!(formatter, "existing_real_directory"),
            SymlinkErrorKind::PermissionDenied => write!(formatter, "permission_denied"),
            SymlinkErrorKind::BrokerUnavailable => write!(formatter, "broker_unavailable"),
            SymlinkErrorKind::BrokerProtocolError => write!(formatter, "broker_protocol_error"),
            SymlinkErrorKind::UnsupportedPlatform => write!(formatter, "unsupported_platform"),
            SymlinkErrorKind::UnsupportedLinkKind => write!(formatter, "unsupported_link_kind"),
            SymlinkErrorKind::Io => write!(formatter, "io"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymlinkError {
    kind: SymlinkErrorKind,
    backend: SymlinkBackend,
    source: Option<PathBuf>,
    link: Option<PathBuf>,
    detail: Option<String>,
    system_code: Option<i32>,
    broker_code: Option<String>,
}

impl SymlinkError {
    pub fn new(kind: SymlinkErrorKind, backend: SymlinkBackend) -> Self {
        Self {
            kind,
            backend,
            source: None,
            link: None,
            detail: None,
            system_code: None,
            broker_code: None,
        }
    }

    pub fn kind(&self) -> SymlinkErrorKind {
        self.kind
    }

    pub fn backend(&self) -> SymlinkBackend {
        self.backend
    }

    pub fn source(&self) -> Option<&Path> {
        self.source.as_deref()
    }

    pub fn link(&self) -> Option<&Path> {
        self.link.as_deref()
    }

    pub fn system_code(&self) -> Option<i32> {
        self.system_code
    }

    pub fn broker_code(&self) -> Option<&str> {
        self.broker_code.as_deref()
    }

    pub fn with_source(mut self, source: impl Into<PathBuf>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn with_link(mut self, link: impl Into<PathBuf>) -> Self {
        self.link = Some(link.into());
        self
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn with_system_code(mut self, system_code: Option<i32>) -> Self {
        self.system_code = system_code;
        self
    }

    pub fn with_broker_code(mut self, broker_code: impl Into<String>) -> Self {
        self.broker_code = Some(broker_code.into());
        self
    }
}

impl fmt::Display for SymlinkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} via {}", self.kind, self.backend)?;

        if let Some(source) = &self.source {
            write!(formatter, "; source={}", source.display())?;
        }

        if let Some(link) = &self.link {
            write!(formatter, "; link={}", link.display())?;
        }

        if let Some(detail) = &self.detail {
            write!(formatter, "; {detail}")?;
        }

        if let Some(system_code) = self.system_code {
            write!(formatter, "; system_code={system_code}")?;
        }

        if let Some(broker_code) = &self.broker_code {
            write!(formatter, "; broker_code={broker_code}")?;
        }

        Ok(())
    }
}

impl error::Error for SymlinkError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreateSymlinkOptions {
    pub force: bool,
}

impl CreateSymlinkOptions {
    pub const fn new() -> Self {
        Self { force: false }
    }

    pub const fn force_wrong_symlink() -> Self {
        Self { force: true }
    }
}

impl Default for CreateSymlinkOptions {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateSymlinkOutcome {
    Created,
    AlreadyCorrect,
    ReplacedWrongSymlink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoveSymlinkOutcome {
    Removed,
    Missing,
}

pub trait SymlinkProvider {
    fn backend(&self) -> SymlinkBackend;

    fn create_symlink(&mut self, source: &Path, link: &Path, kind: LinkKind) -> SymlinkResult<()>;

    fn remove_symlink(&mut self, link: &Path) -> SymlinkResult<RemoveSymlinkOutcome>;

    fn read_link(&self, link: &Path) -> SymlinkResult<PathBuf>;

    fn link_status(&self, source: &Path, link: &Path, kind: LinkKind) -> SymlinkResult<LinkStatus>;
}

pub fn ensure_symlink(
    provider: &mut dyn SymlinkProvider,
    source: &Path,
    link: &Path,
    kind: LinkKind,
    options: CreateSymlinkOptions,
) -> SymlinkResult<CreateSymlinkOutcome> {
    match provider.link_status(source, link, kind)? {
        LinkStatus::Missing => {
            provider.create_symlink(source, link, kind)?;
            Ok(CreateSymlinkOutcome::Created)
        }
        LinkStatus::CorrectSymlink { .. } => Ok(CreateSymlinkOutcome::AlreadyCorrect),
        LinkStatus::WrongSymlinkTarget { actual, expected } => {
            if !options.force {
                return Err(SymlinkError::new(
                    SymlinkErrorKind::WrongSymlinkTarget,
                    provider.backend(),
                )
                .with_source(expected)
                .with_link(link)
                .with_detail(format!("existing link points to {}", actual.display())));
            }

            provider.remove_symlink(link)?;
            provider.create_symlink(source, link, kind)?;
            Ok(CreateSymlinkOutcome::ReplacedWrongSymlink)
        }
        LinkStatus::BrokenSymlink { target } => Err(SymlinkError::new(
            SymlinkErrorKind::SourceNotFound,
            provider.backend(),
        )
        .with_source(source)
        .with_link(link)
        .with_detail(format!(
            "existing symlink points to missing source {}",
            target.display()
        ))),
        LinkStatus::ExistingRealFile => Err(SymlinkError::new(
            SymlinkErrorKind::ExistingRealFile,
            provider.backend(),
        )
        .with_source(source)
        .with_link(link)),
        LinkStatus::ExistingRealDirectory => Err(SymlinkError::new(
            SymlinkErrorKind::ExistingRealDirectory,
            provider.backend(),
        )
        .with_source(source)
        .with_link(link)),
        LinkStatus::UnsupportedFileType { path } => Err(SymlinkError::new(
            SymlinkErrorKind::UnsupportedLinkKind,
            provider.backend(),
        )
        .with_source(source)
        .with_link(link)
        .with_detail(format!("unsupported file type at {}", path.display()))),
    }
}

pub fn default_provider() -> Box<dyn SymlinkProvider> {
    #[cfg(windows)]
    {
        Box::new(WindowsBrokerSymlinkProvider::new())
    }

    #[cfg(not(windows))]
    {
        Box::new(StdSymlinkProvider::new())
    }
}

#[derive(Debug, Clone)]
pub struct StdSymlinkProvider {
    backend: SymlinkBackend,
}

impl StdSymlinkProvider {
    #[cfg(not(windows))]
    pub const fn new() -> Self {
        Self {
            backend: SymlinkBackend::Std,
        }
    }

    #[cfg(windows)]
    pub const fn explicit_windows_fallback() -> Self {
        Self {
            backend: SymlinkBackend::WindowsStdFallback,
        }
    }
}

impl SymlinkProvider for StdSymlinkProvider {
    fn backend(&self) -> SymlinkBackend {
        self.backend
    }

    fn create_symlink(&mut self, source: &Path, link: &Path, kind: LinkKind) -> SymlinkResult<()> {
        validate_source(source, kind, self.backend)?;
        validate_link_parent(link, self.backend)?;
        reject_existing_link_path(link, self.backend)?;
        create_std_symlink(source, link, kind, self.backend)
    }

    fn remove_symlink(&mut self, link: &Path) -> SymlinkResult<RemoveSymlinkOutcome> {
        remove_fs_symlink(link, self.backend)
    }

    fn read_link(&self, link: &Path) -> SymlinkResult<PathBuf> {
        read_fs_link(link, self.backend)
    }

    fn link_status(&self, source: &Path, link: &Path, kind: LinkKind) -> SymlinkResult<LinkStatus> {
        fs_link_status(source, link, kind, self.backend)
    }
}

#[derive(Debug, Clone, Default)]
pub struct WindowsBrokerSymlinkProvider;

impl WindowsBrokerSymlinkProvider {
    pub const fn new() -> Self {
        Self
    }
}

impl SymlinkProvider for WindowsBrokerSymlinkProvider {
    fn backend(&self) -> SymlinkBackend {
        SymlinkBackend::WindowsBroker
    }

    fn create_symlink(&mut self, source: &Path, link: &Path, kind: LinkKind) -> SymlinkResult<()> {
        #[cfg(windows)]
        {
            validate_source(source, kind, SymlinkBackend::WindowsBroker)?;
            validate_link_parent(link, SymlinkBackend::WindowsBroker)?;
            reject_existing_link_path(link, SymlinkBackend::WindowsBroker)?;

            let options = BrokerCreateSymlinkOptions::new(source, link)
                .target_kind(broker_target_kind(kind))
                .replace_existing_symlink(false);

            create_symlink_via_broker(options).map_err(|error| {
                broker_error_to_symlink_error(error)
                    .with_source(source)
                    .with_link(link)
            })
        }

        #[cfg(not(windows))]
        {
            let _ = kind;
            Err(SymlinkError::new(
                SymlinkErrorKind::UnsupportedPlatform,
                SymlinkBackend::WindowsBroker,
            )
            .with_source(source)
            .with_link(link)
            .with_detail("Windows Broker provider is only available on Windows"))
        }
    }

    fn remove_symlink(&mut self, link: &Path) -> SymlinkResult<RemoveSymlinkOutcome> {
        #[cfg(windows)]
        {
            remove_fs_symlink(link, SymlinkBackend::WindowsBroker)
        }

        #[cfg(not(windows))]
        {
            Err(SymlinkError::new(
                SymlinkErrorKind::UnsupportedPlatform,
                SymlinkBackend::WindowsBroker,
            )
            .with_link(link)
            .with_detail("Windows Broker provider is only available on Windows"))
        }
    }

    fn read_link(&self, link: &Path) -> SymlinkResult<PathBuf> {
        #[cfg(windows)]
        {
            read_fs_link(link, SymlinkBackend::WindowsBroker)
        }

        #[cfg(not(windows))]
        {
            Err(SymlinkError::new(
                SymlinkErrorKind::UnsupportedPlatform,
                SymlinkBackend::WindowsBroker,
            )
            .with_link(link)
            .with_detail("Windows Broker provider is only available on Windows"))
        }
    }

    fn link_status(&self, source: &Path, link: &Path, kind: LinkKind) -> SymlinkResult<LinkStatus> {
        #[cfg(windows)]
        {
            fs_link_status(source, link, kind, SymlinkBackend::WindowsBroker)
        }

        #[cfg(not(windows))]
        {
            let _ = (source, kind);
            Err(SymlinkError::new(
                SymlinkErrorKind::UnsupportedPlatform,
                SymlinkBackend::WindowsBroker,
            )
            .with_link(link)
            .with_detail("Windows Broker provider is only available on Windows"))
        }
    }
}

#[cfg(windows)]
fn broker_target_kind(kind: LinkKind) -> TargetKind {
    match kind {
        LinkKind::File => TargetKind::File,
        LinkKind::Directory => TargetKind::Dir,
    }
}

#[cfg(windows)]
fn broker_error_to_symlink_error(error: WinSymlinksError) -> SymlinkError {
    let kind = match error.code() {
        ErrorCode::ServiceNotInstalled | ErrorCode::ServiceUnavailable => {
            SymlinkErrorKind::BrokerUnavailable
        }
        ErrorCode::UnsupportedMode
        | ErrorCode::ServiceIdentityMismatch
        | ErrorCode::RemoteClientRejected => SymlinkErrorKind::BrokerProtocolError,
        ErrorCode::PrivilegeRequired | ErrorCode::CallerParentWriteDenied => {
            SymlinkErrorKind::PermissionDenied
        }
        ErrorCode::LinkAlreadyExists => SymlinkErrorKind::LinkAlreadyExists,
        ErrorCode::LinkPathIsNotSymlink
        | ErrorCode::UnsafeReparsePoint
        | ErrorCode::ReplacementPartiallyCompleted => SymlinkErrorKind::WrongSymlinkTarget,
        ErrorCode::TargetKindRequired | ErrorCode::TargetKindConflict => {
            SymlinkErrorKind::UnsupportedLinkKind
        }
        ErrorCode::SourceBlacklisted
        | ErrorCode::CreateSymlinkFailed
        | ErrorCode::PathNormalizationFailed => SymlinkErrorKind::Io,
    };

    SymlinkError::new(kind, SymlinkBackend::WindowsBroker)
        .with_detail(error.message().to_string())
        .with_broker_code(error.code().to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MockEntry {
    File,
    Directory,
    Symlink { target: PathBuf, kind: LinkKind },
    Unsupported,
}

#[derive(Debug, Clone, Default)]
pub struct MockSymlinkProvider {
    entries: BTreeMap<PathBuf, MockEntry>,
}

impl MockSymlinkProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_file(&mut self, path: impl Into<PathBuf>) {
        self.entries.insert(path.into(), MockEntry::File);
    }

    pub fn add_dir(&mut self, path: impl Into<PathBuf>) {
        self.entries.insert(path.into(), MockEntry::Directory);
    }

    pub fn add_symlink(
        &mut self,
        link: impl Into<PathBuf>,
        target: impl Into<PathBuf>,
        kind: LinkKind,
    ) {
        self.entries.insert(
            link.into(),
            MockEntry::Symlink {
                target: target.into(),
                kind,
            },
        );
    }

    pub fn add_unsupported(&mut self, path: impl Into<PathBuf>) {
        self.entries.insert(path.into(), MockEntry::Unsupported);
    }

    pub fn entry(&self, path: &Path) -> Option<&MockEntry> {
        self.entries.get(path)
    }

    fn source_matches_kind(&self, source: &Path, kind: LinkKind) -> bool {
        matches!(
            (self.entries.get(source), kind),
            (Some(MockEntry::File), LinkKind::File)
                | (Some(MockEntry::Directory), LinkKind::Directory)
        )
    }

    fn parent_exists(&self, link: &Path) -> bool {
        let Some(parent) = link
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        else {
            return true;
        };

        matches!(self.entries.get(parent), Some(MockEntry::Directory))
    }
}

impl SymlinkProvider for MockSymlinkProvider {
    fn backend(&self) -> SymlinkBackend {
        SymlinkBackend::Mock
    }

    fn create_symlink(&mut self, source: &Path, link: &Path, kind: LinkKind) -> SymlinkResult<()> {
        if !self.entries.contains_key(source) {
            return Err(
                SymlinkError::new(SymlinkErrorKind::SourceNotFound, SymlinkBackend::Mock)
                    .with_source(source)
                    .with_link(link),
            );
        }

        if !self.source_matches_kind(source, kind) {
            return Err(SymlinkError::new(
                SymlinkErrorKind::UnsupportedLinkKind,
                SymlinkBackend::Mock,
            )
            .with_source(source)
            .with_link(link)
            .with_detail(format!("source is not a {kind}")));
        }

        if !self.parent_exists(link) {
            return Err(SymlinkError::new(
                SymlinkErrorKind::LinkParentNotFound,
                SymlinkBackend::Mock,
            )
            .with_source(source)
            .with_link(link));
        }

        if self.entries.contains_key(link) {
            return Err(SymlinkError::new(
                SymlinkErrorKind::LinkAlreadyExists,
                SymlinkBackend::Mock,
            )
            .with_source(source)
            .with_link(link));
        }

        self.add_symlink(link.to_path_buf(), source.to_path_buf(), kind);
        Ok(())
    }

    fn remove_symlink(&mut self, link: &Path) -> SymlinkResult<RemoveSymlinkOutcome> {
        match self.entries.get(link) {
            None => Ok(RemoveSymlinkOutcome::Missing),
            Some(MockEntry::Symlink { .. }) => {
                self.entries.remove(link);
                Ok(RemoveSymlinkOutcome::Removed)
            }
            Some(MockEntry::File) => Err(SymlinkError::new(
                SymlinkErrorKind::ExistingRealFile,
                SymlinkBackend::Mock,
            )
            .with_link(link)),
            Some(MockEntry::Directory) => Err(SymlinkError::new(
                SymlinkErrorKind::ExistingRealDirectory,
                SymlinkBackend::Mock,
            )
            .with_link(link)),
            Some(MockEntry::Unsupported) => Err(SymlinkError::new(
                SymlinkErrorKind::UnsupportedLinkKind,
                SymlinkBackend::Mock,
            )
            .with_link(link)
            .with_detail("unsupported file type cannot be removed as a symlink")),
        }
    }

    fn read_link(&self, link: &Path) -> SymlinkResult<PathBuf> {
        match self.entries.get(link) {
            Some(MockEntry::Symlink { target, .. }) => Ok(target.clone()),
            Some(MockEntry::File) => Err(SymlinkError::new(
                SymlinkErrorKind::ExistingRealFile,
                SymlinkBackend::Mock,
            )
            .with_link(link)),
            Some(MockEntry::Directory) => Err(SymlinkError::new(
                SymlinkErrorKind::ExistingRealDirectory,
                SymlinkBackend::Mock,
            )
            .with_link(link)),
            Some(MockEntry::Unsupported) => Err(SymlinkError::new(
                SymlinkErrorKind::UnsupportedLinkKind,
                SymlinkBackend::Mock,
            )
            .with_link(link)),
            None => Err(
                SymlinkError::new(SymlinkErrorKind::Io, SymlinkBackend::Mock)
                    .with_link(link)
                    .with_detail("link does not exist"),
            ),
        }
    }

    fn link_status(&self, source: &Path, link: &Path, kind: LinkKind) -> SymlinkResult<LinkStatus> {
        match self.entries.get(link) {
            None => Ok(LinkStatus::Missing),
            Some(MockEntry::File) => Ok(LinkStatus::ExistingRealFile),
            Some(MockEntry::Directory) => Ok(LinkStatus::ExistingRealDirectory),
            Some(MockEntry::Unsupported) => Ok(LinkStatus::UnsupportedFileType {
                path: link.to_path_buf(),
            }),
            Some(MockEntry::Symlink { target, .. }) => {
                if !mock_targets_match(link, target, source) {
                    return Ok(LinkStatus::WrongSymlinkTarget {
                        actual: target.clone(),
                        expected: source.to_path_buf(),
                    });
                }

                match self.entries.get(source) {
                    None => Ok(LinkStatus::BrokenSymlink {
                        target: target.clone(),
                    }),
                    Some(MockEntry::File) if kind == LinkKind::File => {
                        Ok(LinkStatus::CorrectSymlink {
                            target: target.clone(),
                        })
                    }
                    Some(MockEntry::Directory) if kind == LinkKind::Directory => {
                        Ok(LinkStatus::CorrectSymlink {
                            target: target.clone(),
                        })
                    }
                    Some(_) => Ok(LinkStatus::UnsupportedFileType {
                        path: source.to_path_buf(),
                    }),
                }
            }
        }
    }
}

fn validate_source(source: &Path, kind: LinkKind, backend: SymlinkBackend) -> SymlinkResult<()> {
    let metadata = match fs::metadata(source) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(SymlinkError::new(SymlinkErrorKind::SourceNotFound, backend)
                .with_source(source)
                .with_system_code(error.raw_os_error()));
        }
        Err(error) => return Err(io_to_symlink_error(error, backend).with_source(source)),
    };

    if metadata_matches_kind(&metadata, kind) {
        Ok(())
    } else {
        Err(
            SymlinkError::new(SymlinkErrorKind::UnsupportedLinkKind, backend)
                .with_source(source)
                .with_detail(format!("source is not a {kind}")),
        )
    }
}

fn validate_link_parent(link: &Path, backend: SymlinkBackend) -> SymlinkResult<()> {
    let Some(parent) = link
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Ok(());
    };

    match fs::metadata(parent) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(
            SymlinkError::new(SymlinkErrorKind::LinkParentNotFound, backend)
                .with_link(link)
                .with_detail(format!("parent is not a directory: {}", parent.display())),
        ),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Err(SymlinkError::new(
            SymlinkErrorKind::LinkParentNotFound,
            backend,
        )
        .with_link(link)
        .with_system_code(error.raw_os_error())),
        Err(error) => Err(io_to_symlink_error(error, backend).with_link(link)),
    }
}

fn reject_existing_link_path(link: &Path, backend: SymlinkBackend) -> SymlinkResult<()> {
    match fs::symlink_metadata(link) {
        Ok(_) => {
            Err(SymlinkError::new(SymlinkErrorKind::LinkAlreadyExists, backend).with_link(link))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_to_symlink_error(error, backend).with_link(link)),
    }
}

fn metadata_matches_kind(metadata: &fs::Metadata, kind: LinkKind) -> bool {
    match kind {
        LinkKind::File => metadata.is_file(),
        LinkKind::Directory => metadata.is_dir(),
    }
}

fn create_std_symlink(
    source: &Path,
    link: &Path,
    kind: LinkKind,
    backend: SymlinkBackend,
) -> SymlinkResult<()> {
    #[cfg(unix)]
    {
        let _ = kind;
        std::os::unix::fs::symlink(source, link).map_err(|error| {
            io_to_symlink_error(error, backend)
                .with_source(source)
                .with_link(link)
        })
    }

    #[cfg(windows)]
    {
        let result = match kind {
            LinkKind::File => std::os::windows::fs::symlink_file(source, link),
            LinkKind::Directory => std::os::windows::fs::symlink_dir(source, link),
        };

        result.map_err(|error| {
            io_to_symlink_error(error, backend)
                .with_source(source)
                .with_link(link)
        })
    }

    #[cfg(all(not(unix), not(windows)))]
    {
        let _ = (source, link, kind);
        Err(SymlinkError::new(
            SymlinkErrorKind::UnsupportedPlatform,
            backend,
        ))
    }
}

fn fs_link_status(
    source: &Path,
    link: &Path,
    kind: LinkKind,
    backend: SymlinkBackend,
) -> SymlinkResult<LinkStatus> {
    let link_metadata = match fs::symlink_metadata(link) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(LinkStatus::Missing),
        Err(error) => return Err(io_to_symlink_error(error, backend).with_link(link)),
    };

    let file_type = link_metadata.file_type();

    if file_type.is_symlink() {
        let target = read_fs_link(link, backend)?;

        if !fs_targets_match(link, &target, source) {
            return Ok(LinkStatus::WrongSymlinkTarget {
                actual: target,
                expected: source.to_path_buf(),
            });
        }

        return match fs::metadata(source) {
            Ok(source_metadata) if metadata_matches_kind(&source_metadata, kind) => {
                Ok(LinkStatus::CorrectSymlink { target })
            }
            Ok(_) => Ok(LinkStatus::UnsupportedFileType {
                path: source.to_path_buf(),
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Ok(LinkStatus::BrokenSymlink { target })
            }
            Err(error) => Err(io_to_symlink_error(error, backend).with_source(source)),
        };
    }

    if link_metadata.is_file() {
        Ok(LinkStatus::ExistingRealFile)
    } else if link_metadata.is_dir() {
        Ok(LinkStatus::ExistingRealDirectory)
    } else {
        Ok(LinkStatus::UnsupportedFileType {
            path: link.to_path_buf(),
        })
    }
}

fn read_fs_link(link: &Path, backend: SymlinkBackend) -> SymlinkResult<PathBuf> {
    fs::read_link(link).map_err(|error| io_to_symlink_error(error, backend).with_link(link))
}

fn remove_fs_symlink(link: &Path, backend: SymlinkBackend) -> SymlinkResult<RemoveSymlinkOutcome> {
    let link_metadata = match fs::symlink_metadata(link) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(RemoveSymlinkOutcome::Missing);
        }
        Err(error) => return Err(io_to_symlink_error(error, backend).with_link(link)),
    };

    if !link_metadata.file_type().is_symlink() {
        if link_metadata.is_file() {
            return Err(
                SymlinkError::new(SymlinkErrorKind::ExistingRealFile, backend).with_link(link),
            );
        }

        if link_metadata.is_dir() {
            return Err(
                SymlinkError::new(SymlinkErrorKind::ExistingRealDirectory, backend).with_link(link),
            );
        }

        return Err(
            SymlinkError::new(SymlinkErrorKind::UnsupportedLinkKind, backend)
                .with_link(link)
                .with_detail("unsupported file type cannot be removed as a symlink"),
        );
    }

    remove_symlink_path(link, &link_metadata, backend)?;
    Ok(RemoveSymlinkOutcome::Removed)
}

#[cfg(unix)]
fn remove_symlink_path(
    link: &Path,
    _metadata: &fs::Metadata,
    backend: SymlinkBackend,
) -> SymlinkResult<()> {
    fs::remove_file(link).map_err(|error| io_to_symlink_error(error, backend).with_link(link))
}

#[cfg(windows)]
fn remove_symlink_path(
    link: &Path,
    metadata: &fs::Metadata,
    backend: SymlinkBackend,
) -> SymlinkResult<()> {
    use std::os::windows::fs::FileTypeExt;

    let result = if metadata.file_type().is_symlink_dir() {
        fs::remove_dir(link)
    } else {
        fs::remove_file(link)
    };

    result.map_err(|error| io_to_symlink_error(error, backend).with_link(link))
}

#[cfg(all(not(unix), not(windows)))]
fn remove_symlink_path(
    link: &Path,
    _metadata: &fs::Metadata,
    backend: SymlinkBackend,
) -> SymlinkResult<()> {
    Err(SymlinkError::new(SymlinkErrorKind::UnsupportedPlatform, backend).with_link(link))
}

fn fs_targets_match(link: &Path, actual: &Path, expected: &Path) -> bool {
    if actual == expected || resolve_relative_target(link, actual) == expected {
        return true;
    }

    let actual = resolve_relative_target(link, actual);
    match (actual.canonicalize(), expected.canonicalize()) {
        (Ok(actual), Ok(expected)) => actual == expected,
        _ => false,
    }
}

fn mock_targets_match(link: &Path, actual: &Path, expected: &Path) -> bool {
    actual == expected || resolve_relative_target(link, actual) == expected
}

fn resolve_relative_target(link: &Path, target: &Path) -> PathBuf {
    if target.is_absolute() {
        return target.to_path_buf();
    }

    link.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map_or_else(|| target.to_path_buf(), |parent| parent.join(target))
}

fn io_to_symlink_error(error: io::Error, backend: SymlinkBackend) -> SymlinkError {
    let kind = match error.kind() {
        io::ErrorKind::PermissionDenied => SymlinkErrorKind::PermissionDenied,
        _ => SymlinkErrorKind::Io,
    };

    SymlinkError::new(kind, backend)
        .with_detail(error.to_string())
        .with_system_code(error.raw_os_error())
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use super::{broker_error_to_symlink_error, ErrorCode, WinSymlinksError};
    use super::{
        default_provider, ensure_symlink, CreateSymlinkOptions, CreateSymlinkOutcome, LinkKind,
        LinkStatus, MockEntry, MockSymlinkProvider, RemoveSymlinkOutcome, StdSymlinkProvider,
        SymlinkBackend, SymlinkErrorKind, SymlinkProvider,
    };
    use std::path::PathBuf;
    #[cfg(unix)]
    use std::{
        fs,
        path::Path,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn path(value: &str) -> PathBuf {
        PathBuf::from(value)
    }

    fn base_mock() -> MockSymlinkProvider {
        let mut provider = MockSymlinkProvider::new();
        provider.add_dir(path("project"));
        provider
    }

    #[test]
    fn mock_link_status_covers_all_statuses() {
        let mut provider = base_mock();
        provider.add_file(path("project/source.md"));
        provider.add_file(path("project/other.md"));
        provider.add_symlink(
            path("project/correct.md"),
            path("project/source.md"),
            LinkKind::File,
        );
        provider.add_symlink(
            path("project/wrong.md"),
            path("project/other.md"),
            LinkKind::File,
        );
        provider.add_symlink(
            path("project/broken.md"),
            path("project/missing-source.md"),
            LinkKind::File,
        );
        provider.add_file(path("project/real-file.md"));
        provider.add_dir(path("project/real-dir"));
        provider.add_unsupported(path("project/socket"));

        assert_eq!(
            provider
                .link_status(
                    &path("project/source.md"),
                    &path("project/missing-link.md"),
                    LinkKind::File,
                )
                .unwrap(),
            LinkStatus::Missing
        );
        assert_eq!(
            provider
                .link_status(
                    &path("project/source.md"),
                    &path("project/correct.md"),
                    LinkKind::File,
                )
                .unwrap(),
            LinkStatus::CorrectSymlink {
                target: path("project/source.md")
            }
        );
        assert_eq!(
            provider
                .link_status(
                    &path("project/source.md"),
                    &path("project/wrong.md"),
                    LinkKind::File,
                )
                .unwrap(),
            LinkStatus::WrongSymlinkTarget {
                actual: path("project/other.md"),
                expected: path("project/source.md")
            }
        );
        assert_eq!(
            provider
                .link_status(
                    &path("project/missing-source.md"),
                    &path("project/broken.md"),
                    LinkKind::File,
                )
                .unwrap(),
            LinkStatus::BrokenSymlink {
                target: path("project/missing-source.md")
            }
        );
        assert_eq!(
            provider
                .link_status(
                    &path("project/source.md"),
                    &path("project/real-file.md"),
                    LinkKind::File,
                )
                .unwrap(),
            LinkStatus::ExistingRealFile
        );
        assert_eq!(
            provider
                .link_status(
                    &path("project/source.md"),
                    &path("project/real-dir"),
                    LinkKind::File,
                )
                .unwrap(),
            LinkStatus::ExistingRealDirectory
        );
        assert_eq!(
            provider
                .link_status(
                    &path("project/source.md"),
                    &path("project/socket"),
                    LinkKind::File,
                )
                .unwrap(),
            LinkStatus::UnsupportedFileType {
                path: path("project/socket")
            }
        );
    }

    #[test]
    fn mock_create_read_and_remove_are_safe() {
        let mut provider = base_mock();
        provider.add_file(path("project/source.md"));

        provider
            .create_symlink(
                &path("project/source.md"),
                &path("project/link.md"),
                LinkKind::File,
            )
            .unwrap();

        assert_eq!(
            provider.read_link(&path("project/link.md")).unwrap(),
            path("project/source.md")
        );

        let duplicate = provider
            .create_symlink(
                &path("project/source.md"),
                &path("project/link.md"),
                LinkKind::File,
            )
            .unwrap_err();
        assert_eq!(duplicate.kind(), SymlinkErrorKind::LinkAlreadyExists);

        assert_eq!(
            provider.remove_symlink(&path("project/link.md")).unwrap(),
            RemoveSymlinkOutcome::Removed
        );
        assert_eq!(
            provider.remove_symlink(&path("project/link.md")).unwrap(),
            RemoveSymlinkOutcome::Missing
        );
    }

    #[test]
    fn mock_create_rejects_missing_source_and_parent() {
        let mut provider = base_mock();

        let missing_source = provider
            .create_symlink(
                &path("project/source.md"),
                &path("project/link.md"),
                LinkKind::File,
            )
            .unwrap_err();
        assert_eq!(missing_source.kind(), SymlinkErrorKind::SourceNotFound);

        provider.add_file(path("project/source.md"));
        let missing_parent = provider
            .create_symlink(
                &path("project/source.md"),
                &path("missing-parent/link.md"),
                LinkKind::File,
            )
            .unwrap_err();
        assert_eq!(missing_parent.kind(), SymlinkErrorKind::LinkParentNotFound);
    }

    #[test]
    fn ensure_symlink_creates_then_skips_correct_link() {
        let mut provider = base_mock();
        provider.add_file(path("project/source.md"));

        let created = ensure_symlink(
            &mut provider,
            &path("project/source.md"),
            &path("project/link.md"),
            LinkKind::File,
            CreateSymlinkOptions::new(),
        )
        .unwrap();
        assert_eq!(created, CreateSymlinkOutcome::Created);

        let skipped = ensure_symlink(
            &mut provider,
            &path("project/source.md"),
            &path("project/link.md"),
            LinkKind::File,
            CreateSymlinkOptions::new(),
        )
        .unwrap();
        assert_eq!(skipped, CreateSymlinkOutcome::AlreadyCorrect);
    }

    #[test]
    fn ensure_symlink_rejects_wrong_target_without_force() {
        let mut provider = base_mock();
        provider.add_file(path("project/source.md"));
        provider.add_file(path("project/other.md"));
        provider.add_symlink(
            path("project/link.md"),
            path("project/other.md"),
            LinkKind::File,
        );

        let error = ensure_symlink(
            &mut provider,
            &path("project/source.md"),
            &path("project/link.md"),
            LinkKind::File,
            CreateSymlinkOptions::new(),
        )
        .unwrap_err();

        assert_eq!(error.kind(), SymlinkErrorKind::WrongSymlinkTarget);
        assert_eq!(
            provider.read_link(&path("project/link.md")).unwrap(),
            path("project/other.md")
        );
    }

    #[test]
    fn force_replaces_wrong_symlink_only() {
        let mut provider = base_mock();
        provider.add_file(path("project/source.md"));
        provider.add_file(path("project/other.md"));
        provider.add_symlink(
            path("project/link.md"),
            path("project/other.md"),
            LinkKind::File,
        );

        let outcome = ensure_symlink(
            &mut provider,
            &path("project/source.md"),
            &path("project/link.md"),
            LinkKind::File,
            CreateSymlinkOptions::force_wrong_symlink(),
        )
        .unwrap();

        assert_eq!(outcome, CreateSymlinkOutcome::ReplacedWrongSymlink);
        assert_eq!(
            provider.read_link(&path("project/link.md")).unwrap(),
            path("project/source.md")
        );
    }

    #[test]
    fn force_never_deletes_real_file_or_directory() {
        let mut file_provider = base_mock();
        file_provider.add_file(path("project/source.md"));
        file_provider.add_file(path("project/link.md"));

        let file_error = ensure_symlink(
            &mut file_provider,
            &path("project/source.md"),
            &path("project/link.md"),
            LinkKind::File,
            CreateSymlinkOptions::force_wrong_symlink(),
        )
        .unwrap_err();
        assert_eq!(file_error.kind(), SymlinkErrorKind::ExistingRealFile);
        assert_eq!(
            file_provider.entry(&path("project/link.md")),
            Some(&MockEntry::File)
        );

        let mut dir_provider = base_mock();
        dir_provider.add_file(path("project/source.md"));
        dir_provider.add_dir(path("project/link.md"));

        let dir_error = ensure_symlink(
            &mut dir_provider,
            &path("project/source.md"),
            &path("project/link.md"),
            LinkKind::File,
            CreateSymlinkOptions::force_wrong_symlink(),
        )
        .unwrap_err();
        assert_eq!(dir_error.kind(), SymlinkErrorKind::ExistingRealDirectory);
        assert_eq!(
            dir_provider.entry(&path("project/link.md")),
            Some(&MockEntry::Directory)
        );
    }

    #[test]
    fn broken_symlink_to_expected_source_reports_source_not_found() {
        let mut provider = base_mock();
        provider.add_symlink(
            path("project/link.md"),
            path("project/source.md"),
            LinkKind::File,
        );

        let error = ensure_symlink(
            &mut provider,
            &path("project/source.md"),
            &path("project/link.md"),
            LinkKind::File,
            CreateSymlinkOptions::force_wrong_symlink(),
        )
        .unwrap_err();

        assert_eq!(error.kind(), SymlinkErrorKind::SourceNotFound);
        assert_eq!(
            provider.read_link(&path("project/link.md")).unwrap(),
            path("project/source.md")
        );
    }

    #[test]
    fn missing_wrong_target_is_wrong_symlink_not_source_not_found() {
        let mut provider = base_mock();
        provider.add_file(path("project/source.md"));
        provider.add_symlink(
            path("project/link.md"),
            path("project/missing-other.md"),
            LinkKind::File,
        );

        let status = provider
            .link_status(
                &path("project/source.md"),
                &path("project/link.md"),
                LinkKind::File,
            )
            .unwrap();

        assert_eq!(
            status,
            LinkStatus::WrongSymlinkTarget {
                actual: path("project/missing-other.md"),
                expected: path("project/source.md")
            }
        );
    }

    #[test]
    fn mock_remove_refuses_real_paths() {
        let mut provider = base_mock();
        provider.add_file(path("project/real-file.md"));
        provider.add_dir(path("project/real-dir"));

        let file_error = provider
            .remove_symlink(&path("project/real-file.md"))
            .unwrap_err();
        assert_eq!(file_error.kind(), SymlinkErrorKind::ExistingRealFile);

        let dir_error = provider
            .remove_symlink(&path("project/real-dir"))
            .unwrap_err();
        assert_eq!(dir_error.kind(), SymlinkErrorKind::ExistingRealDirectory);
    }

    #[cfg(windows)]
    #[test]
    fn default_provider_uses_windows_broker() {
        assert_eq!(default_provider().backend(), SymlinkBackend::WindowsBroker);
        assert_eq!(
            StdSymlinkProvider::explicit_windows_fallback().backend(),
            SymlinkBackend::WindowsStdFallback
        );
    }

    #[cfg(windows)]
    #[test]
    fn broker_error_mapping_preserves_stable_codes() {
        let cases = [
            (
                ErrorCode::ServiceNotInstalled,
                SymlinkErrorKind::BrokerUnavailable,
            ),
            (
                ErrorCode::ServiceUnavailable,
                SymlinkErrorKind::BrokerUnavailable,
            ),
            (
                ErrorCode::ServiceIdentityMismatch,
                SymlinkErrorKind::BrokerProtocolError,
            ),
            (
                ErrorCode::PrivilegeRequired,
                SymlinkErrorKind::PermissionDenied,
            ),
            (
                ErrorCode::CallerParentWriteDenied,
                SymlinkErrorKind::PermissionDenied,
            ),
            (
                ErrorCode::LinkAlreadyExists,
                SymlinkErrorKind::LinkAlreadyExists,
            ),
            (
                ErrorCode::LinkPathIsNotSymlink,
                SymlinkErrorKind::WrongSymlinkTarget,
            ),
            (
                ErrorCode::TargetKindConflict,
                SymlinkErrorKind::UnsupportedLinkKind,
            ),
            (ErrorCode::CreateSymlinkFailed, SymlinkErrorKind::Io),
        ];

        for (broker_code, expected_kind) in cases {
            let error =
                broker_error_to_symlink_error(WinSymlinksError::new(broker_code, "broker failure"));

            assert_eq!(error.kind(), expected_kind);
            assert_eq!(error.backend(), SymlinkBackend::WindowsBroker);
            assert_eq!(error.broker_code(), Some(broker_code.to_string().as_str()));
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn default_provider_uses_std_backend_off_windows() {
        assert_eq!(default_provider().backend(), SymlinkBackend::Std);
    }

    #[cfg(unix)]
    #[test]
    fn std_provider_uses_unix_symlinks() {
        let temp_dir = TestDir::new();
        let source = temp_dir.path().join("source.txt");
        let link = temp_dir.path().join("link.txt");
        fs::write(&source, "source").unwrap();

        let mut provider = StdSymlinkProvider::new();
        provider
            .create_symlink(&source, &link, LinkKind::File)
            .unwrap();

        assert_eq!(provider.read_link(&link).unwrap(), source);
        assert!(matches!(
            provider
                .link_status(&source, &link, LinkKind::File)
                .unwrap(),
            LinkStatus::CorrectSymlink { .. }
        ));
        assert_eq!(
            provider.remove_symlink(&link).unwrap(),
            RemoveSymlinkOutcome::Removed
        );
        assert_eq!(
            provider
                .link_status(&source, &link, LinkKind::File)
                .unwrap(),
            LinkStatus::Missing
        );
    }

    #[cfg(unix)]
    struct TestDir {
        path: PathBuf,
    }

    #[cfg(unix)]
    impl TestDir {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "agent-linker-symlink-test-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    #[cfg(unix)]
    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
