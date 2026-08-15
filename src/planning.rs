//! Deciding what should be renamed, without touching anything.
//!
//! Planning is deliberately free of side effects beyond reading directory
//! entries: it answers "what would happen", and [`crate::applying`] answers "make
//! it happen". Matching is scoped per directory — a subtitle is only ever paired
//! with a video sitting beside it.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::names::{episode_key, language_tag, normalize_stem};
use crate::similarity::ratio;

pub const VIDEO_EXTS_DEFAULT: &[&str] = &["mkv", "mp4", "avi", "mov", "wmv", "m4v", "webm"];
pub const SUB_EXTS_DEFAULT: &[&str] = &["ass", "srt", "ssa", "vtt", "sub"];

/// How far ahead of the runner-up the best fuzzy match has to be.
///
/// Two videos that score almost the same against one subtitle mean the filenames
/// do not actually say which is which, so neither is chosen.
const MIN_SCORE_MARGIN: f64 = 0.06;

/// Why a subtitle was paired with a video.
#[derive(Clone, Debug, PartialEq)]
pub enum MatchReason {
    /// Both filenames carry the same episode id, which settles it.
    Episode(String),
    /// The filename stems are similar enough, with the score that decided it.
    Fuzzy(f64),
}

/// Why a subtitle was left alone.
#[derive(Clone, Debug, PartialEq)]
pub enum SkipReason {
    /// No video scored high enough, or two scored too close together.
    Unmatched { best_score: f64 },
    /// Several videos in the folder claim the same episode id.
    AmbiguousEpisode(String),
    /// The subtitle is already named after its video.
    AlreadyMatches,
    /// The exact target name is taken and strict mode forbids a suffix.
    StrictCollision,
    /// The folder holds subtitles but no videos.
    NoVideo,
    /// The target name is taken and no free variant could be found.
    Collision,
}

#[derive(Clone, Debug)]
pub struct RenameOp {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub reason: MatchReason,
}

#[derive(Clone, Debug)]
pub struct SkippedRename {
    pub path: PathBuf,
    pub reason: SkipReason,
}

/// Everything a preview needs to describe one run.
#[derive(Clone, Debug)]
pub struct RenamePlan {
    pub root: PathBuf,
    pub operations: Vec<RenameOp>,
    pub skipped: Vec<SkippedRename>,
    pub video_count: usize,
    pub subtitle_count: usize,
    pub directory_count: usize,
}

#[derive(Clone, Debug)]
pub struct PlanOptions {
    pub recursive: bool,
    /// Refuse any subtitle that cannot take the plain `VideoName.ext` form.
    pub strict: bool,
    /// Fuzzy threshold in `0.0..=1.0`; episode-id matches ignore it.
    pub min_score: f64,
    pub video_exts: Vec<String>,
    pub sub_exts: Vec<String>,
}

impl Default for PlanOptions {
    fn default() -> Self {
        Self {
            recursive: false,
            strict: false,
            min_score: crate::presentation::MatchLevel::default().score(),
            video_exts: VIDEO_EXTS_DEFAULT
                .iter()
                .map(|ext| ext.to_string())
                .collect(),
            sub_exts: SUB_EXTS_DEFAULT.iter().map(|ext| ext.to_string()).collect(),
        }
    }
}

#[derive(Debug)]
pub enum PlanError {
    NotADirectory(PathBuf),
    Io(std::io::Error),
}

impl fmt::Display for PlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotADirectory(path) => write!(formatter, "not a directory: {}", path.display()),
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for PlanError {}

impl From<std::io::Error> for PlanError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Accept extensions with or without a leading dot, in any case.
pub fn normalize_extension(raw: &str) -> String {
    raw.trim().trim_start_matches('.').to_lowercase()
}

/// A file worth considering, with the parts matching needs precomputed.
#[derive(Clone, Debug)]
struct Candidate {
    path: PathBuf,
    stem_norm: String,
    episode_key: Option<String>,
}

impl Candidate {
    fn new(path: PathBuf) -> Self {
        let stem = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self {
            stem_norm: normalize_stem(&stem),
            episode_key: episode_key(&name),
            path,
        }
    }
}

/// Sort key that keeps output stable regardless of directory iteration order.
fn sort_key(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}

