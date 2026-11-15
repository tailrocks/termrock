#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    DockerRestart(String),
    DockerStop(String),

    DockerStopAll,
    DockerRemoveAll,
    DockerImagesPrune,
    DockerNetworkPrune,
    DockerVolumePrune,
    DockerBuilderPrune,
    ComposeUp,
    ComposeDown,
    ServiceRestart(String),
    PgCancel(u32),
    PgTerminate(u32),

    GitPull(String),

    GitPullMerge(String),
    GitPush(String),

    GitPushRemote(String, String),
    GitFetch(String),
    GitSwitch(String, String),
    GitGc(String),
    GitDeleteBranches(String, Vec<String>),

    CargoClean(String),
    MiseInstall,
    MiseUpgrade,
    BrewService(String, String),

    BrewUpgrade(bool),
    AmpUpdate,
    OmzUpgrade,
    GradleStop,

    GradleClean(String),
    KillProcess(u32),

    DeleteAll(String),
}
