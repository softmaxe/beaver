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
use std::sync::{Condvar, Mutex};

use crate::names::{episode_key, language_tag, normalize_stem};
use crate::paths::sort_key;
use crate::similarity::{CharCounts, Scratch, Target};

pub const VIDEO_EXTS_DEFAULT: &[&str] = &["mkv", "mp4", "avi", "mov", "wmv", "m4v", "webm"];
pub const SUB_EXTS_DEFAULT: &[&str] = &["ass", "srt", "ssa", "vtt", "sub"];
pub(crate) const RELAXED_MIN_SCORE: f64 = 0.60;
pub(crate) const BALANCED_MIN_SCORE: f64 = 0.72;
pub(crate) const CAUTIOUS_MIN_SCORE: f64 = 0.84;

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
    /// No video carries the episode id found in the subtitle filename.
    NoMatchingEpisode(String),
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
}

#[derive(Clone, Debug)]
pub struct PlanOptions {
    pub recursive: bool,
    /// Refuse any subtitle that cannot take the plain `VideoName.ext` form.
    pub strict: bool,
    /// Allow the plain target name to replace a file already on disk.
    pub overwrite_existing: bool,
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
            overwrite_existing: false,
            min_score: BALANCED_MIN_SCORE,
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
            Self::NotADirectory(path) => {
                write!(
                    formatter,
                    "not a directory: {}",
                    crate::paths::display(path)
                )
            }
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
    /// The comparable form of the stem, as characters: fuzzy matching walks it
    /// element by element, and every folder walks it once per video.
    stem_norm: Vec<char>,
    episode_key: Option<String>,
}

impl Candidate {
    fn new(path: PathBuf) -> Self {
        // `to_string_lossy` borrows for the usual case of a valid UTF-8 name, so
        // scanning a large library does not allocate a string per file here.
        let stem_norm = path
            .file_stem()
            .map(|stem| normalize_stem(&stem.to_string_lossy()))
            .unwrap_or_default();
        let episode_key = path
            .file_name()
            .and_then(|name| episode_key(&name.to_string_lossy()));
        Self {
            stem_norm: stem_norm.chars().collect(),
            episode_key,
            path,
        }
    }
}

/// One folder's worth of candidates.
///
/// Matching never crosses a folder boundary, so a group is a self-contained unit
/// of work: it can be read, sorted and planned without looking at any other.
#[derive(Debug, Default)]
struct Group {
    directory: PathBuf,
    videos: Vec<Candidate>,
    subtitles: Vec<Candidate>,
}

impl Group {
    fn is_empty(&self) -> bool {
        self.videos.is_empty() && self.subtitles.is_empty()
    }
}

/// The candidates of one folder in a stable order, without moving them.
fn ordered(candidates: &[Candidate]) -> Vec<&Candidate> {
    let mut ordered: Vec<&Candidate> = candidates.iter().collect();
    ordered.sort_by_cached_key(|candidate| sort_key(&candidate.path));
    ordered
}

/// The extension rules, normalised once instead of per file.
struct Scan {
    video_exts: Vec<String>,
    sub_exts: Vec<String>,
    recursive: bool,
}

impl Scan {
    fn new(options: &PlanOptions) -> Self {
        Self {
            video_exts: options
                .video_exts
                .iter()
                .map(|e| normalize_extension(e))
                .collect(),
            sub_exts: options
                .sub_exts
                .iter()
                .map(|e| normalize_extension(e))
                .collect(),
            recursive: options.recursive,
        }
    }

    /// Put `path` in the bucket its extension calls for, or drop it.
    fn classify_into(&self, path: PathBuf, group: &mut Group) {
        let Some(extension) = path.extension() else {
            return;
        };
        if extension_matches(&self.video_exts, extension) {
            group.videos.push(Candidate::new(path));
        } else if extension_matches(&self.sub_exts, extension) {
            group.subtitles.push(Candidate::new(path));
        }
    }

    /// Read one directory into a group, returning the subdirectories to descend
    /// into (empty unless the scan is recursive).
    ///
    /// Symlinked directories are not followed, so a loop cannot hang a scan.
    fn read(&self, directory: &Path) -> std::io::Result<(Group, Vec<PathBuf>)> {
        let mut group = Group {
            directory: directory.to_path_buf(),
            ..Group::default()
        };
        let mut subdirectories = Vec::new();
        for entry in fs::read_dir(directory)?.flatten() {
            let path = entry.path();
            match entry.file_type() {
                Ok(file_type) if file_type.is_dir() => {
                    if self.recursive {
                        subdirectories.push(path);
                    }
                }
                Ok(file_type) if file_type.is_file() => self.classify_into(path, &mut group),
                // Follows symlinks, so a link to a video counts as one.
                _ if path.is_file() => self.classify_into(path, &mut group),
                _ => {}
            }
        }
        Ok((group, subdirectories))
    }
}