/// Plan the renames for a real directory on disk. Reads, never writes.
pub fn plan_directory(root: &Path, options: &PlanOptions) -> Result<RenamePlan, PlanError> {
    let root = crate::paths::resolve(root);
    if !root.is_dir() {
        return Err(PlanError::NotADirectory(root));
    }

    let video_exts: HashSet<String> = options
        .video_exts
        .iter()
        .map(|e| normalize_extension(e))
        .collect();
    let sub_exts: HashSet<String> = options
        .sub_exts
        .iter()
        .map(|e| normalize_extension(e))
        .collect();

    let mut videos_by_directory: HashMap<PathBuf, Vec<Candidate>> = HashMap::new();
    let mut subtitles_by_directory: HashMap<PathBuf, Vec<Candidate>> = HashMap::new();

    for path in collect_files(&root, options.recursive)? {
        let Some(extension) = path.extension() else {
            continue;
        };
        let extension = extension.to_string_lossy().to_lowercase();
        let parent = path.parent().unwrap_or(&root).to_path_buf();
        if video_exts.contains(&extension) {
            videos_by_directory
                .entry(parent)
                .or_default()
                .push(Candidate::new(path));
        } else if sub_exts.contains(&extension) {
            subtitles_by_directory
                .entry(parent)
                .or_default()
                .push(Candidate::new(path));
        }
    }

    Ok(create_plan(
        root,
        videos_by_directory,
        subtitles_by_directory,
        options,
        &|path: &Path| path.exists(),
    ))
}

/// Plan renames for a made-up file listing, used by the demo mode.
///
/// Nothing here touches the filesystem: "does this target already exist" is
/// answered from the listing itself.
pub fn plan_virtual_files(file_names: &[&str], options: &PlanOptions) -> RenamePlan {
    let root = PathBuf::from("/virtual-subtitle-library");
    let video_exts: HashSet<String> = options
        .video_exts
        .iter()
        .map(|e| normalize_extension(e))
        .collect();
    let sub_exts: HashSet<String> = options
        .sub_exts
        .iter()
        .map(|e| normalize_extension(e))
        .collect();

    let mut videos_by_directory: HashMap<PathBuf, Vec<Candidate>> = HashMap::new();
    let mut subtitles_by_directory: HashMap<PathBuf, Vec<Candidate>> = HashMap::new();
    let mut existing: HashSet<PathBuf> = HashSet::new();

    for name in file_names {
        let path = root.join(name);
        existing.insert(path.clone());
        let Some(extension) = path.extension() else {
            continue;
        };
        let extension = extension.to_string_lossy().to_lowercase();
        let parent = path.parent().unwrap_or(&root).to_path_buf();
        if video_exts.contains(&extension) {
            videos_by_directory
                .entry(parent)
                .or_default()
                .push(Candidate::new(path));
        } else if sub_exts.contains(&extension) {
            subtitles_by_directory
                .entry(parent)
                .or_default()
                .push(Candidate::new(path));
        }
    }

    create_plan(
        root,
        videos_by_directory,
        subtitles_by_directory,
        options,
        &move |path: &Path| existing.contains(path),
    )
}

/// List the files under `root`, optionally descending into subdirectories.
///
/// Symlinked directories are not followed, so a loop cannot hang a scan.
/// Unreadable subdirectories are skipped rather than failing the whole run.
fn collect_files(root: &Path, recursive: bool) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            // The root itself must be readable; anything below it may not be.
            Err(error) if directory == root => return Err(error),
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            match entry.file_type() {
                Ok(file_type) if file_type.is_dir() => {
                    if recursive {
                        directories.push(path);
                    }
                }
                // Follows symlinks, so a link to a video counts as one.
                _ if path.is_file() => files.push(path),
                _ => {}
            }
        }
    }
    Ok(files)
}

