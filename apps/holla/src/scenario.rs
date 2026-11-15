use junie_tui::clock::SimClock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scenario {
    FirstUse,

    RustDirty,

    MonorepoRoot,

    MonorepoChild,

    DockerCleanup,

    DiskCleanup,

    UpgradePlan,

    ActivitiesMulti,

    RemoteHost,

    LaunchFailure,

    HardCases,

    ParityDiscovery,
    ParityHistory,
    ParityFiles,
    ParityBrowser,
    ParityGitCurrent,
    ParityGitBatch,
    ParityTaskSources,
    ParityCargo,
    ParityDocker,
    ParityBrewServices,
    ParityGradle,
    ParityIdea,
    ParityUpgradeManagers,
    ParityExecutor,
    ParityTaskInput,
    ParityCustomActions,
    ParityDiskScan,
    ParityDiskNavigation,
    ParityInsights,
    ParityDeleteSafety,
    ParityCleanupResults,
    ParityPlatforms,
    ParityPlatformsLinux,
}

impl Scenario {
    pub const CONCEPT: [Scenario; 11] = [
        Scenario::FirstUse,
        Scenario::RustDirty,
        Scenario::MonorepoRoot,
        Scenario::MonorepoChild,
        Scenario::DockerCleanup,
        Scenario::DiskCleanup,
        Scenario::UpgradePlan,
        Scenario::ActivitiesMulti,
        Scenario::RemoteHost,
        Scenario::LaunchFailure,
        Scenario::HardCases,
    ];

    pub const PARITY: [Scenario; 23] = [
        Scenario::ParityDiscovery,
        Scenario::ParityHistory,
        Scenario::ParityFiles,
        Scenario::ParityBrowser,
        Scenario::ParityGitCurrent,
        Scenario::ParityGitBatch,
        Scenario::ParityTaskSources,
        Scenario::ParityCargo,
        Scenario::ParityDocker,
        Scenario::ParityBrewServices,
        Scenario::ParityGradle,
        Scenario::ParityIdea,
        Scenario::ParityUpgradeManagers,
        Scenario::ParityExecutor,
        Scenario::ParityTaskInput,
        Scenario::ParityCustomActions,
        Scenario::ParityDiskScan,
        Scenario::ParityDiskNavigation,
        Scenario::ParityInsights,
        Scenario::ParityDeleteSafety,
        Scenario::ParityCleanupResults,
        Scenario::ParityPlatforms,
        Scenario::ParityPlatformsLinux,
    ];

    pub const ALL: [Scenario; 34] = [
        Scenario::FirstUse,
        Scenario::RustDirty,
        Scenario::MonorepoRoot,
        Scenario::MonorepoChild,
        Scenario::DockerCleanup,
        Scenario::DiskCleanup,
        Scenario::UpgradePlan,
        Scenario::ActivitiesMulti,
        Scenario::RemoteHost,
        Scenario::LaunchFailure,
        Scenario::HardCases,
        Scenario::ParityDiscovery,
        Scenario::ParityHistory,
        Scenario::ParityFiles,
        Scenario::ParityBrowser,
        Scenario::ParityGitCurrent,
        Scenario::ParityGitBatch,
        Scenario::ParityTaskSources,
        Scenario::ParityCargo,
        Scenario::ParityDocker,
        Scenario::ParityBrewServices,
        Scenario::ParityGradle,
        Scenario::ParityIdea,
        Scenario::ParityUpgradeManagers,
        Scenario::ParityExecutor,
        Scenario::ParityTaskInput,
        Scenario::ParityCustomActions,
        Scenario::ParityDiskScan,
        Scenario::ParityDiskNavigation,
        Scenario::ParityInsights,
        Scenario::ParityDeleteSafety,
        Scenario::ParityCleanupResults,
        Scenario::ParityPlatforms,
        Scenario::ParityPlatformsLinux,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Scenario::FirstUse => "first-use",
            Scenario::RustDirty => "rust-dirty",
            Scenario::MonorepoRoot => "monorepo-root",
            Scenario::MonorepoChild => "monorepo-child",
            Scenario::DockerCleanup => "docker-cleanup",
            Scenario::DiskCleanup => "disk-cleanup",
            Scenario::UpgradePlan => "upgrade-plan",
            Scenario::ActivitiesMulti => "activities-multi",
            Scenario::RemoteHost => "remote-host",
            Scenario::LaunchFailure => "launch-failure",
            Scenario::HardCases => "hard-cases",
            Scenario::ParityDiscovery => "parity-discovery",
            Scenario::ParityHistory => "parity-history",
            Scenario::ParityFiles => "parity-files",
            Scenario::ParityBrowser => "parity-browser",
            Scenario::ParityGitCurrent => "parity-git-current",
            Scenario::ParityGitBatch => "parity-git-batch",
            Scenario::ParityTaskSources => "parity-task-sources",
            Scenario::ParityCargo => "parity-cargo",
            Scenario::ParityDocker => "parity-docker",
            Scenario::ParityBrewServices => "parity-brew-services",
            Scenario::ParityGradle => "parity-gradle",
            Scenario::ParityIdea => "parity-idea",
            Scenario::ParityUpgradeManagers => "parity-upgrade-managers",
            Scenario::ParityExecutor => "parity-executor",
            Scenario::ParityTaskInput => "parity-task-input",
            Scenario::ParityCustomActions => "parity-custom-actions",
            Scenario::ParityDiskScan => "parity-disk-scan",
            Scenario::ParityDiskNavigation => "parity-disk-navigation",
            Scenario::ParityInsights => "parity-insights",
            Scenario::ParityDeleteSafety => "parity-delete-safety",
            Scenario::ParityCleanupResults => "parity-cleanup-results",
            Scenario::ParityPlatforms => "parity-platforms",
            Scenario::ParityPlatformsLinux => "parity-platforms-linux",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|sc| sc.name() == s)
    }
}

pub const EPOCH_SECS: i64 = 1_789_004_460;

pub const UTC_OFFSET_SECS: i64 = 7 * 3600;

pub const CLOCK: SimClock = SimClock::new(EPOCH_SECS, UTC_OFFSET_SECS);

pub use junie_tui::motion::Motion;