/// Whether `extension` is in `list`, which holds normalised lowercase forms.
///
/// The lists are a handful of entries, so a scan beats hashing — and the ASCII
/// path avoids lowercasing every filename's extension into a fresh string.
fn extension_matches(list: &[String], extension: &std::ffi::OsStr) -> bool {
    match extension.to_str() {
        Some(extension) if extension.is_ascii() => list
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(extension)),
        Some(extension) => {
            let lowered = extension.to_lowercase();
            list.contains(&lowered)
        }
        None => {
            let lowered = extension.to_string_lossy().to_lowercase();
            list.contains(&lowered)
        }
    }
}

/// Plan the renames for a real directory on disk. Reads, never writes.
pub fn plan_directory(root: &Path, options: &PlanOptions) -> Result<RenamePlan, PlanError> {
    let root = crate::paths::resolve(root);
    if !root.is_dir() {
        return Err(PlanError::NotADirectory(root));
    }

    let groups = scan_directory(&root, options)?;
    Ok(create_plan(root, groups, options, &|path: &Path| {
        path.exists()
    }))
}

/// Read `root`, and everything below it when asked, into one group per folder.
///
/// The root itself must be readable; anything below it may not be, and an
/// unreadable subdirectory is skipped rather than failing the whole run.
fn scan_directory(root: &Path, options: &PlanOptions) -> std::io::Result<Vec<Group>> {
    let scan = Scan::new(options);
    let (root_group, subdirectories) = scan.read(root)?;

    let mut groups = Vec::new();
    if !root_group.is_empty() {
        groups.push(root_group);
    }
    if !subdirectories.is_empty() {
        groups.extend(walk(subdirectories, &scan));
    }
    Ok(groups)
}

/// Directories still to be read, and how many workers are reading right now.
///
/// The count is what tells a worker the difference between "nothing to do yet"
/// and "nothing left to do at all".
struct Pending {
    stack: Vec<PathBuf>,
    busy: usize,
}

struct Queue {
    pending: Mutex<Pending>,
    ready: Condvar,
}

impl Queue {
    fn new(stack: Vec<PathBuf>) -> Self {
        Self {
            pending: Mutex::new(Pending { stack, busy: 0 }),
            ready: Condvar::new(),
        }
    }

    /// Claim the next directory, or `None` once the whole tree is read.
    fn take(&self) -> Option<PathBuf> {
        let mut pending = self.lock();
        loop {
            if let Some(directory) = pending.stack.pop() {
                pending.busy += 1;
                return Some(directory);
            }
            if pending.busy == 0 {
                // Nothing queued and nobody still reading: everyone can stop.
                self.ready.notify_all();
                return None;
            }
            pending = self
                .ready
                .wait(pending)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
    }

    /// Hand back whatever the claimed directory contained.
    fn finish(&self, subdirectories: Vec<PathBuf>) {
        let mut pending = self.lock();
        pending.stack.extend(subdirectories);
        pending.busy -= 1;
        drop(pending);
        self.ready.notify_all();
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Pending> {
        self.pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Walk the tree below the root, reading folders on several threads at once.
fn walk(seed: Vec<PathBuf>, scan: &Scan) -> Vec<Group> {
    let queue = Queue::new(seed);
    let drain = || {
        let mut groups = Vec::new();
        while let Some(directory) = queue.take() {
            let (group, subdirectories) = scan.read(&directory).unwrap_or_default();
            queue.finish(subdirectories);
            if !group.is_empty() {
                groups.push(group);
            }
        }
        groups
    };

    // How wide the tree turns out to be is not known until it has been walked,
    // so the worker count comes from the machine rather than from the seed.
    let workers = crate::parallel::worker_count(usize::MAX);
    if workers <= 1 {
        return drain();
    }
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers).map(|_| scope.spawn(drain)).collect();
        handles
            .into_iter()
            .flat_map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|payload| std::panic::resume_unwind(payload))
            })
            .collect()
    })
}