fn create_plan(
    root: PathBuf,
    mut videos_by_directory: HashMap<PathBuf, Vec<Candidate>>,
    mut subtitles_by_directory: HashMap<PathBuf, Vec<Candidate>>,
    options: &PlanOptions,
    path_exists: &dyn Fn(&Path) -> bool,
) -> RenamePlan {
    for candidates in videos_by_directory
        .values_mut()
        .chain(subtitles_by_directory.values_mut())
    {
        candidates.sort_by_key(|candidate| sort_key(&candidate.path));
    }

    let mut directories: Vec<PathBuf> = videos_by_directory
        .keys()
        .chain(subtitles_by_directory.keys())
        .cloned()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    directories.sort_by_key(|directory| sort_key(directory));

    let video_count = videos_by_directory.values().map(Vec::len).sum();
    let subtitle_count = subtitles_by_directory.values().map(Vec::len).sum();
    let directory_count = directories.len();

    let mut operations = Vec::new();
    let mut skipped = Vec::new();
    let empty: Vec<Candidate> = Vec::new();

    for directory in &directories {
        let subtitles = subtitles_by_directory.get(directory).unwrap_or(&empty);
        if subtitles.is_empty() {
            continue;
        }
        let videos = videos_by_directory.get(directory).unwrap_or(&empty);
        if videos.is_empty() {
            skipped.extend(subtitles.iter().map(|subtitle| SkippedRename {
                path: subtitle.path.clone(),
                reason: SkipReason::NoVideo,
            }));
            continue;
        }
        build_directory_plan(
            subtitles,
            videos,
            options,
            path_exists,
            &mut operations,
            &mut skipped,
        );
    }

    RenamePlan {
        root,
        operations,
        skipped,
        video_count,
        subtitle_count,
        directory_count,
    }
}

fn build_directory_plan(
    subtitles: &[Candidate],
    videos: &[Candidate],
    options: &PlanOptions,
    path_exists: &dyn Fn(&Path) -> bool,
    operations: &mut Vec<RenameOp>,
    skipped: &mut Vec<SkippedRename>,
) {
    // An episode id shared by two videos identifies neither, so both drop out of
    // the index and any subtitle carrying that id is reported as ambiguous.
    let mut videos_by_episode: HashMap<&str, &Candidate> = HashMap::new();
    let mut ambiguous: HashSet<&str> = HashSet::new();
    for video in videos {
        let Some(key) = video.episode_key.as_deref() else {
            continue;
        };
        if videos_by_episode.insert(key, video).is_some() {
            ambiguous.insert(key);
        }
    }
    for key in &ambiguous {
        videos_by_episode.remove(key);
    }

    let mut planned: HashSet<PathBuf> = HashSet::new();

    for subtitle in subtitles {
        let episode = subtitle.episode_key.as_deref();
        if let Some(key) = episode {
            if ambiguous.contains(key) {
                skipped.push(SkippedRename {
                    path: subtitle.path.clone(),
                    reason: SkipReason::AmbiguousEpisode(key.to_string()),
                });
                continue;
            }
        }

        let by_episode = episode.and_then(|key| {
            videos_by_episode
                .get(key)
                .map(|video| (*video, MatchReason::Episode(key.to_string())))
        });
        let (video, reason) = match by_episode {
            Some(matched) => matched,
            None => match choose_unique_best(subtitle, videos, options.min_score) {
                (Some(video), score) => (video, MatchReason::Fuzzy(score)),
                (None, best_score) => {
                    skipped.push(SkippedRename {
                        path: subtitle.path.clone(),
                        reason: SkipReason::Unmatched { best_score },
                    });
                    continue;
                }
            },
        };

        let Some(destination) =
            choose_destination(subtitle, video, options.strict, path_exists, &planned)
        else {
            skipped.push(SkippedRename {
                path: subtitle.path.clone(),
                reason: if options.strict {
                    SkipReason::StrictCollision
                } else {
                    SkipReason::Collision
                },
            });
            continue;
        };

        if destination == subtitle.path {
            skipped.push(SkippedRename {
                path: subtitle.path.clone(),
                reason: SkipReason::AlreadyMatches,
            });
            continue;
        }

        planned.insert(destination.clone());
        operations.push(RenameOp {
            source: subtitle.path.clone(),
            destination,
            reason,
        });
    }
}

/// Work out where a subtitle should go, or `None` if every name is taken.
///
/// The plain `VideoName.ext` form is preferred; when it is occupied, a language
/// tag from the subtitle's own name is tried before falling back to a number.
/// Strict mode stops after the first form.
fn choose_destination(
    subtitle: &Candidate,
    video: &Candidate,
    strict: bool,
    path_exists: &dyn Fn(&Path) -> bool,
    planned: &HashSet<PathBuf>,
) -> Option<PathBuf> {
    let extension = subtitle
        .path
        .extension()
        .map(|e| e.to_string_lossy().into_owned())?;
    let directory = video.path.parent()?;
    let video_stem = video.path.file_stem()?.to_string_lossy().into_owned();

    let base = directory.join(format!("{video_stem}.{extension}"));
    // Renaming a file onto its own name is not a collision; the caller reports
    // that case as "already matches".
    if base == subtitle.path {
        return Some(base);
    }
    let taken = |candidate: &Path| path_exists(candidate) || planned.contains(candidate);
    if !taken(&base) {
        return Some(base);
    }
    if strict {
        return None;
    }

    if let Some(tag) = language_tag(&subtitle.path.file_stem()?.to_string_lossy()) {
        let tagged = directory.join(format!("{video_stem}.{tag}.{extension}"));
        if tagged == subtitle.path {
            return Some(tagged);
        }
        if !taken(&tagged) {
            return Some(tagged);
        }
    }

    // Bounded so a pathological directory cannot spin here forever.
    for number in 2..1000 {
        let numbered = directory.join(format!("{video_stem}.{number}.{extension}"));
        if numbered == subtitle.path {
            return Some(numbered);
        }
        if !taken(&numbered) {
            return Some(numbered);
        }
    }
    None
}

/// Pick the one video that clearly fits `subtitle`, with its score.
///
/// Returns the best score even when nothing is chosen, so the preview can show
/// how close the near miss was.
fn choose_unique_best<'a>(
    subtitle: &Candidate,
    videos: &'a [Candidate],
    min_score: f64,
) -> (Option<&'a Candidate>, f64) {
    if subtitle.stem_norm.is_empty() {
        return (None, 0.0);
    }

    let mut scored: Vec<(f64, &Candidate)> = videos
        .iter()
        .filter(|video| !video.stem_norm.is_empty())
        .map(|video| (ratio(&subtitle.stem_norm, &video.stem_norm), video))
        .collect();
    // Stable, so equal scores keep directory order and the choice is repeatable.
    scored.sort_by(|left, right| right.0.total_cmp(&left.0));

    let Some(&(best_score, best)) = scored.first() else {
        return (None, 0.0);
    };
    if best_score < min_score {
        return (None, best_score);
    }
    let runner_up = scored.get(1).map_or(0.0, |entry| entry.0);
    if best_score - runner_up < MIN_SCORE_MARGIN {
        return (None, best_score);
    }
    (Some(best), best_score)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(names: &[&str]) -> RenamePlan {
        plan_virtual_files(names, &PlanOptions::default())
    }

    fn plan_with(names: &[&str], options: PlanOptions) -> RenamePlan {
        plan_virtual_files(names, &options)
    }

    fn destinations(plan: &RenamePlan) -> Vec<String> {
        plan.operations
            .iter()
            .map(|operation| {
                operation
                    .destination
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    }

    #[test]
    fn matches_on_episode_id() {
        let plan = plan(&[
            "Nebula.S01E01.1080p.mkv",
            "some.other.release.S01E01.chs.ass",
        ]);
        assert_eq!(destinations(&plan), ["Nebula.S01E01.1080p.ass"]);
        assert_eq!(
            plan.operations[0].reason,
            MatchReason::Episode("S01E01".into())
        );
        assert_eq!(plan.video_count, 1);
        assert_eq!(plan.subtitle_count, 1);
        assert_eq!(plan.directory_count, 1);
    }

    #[test]
    fn matches_on_stem_similarity_when_no_episode_id() {
        let plan = plan(&["Deep Field Report.mkv", "Deep Field Report.eng.srt"]);
        assert_eq!(destinations(&plan), ["Deep Field Report.srt"]);
        assert!(matches!(plan.operations[0].reason, MatchReason::Fuzzy(score) if score > 0.9));
    }

    #[test]
    fn refuses_a_fuzzy_match_that_is_too_close_to_the_runner_up() {
        let plan = plan(&["Report A.mkv", "Report B.mkv", "Report C.srt"]);
        assert!(plan.operations.is_empty());
        assert!(matches!(
            plan.skipped[0].reason,
            SkipReason::Unmatched { .. }
        ));
    }

    #[test]
    fn reports_an_episode_id_claimed_by_two_videos() {
        let plan = plan(&["A.S01E01.mkv", "B.S01E01.mkv", "Subs.S01E01.srt"]);
        assert!(plan.operations.is_empty());
        assert_eq!(
            plan.skipped[0].reason,
            SkipReason::AmbiguousEpisode("S01E01".into())
        );
    }

    #[test]
    fn leaves_a_subtitle_that_already_matches() {
        let plan = plan(&["Nebula.S01E01.mkv", "Nebula.S01E01.srt"]);
        assert!(plan.operations.is_empty());
        assert_eq!(plan.skipped[0].reason, SkipReason::AlreadyMatches);
    }

    #[test]
    fn separates_two_subtitles_for_one_video_by_language() {
        let plan = plan(&[
            "Nebula.S01E01.mkv",
            "Deep.Release.S01E01.chs.srt",
            "Other.Release.S01E01.eng.srt",
        ]);
        let mut destinations = destinations(&plan);
        destinations.sort();
        assert_eq!(destinations, ["Nebula.S01E01.eng.srt", "Nebula.S01E01.srt"]);
    }

    #[test]
    fn leaves_a_subtitle_that_is_already_at_a_language_tagged_name() {
        // The plain name is taken, and this file already sits at the name it
        // would be given, so there is nothing to do.
        let plan = plan(&[
            "Nebula.S01E01.mkv",
            "Nebula.S01E01.srt",
            "Nebula.S01E01.eng.srt",
        ]);
        assert!(plan.operations.is_empty());
        assert!(plan
            .skipped
            .iter()
            .all(|skipped| skipped.reason == SkipReason::AlreadyMatches));
    }

    #[test]
    fn falls_back_to_a_number_when_the_language_name_is_taken_too() {
        let plan = plan(&[
            "Nebula.S01E01.mkv",
            "Nebula.S01E01.chs.srt",
            "Other.Release.S01E01.chs.srt",
            "Third.Release.S01E01.chs.srt",
        ]);
        let mut destinations = destinations(&plan);
        destinations.sort();
        assert_eq!(
            destinations,
            [
                "Nebula.S01E01.2.srt",
                "Nebula.S01E01.3.srt",
                "Nebula.S01E01.srt"
            ]
        );
    }

    #[test]
    fn strict_mode_skips_a_collision_instead_of_adding_a_suffix() {
        let options = PlanOptions {
            strict: true,
            ..PlanOptions::default()
        };
        let plan = plan_with(
            &[
                "Nebula.S01E01.mkv",
                "Nebula.S01E01.chs.srt",
                "Nebula.S01E01.eng.srt",
            ],
            options,
        );
        assert_eq!(plan.operations.len(), 1);
        assert_eq!(plan.skipped[0].reason, SkipReason::StrictCollision);
    }

    #[test]
    fn reports_a_folder_with_subtitles_but_no_videos() {
        let plan = plan(&["lonely/Nebula.S01E01.srt"]);
        assert_eq!(plan.skipped[0].reason, SkipReason::NoVideo);
    }

    #[test]
    fn never_matches_across_directories() {
        let plan = plan(&["a/Nebula.S01E01.mkv", "b/Nebula.S01E01.srt"]);
        assert!(plan.operations.is_empty());
        assert_eq!(plan.skipped[0].reason, SkipReason::NoVideo);
        assert_eq!(plan.directory_count, 2);
    }

    #[test]
    fn a_cautious_level_rejects_what_a_relaxed_one_accepts() {
        let names = &["Deep Field Report 2031.mkv", "Deep Feild Raport.srt"];
        let relaxed = plan_with(
            names,
            PlanOptions {
                min_score: 0.6,
                ..PlanOptions::default()
            },
        );
        let cautious = plan_with(
            names,
            PlanOptions {
                min_score: 0.95,
                ..PlanOptions::default()
            },
        );
        assert_eq!(relaxed.operations.len(), 1);
        assert!(cautious.operations.is_empty());
    }

    #[test]
    fn rejects_a_directory_that_does_not_exist() {
        let error = plan_directory(Path::new("/no/such/directory"), &PlanOptions::default());
        assert!(error.is_err());
    }

    #[test]
    fn scans_a_real_directory() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        std::fs::write(root.join("Nebula.S01E01.1080p.mkv"), b"").unwrap();
        std::fs::write(root.join("random.name.S01E01.chs.ass"), b"").unwrap();
        std::fs::create_dir(root.join("season2")).unwrap();
        std::fs::write(root.join("season2/Nebula.S02E01.mkv"), b"").unwrap();
        std::fs::write(root.join("season2/Nebula.S02E01.srt"), b"").unwrap();

        let flat = plan_directory(root, &PlanOptions::default()).unwrap();
        assert_eq!(flat.operations.len(), 1);
        assert_eq!(flat.video_count, 1);

        let deep = plan_directory(
            root,
            &PlanOptions {
                recursive: true,
                ..PlanOptions::default()
            },
        )
        .unwrap();
        assert_eq!(deep.video_count, 2);
        assert_eq!(deep.directory_count, 2);
        // The nested subtitle already matches, so only the top-level one moves.
        assert_eq!(deep.operations.len(), 1);
    }
}