/// Plan renames for a made-up file listing used by tests.
///
/// Nothing here touches the filesystem: "does this target already exist" is
/// answered from the listing itself.
#[cfg(test)]
pub(crate) fn plan_virtual_files(file_names: &[&str], options: &PlanOptions) -> RenamePlan {
    let root = PathBuf::from("/virtual-subtitle-library");
    let paths: Vec<PathBuf> = file_names.iter().map(|name| root.join(name)).collect();
    let existing: HashSet<PathBuf> = paths.iter().cloned().collect();

    let scan = Scan::new(options);
    let mut groups: HashMap<PathBuf, Group> = HashMap::new();
    for path in paths {
        let directory = path.parent().unwrap_or(&root).to_path_buf();
        let group = groups.entry(directory.clone()).or_insert_with(|| Group {
            directory,
            ..Group::default()
        });
        scan.classify_into(path, group);
    }

    create_plan(
        root,
        groups
            .into_values()
            .filter(|group| !group.is_empty())
            .collect(),
        options,
        &move |path: &Path| existing.contains(path),
    )
}

fn create_plan(
    root: PathBuf,
    mut groups: Vec<Group>,
    options: &PlanOptions,
    path_exists: &(dyn Fn(&Path) -> bool + Sync),
) -> RenamePlan {
    groups.sort_by_cached_key(|group| sort_key(&group.directory));

    let video_count = groups.iter().map(|group| group.videos.len()).sum();
    let subtitle_count = groups.iter().map(|group| group.subtitles.len()).sum();

    // Folders are independent, so they are planned at once and stitched back
    // together in the sorted order above.
    // With enough folders the threads go here; with only a few, they go inside
    // a folder instead, so one enormous flat directory is not left to a single
    // core. Nesting both would just oversubscribe the machine.
    let spread_within = crate::parallel::worker_count(groups.len()) <= 1;
    let per_directory = crate::parallel::map(&groups, |group| {
        build_directory_plan(
            &ordered(&group.subtitles),
            &ordered(&group.videos),
            options,
            path_exists,
            spread_within,
        )
    });

    let mut operations = Vec::new();
    let mut skipped = Vec::new();
    for (directory_operations, directory_skipped) in per_directory {
        operations.extend(directory_operations);
        skipped.extend(directory_skipped);
    }

    RenamePlan {
        root,
        operations,
        skipped,
        video_count,
        subtitle_count,
    }
}

/// Decide every rename inside one folder.
fn build_directory_plan(
    subtitles: &[&Candidate],
    videos: &[&Candidate],
    options: &PlanOptions,
    path_exists: &(dyn Fn(&Path) -> bool + Sync),
    spread_within: bool,
) -> (Vec<RenameOp>, Vec<SkippedRename>) {
    let mut operations = Vec::new();
    let mut skipped = Vec::new();
    if subtitles.is_empty() {
        return (operations, skipped);
    }
    if videos.is_empty() {
        skipped.extend(subtitles.iter().map(|subtitle| SkippedRename {
            path: subtitle.path.clone(),
            reason: SkipReason::NoVideo,
        }));
        return (operations, skipped);
    }

    // An episode id shared by two videos identifies neither, so both drop out of
    // the index and any subtitle carrying that id is reported as ambiguous.
    let mut videos_by_episode: HashMap<&str, &Candidate> = HashMap::new();
    let mut ambiguous: HashSet<&str> = HashSet::new();
    for video in videos {
        let Some(key) = video.episode_key.as_deref() else {
            continue;
        };
        if videos_by_episode.insert(key, *video).is_some() {
            ambiguous.insert(key);
        }
    }
    for key in &ambiguous {
        videos_by_episode.remove(key);
    }

    // Which video a subtitle belongs to depends on nothing but the folder, so
    // every subtitle is decided at once. Where it then *goes* does depend on the
    // subtitles before it, and stays in order below.
    let targets = fuzzy_index(subtitles, videos, spread_within);
    let decide = |scratch: &mut Scratch, subtitle: &&Candidate| {
        match_subtitle(
            subtitle,
            videos,
            &videos_by_episode,
            &ambiguous,
            &targets,
            options,
            scratch,
        )
    };
    let matches = if spread_within {
        crate::parallel::map_with(subtitles, Scratch::default, decide)
    } else {
        let mut scratch = Scratch::default();
        subtitles
            .iter()
            .map(|subtitle| decide(&mut scratch, subtitle))
            .collect()
    };

    let mut planned: HashSet<PathBuf> = HashSet::new();
    for (subtitle, matched) in subtitles.iter().zip(matches) {
        let (video, reason) = match matched {
            Ok(matched) => matched,
            Err(reason) => {
                skipped.push(SkippedRename {
                    path: subtitle.path.clone(),
                    reason,
                });
                continue;
            }
        };

        let Some(destination) = choose_destination(
            subtitle,
            video,
            options.strict,
            options.overwrite_existing,
            path_exists,
            &planned,
        ) else {
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

    (operations, skipped)
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
    overwrite_existing: bool,
    path_exists: &(dyn Fn(&Path) -> bool + Sync),
    planned: &HashSet<PathBuf>,
) -> Option<PathBuf> {
    let extension = subtitle
        .path
        .extension()
        .map(|e| e.to_string_lossy().into_owned())?;
    let directory = video.path.parent()?;
    let video_stem = video.path.file_stem()?.to_string_lossy().into_owned();

    // Renaming a file onto its own name is not a collision; the caller reports
    // that case as "already matches".
    let taken = |candidate: &Path| {
        planned.contains(candidate) || (!overwrite_existing && path_exists(candidate))
    };
    let available = |candidate: PathBuf| {
        (candidate == subtitle.path || !taken(&candidate)).then_some(candidate)
    };

    let base = directory.join(format!("{video_stem}.{extension}"));
    if let Some(base) = available(base) {
        return Some(base);
    }
    if strict {
        return None;
    }

    if let Some(tag) = language_tag(&subtitle.path.file_stem()?.to_string_lossy()) {
        let tagged = directory.join(format!("{video_stem}.{tag}.{extension}"));
        if let Some(tagged) = available(tagged) {
            return Some(tagged);
        }
    }

    // Bounded so a pathological directory cannot spin here forever.
    for number in 2..1000 {
        let numbered = directory.join(format!("{video_stem}.{number}.{extension}"));
        if let Some(numbered) = available(numbered) {
            return Some(numbered);
        }
    }
    None
}

/// Index every video stem for fuzzy matching, once per folder.
///
/// Skipped entirely when every subtitle in the folder settles on an episode id,
/// which is the common case for a tidy library.
fn fuzzy_index(subtitles: &[&Candidate], videos: &[&Candidate], spread: bool) -> Vec<Target> {
    let wanted = subtitles
        .iter()
        .any(|subtitle| subtitle.episode_key.is_none());
    if !wanted {
        return Vec::new();
    }
    let build = |video: &&Candidate| Target::new(&video.stem_norm);
    if spread {
        crate::parallel::map(videos, build)
    } else {
        videos.iter().map(build).collect()
    }
}

/// Work out which video a subtitle belongs to, or why it is being left alone.
fn match_subtitle<'a>(
    subtitle: &Candidate,
    videos: &[&'a Candidate],
    videos_by_episode: &HashMap<&str, &'a Candidate>,
    ambiguous: &HashSet<&str>,
    targets: &[Target],
    options: &PlanOptions,
    scratch: &mut Scratch,
) -> Result<(&'a Candidate, MatchReason), SkipReason> {
    let Some(key) = subtitle.episode_key.as_deref() else {
        return match choose_unique_best(subtitle, videos, targets, options.min_score, scratch) {
            (Some(video), score) => Ok((video, MatchReason::Fuzzy(score))),
            (None, best_score) => Err(SkipReason::Unmatched { best_score }),
        };
    };
    if ambiguous.contains(key) {
        return Err(SkipReason::AmbiguousEpisode(key.to_string()));
    }
    match videos_by_episode.get(key) {
        Some(video) => Ok((*video, MatchReason::Episode(key.to_string()))),
        None => Err(SkipReason::NoMatchingEpisode(key.to_string())),
    }
}

/// Pick the one video that clearly fits `subtitle`, with its score.
///
/// Returns the best score even when nothing is chosen, so the preview can show
/// how close the near miss was.
fn choose_unique_best<'a>(
    subtitle: &Candidate,
    videos: &[&'a Candidate],
    targets: &[Target],
    min_score: f64,
    scratch: &mut Scratch,
) -> (Option<&'a Candidate>, f64) {
    if subtitle.stem_norm.is_empty() {
        return (None, 0.0);
    }
    let counts = CharCounts::new(&subtitle.stem_norm);

    let mut best: Option<(f64, &Candidate)> = None;
    let mut runner_up = 0.0;
    for (video, target) in videos.iter().copied().zip(targets) {
        if target.is_empty() {
            continue;
        }
        // A video that cannot reach the runner-up even in the best case can
        // change neither the winner nor the margin, so it is not scored.
        if target.ratio_bound(&counts) <= runner_up {
            continue;
        }
        let score = target.ratio(&subtitle.stem_norm, scratch);
        match best {
            Some((best_score, _)) if score > best_score => {
                runner_up = best_score;
                best = Some((score, video));
            }
            Some(_) if score > runner_up => runner_up = score,
            None => best = Some((score, video)),
            _ => {}
        }
    }

    let Some((best_score, best)) = best else {
        return (None, 0.0);
    };
    if best_score < min_score {
        return (None, best_score);
    }
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
            "some.other.release.S01E02.mkv",
            "some.other.release.S01E01.chs.ass",
        ]);
        assert_eq!(destinations(&plan), ["Nebula.S01E01.1080p.ass"]);
        assert_eq!(
            plan.operations[0].reason,
            MatchReason::Episode("S01E01".into())
        );
        assert_eq!(plan.video_count, 2);
        assert_eq!(plan.subtitle_count, 1);
    }

    #[test]
    fn does_not_fuzzy_match_when_episode_id_has_no_matching_video() {
        let plan = plan(&["Nebula.S01E01.mkv", "Nebula.S01E02.srt"]);

        assert!(plan.operations.is_empty());
        assert_eq!(
            plan.skipped[0].reason,
            SkipReason::NoMatchingEpisode("S01E02".into())
        );
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
    fn overwrite_existing_uses_the_plain_target() {
        let plan = plan_with(
            &[
                "Nebula.S01E01.mkv",
                "Nebula.S01E01.srt",
                "Other.Release.S01E01.chs.srt",
            ],
            PlanOptions {
                overwrite_existing: true,
                ..PlanOptions::default()
            },
        );

        assert_eq!(destinations(&plan), ["Nebula.S01E01.srt"]);
        assert_eq!(
            plan.operations[0].source.file_name().unwrap(),
            "Other.Release.S01E01.chs.srt"
        );
    }

    #[test]
    fn overwrite_existing_keeps_planned_targets_unique() {
        let plan = plan_with(
            &[
                "Nebula.S01E01.mkv",
                "A.Release.S01E01.chs.srt",
                "B.Release.S01E01.eng.srt",
            ],
            PlanOptions {
                overwrite_existing: true,
                ..PlanOptions::default()
            },
        );

        let destinations = destinations(&plan);
        assert_eq!(destinations, ["Nebula.S01E01.srt", "Nebula.S01E01.eng.srt"]);
        assert_eq!(
            destinations.iter().collect::<HashSet<_>>().len(),
            destinations.len()
        );
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
        // The nested subtitle already matches, so only the top-level one moves.
        assert_eq!(deep.operations.len(), 1);
    }

    /// Wide enough that the scan runs on several threads, which must not change
    /// what is found or the order it comes back in.
    #[test]
    fn a_wide_recursive_scan_finds_every_folder_in_a_stable_order() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        for show in 0..40 {
            let directory = root.join(format!("show{show:02}/season01"));
            std::fs::create_dir_all(&directory).unwrap();
            for episode in 1..=4 {
                std::fs::write(
                    directory.join(format!("Nebula.S01E{episode:02}.1080p.mkv")),
                    b"",
                )
                .unwrap();
                std::fs::write(
                    directory.join(format!("Other.Release.S01E{episode:02}.chs.srt")),
                    b"",
                )
                .unwrap();
            }
        }

        let options = PlanOptions {
            recursive: true,
            ..PlanOptions::default()
        };
        let plan = plan_directory(root, &options).unwrap();
        assert_eq!(plan.video_count, 160);
        assert_eq!(plan.subtitle_count, 160);
        assert_eq!(plan.operations.len(), 160);

        let sources: Vec<PathBuf> = plan
            .operations
            .iter()
            .map(|operation| operation.source.clone())
            .collect();
        let mut sorted = sources.clone();
        sorted.sort_by_cached_key(|path| sort_key(path));
        assert_eq!(sources, sorted);

        // And again: two runs of the same tree must agree exactly.
        let again = plan_directory(root, &options).unwrap();
        assert_eq!(
            again
                .operations
                .iter()
                .map(|operation| operation.destination.clone())
                .collect::<Vec<_>>(),
            plan.operations
                .iter()
                .map(|operation| operation.destination.clone())
                .collect::<Vec<_>>()
        );
    }
}
