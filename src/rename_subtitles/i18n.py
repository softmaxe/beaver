"""Bilingual string catalog for the terminal interface.

Keys are dot-namespaced by where they appear (``setup.*``, ``action.*``,
``skip.*``, ...). Both languages must define exactly the same keys with the same
``str.format`` placeholders; ``tests/test_i18n.py`` enforces that.
"""

from __future__ import annotations

__all__ = ["CATALOG", "DEFAULT_LANGUAGE", "LANGUAGES", "Translator"]

DEFAULT_LANGUAGE = "zh"
LANGUAGES: tuple[str, ...] = ("zh", "en")

CATALOG: dict[str, dict[str, str]] = {
    "zh": {
        "app.title": "字幕重命名",
        "app.subtitle": "把字幕文件名对齐到同目录的视频",
        # Left column: scan options.
        "setup.heading": "设置",
        "setup.directory_label": "目录",
        "setup.directory_placeholder": "输入或粘贴目录路径",
        "setup.browse": "浏览…",
        "setup.recursive": "递归子目录",
        "setup.strict": "严格模式",
        "setup.strict_hint": "只接受能直接改成「视频名+后缀」的字幕",
        "setup.match_level": "匹配级别",
        "setup.level_relaxed": "宽松",
        "setup.level_balanced": "均衡",
        "setup.level_cautious": "谨慎",
        "setup.level_hint_relaxed": "宁可多匹配，适合命名混乱的目录",
        "setup.level_hint_balanced": "推荐；兼顾覆盖率与准确度",
        "setup.level_hint_cautious": "只接受几乎确定的匹配",
        # Buttons and key bindings.
        "action.preview": "生成预览",
        "action.apply": "应用重命名",
        "action.demo": "演示模式",
        "action.browse": "浏览目录",
        "action.toggle": "勾选/取消",
        "action.select_all": "全选",
        "action.clear_all": "全不选",
        "action.language": "语言",
        "action.theme": "主题",
        "action.help": "帮助",
        "action.quit": "退出",
        # Summary bar and result tabs.
        "summary.headline": "视频 {videos} · 字幕 {subtitles} · 匹配 {matched} · 跳过 {skipped}",
        "summary.directories": "目录 {directories}",
        "summary.empty": "尚未生成预览",
        "summary.selected": "已选 {selected} / {total}",
        "tab.matched": "待重命名 ({count})",
        "tab.skipped": "已跳过 ({count})",
        "table.source": "源文件",
        "table.reason": "原因",
        "list.empty_matched": "没有可重命名的字幕",
        "list.empty_skipped": "没有被跳过的字幕",
        # How a match or a skip is explained.
        "match.episode": "剧集 {detail}",
        "match.fuzzy": "模糊 {detail}",
        "skip.unmatched": "找不到匹配的视频",
        "skip.already_matching": "文件名已经对齐",
        "skip.strict_collision": "严格模式下目标名冲突",
        "skip.no_video": "该目录没有视频文件",
        "skip.collision": "目标名冲突",
        # Status line and errors.
        "status.ready": "填写目录后按 Enter 生成预览",
        "status.scanning": "正在扫描…",
        "status.previewed": "预览完成；勾选后按 A 应用",
        "status.stale": "选项已改动，请重新生成预览",
        "status.demo": "演示模式：样本数据，不会改动磁盘",
        "status.applying": "正在应用…",
        "error.empty_directory": "请先填写目录路径",
        "error.invalid_directory": "不是有效的目录：{path}",
        "error.scan_failed": "扫描失败：{detail}",
        "error.no_video": "该目录下没有视频文件",
        "error.no_subtitle": "该目录下没有字幕文件",
        "error.nothing_selected": "请至少勾选一项",
        "error.plan_changed": "文件已变动，预览已失效，请重新生成：{detail}",
        "error.demo_not_applicable": "演示模式不会改动磁盘",
        # Confirmation modal.
        "confirm.title": "确认应用",
        "confirm.body": "即将重命名 {count} 个字幕文件，此操作不会覆盖已存在的文件。",
        "confirm.ok": "确认应用",
        "confirm.cancel": "取消",
        # Outcome of an apply run.
        "result.completed": "已重命名 {applied} 个文件",
        "result.partial": "已重命名 {applied} 个，{failed} 个失败",
        "result.failed": "{failed} 个文件重命名失败",
        # Directory picker modal.
        "picker.title": "选择目录",
        "picker.select": "选择当前目录",
        "picker.cancel": "取消",
        # Help modal.
        "help.title": "快捷键",
        "help.close": "关闭",
        "help.workflow": "工作流：填目录 → Enter/P 预览 → 勾选 → A 应用",
        "help.note": "输入框获得焦点时，单字母快捷键会被当作输入；按 Tab 离开输入框即可使用。",
    },
    "en": {
        "app.title": "Subtitle Renamer",
        "app.subtitle": "Align subtitle filenames with the videos beside them",
        # Left column: scan options.
        "setup.heading": "Setup",
        "setup.directory_label": "Directory",
        "setup.directory_placeholder": "Type or paste a directory path",
        "setup.browse": "Browse…",
        "setup.recursive": "Include subfolders",
        "setup.strict": "Strict mode",
        "setup.strict_hint": "Only accept subtitles that fit VideoName+ext exactly",
        "setup.match_level": "Match level",
        "setup.level_relaxed": "Relaxed",
        "setup.level_balanced": "Balanced",
        "setup.level_cautious": "Cautious",
        "setup.level_hint_relaxed": "Matches more, for messy naming",
        "setup.level_hint_balanced": "Recommended; coverage and accuracy",
        "setup.level_hint_cautious": "Only near-certain matches",
        # Buttons and key bindings.
        "action.preview": "Preview",
        "action.apply": "Apply renames",
        "action.demo": "Demo mode",
        "action.browse": "Browse",
        "action.toggle": "Toggle",
        "action.select_all": "Select all",
        "action.clear_all": "Clear all",
        "action.language": "Language",
        "action.theme": "Theme",
        "action.help": "Help",
        "action.quit": "Quit",
        # Summary bar and result tabs.
        "summary.headline": (
            "{videos} videos · {subtitles} subtitles · {matched} matched · {skipped} skipped"
        ),
        "summary.directories": "{directories} directories",
        "summary.empty": "No preview yet",
        "summary.selected": "{selected} of {total} selected",
        "tab.matched": "To rename ({count})",
        "tab.skipped": "Skipped ({count})",
        "table.source": "Source",
        "table.reason": "Reason",
        "list.empty_matched": "Nothing to rename",
        "list.empty_skipped": "Nothing was skipped",
        # How a match or a skip is explained.
        "match.episode": "episode {detail}",
        "match.fuzzy": "fuzzy {detail}",
        "skip.unmatched": "No matching video",
        "skip.already_matching": "Filename already matches",
        "skip.strict_collision": "Target name taken (strict mode)",
        "skip.no_video": "No video files in this folder",
        "skip.collision": "Target name taken",
        # Status line and errors.
        "status.ready": "Type a directory path, then press Enter to preview",
        "status.scanning": "Scanning…",
        "status.previewed": "Preview ready — tick items, then press A to apply",
        "status.stale": "Options changed — preview again",
        "status.demo": "Demo mode: sample data, nothing is written",
        "status.applying": "Applying…",
        "error.empty_directory": "Enter a directory path first",
        "error.invalid_directory": "Not a directory: {path}",
        "error.scan_failed": "Scan failed: {detail}",
        "error.no_video": "No video files in this directory",
        "error.no_subtitle": "No subtitle files in this directory",
        "error.nothing_selected": "Select at least one subtitle",
        "error.plan_changed": "Files changed on disk — preview again: {detail}",
        "error.demo_not_applicable": "Demo mode never writes to disk",
        # Confirmation modal.
        "confirm.title": "Confirm apply",
        "confirm.body": "About to rename {count} subtitle files. Existing files are never overwritten.",
        "confirm.ok": "Apply",
        "confirm.cancel": "Cancel",
        # Outcome of an apply run.
        "result.completed": "Renamed {applied} files",
        "result.partial": "Renamed {applied}, {failed} failed",
        "result.failed": "{failed} renames failed",
        # Directory picker modal.
        "picker.title": "Choose a directory",
        "picker.select": "Use this directory",
        "picker.cancel": "Cancel",
        # Help modal.
        "help.title": "Keyboard shortcuts",
        "help.close": "Close",
        "help.workflow": "Workflow: directory → Enter or P to preview → tick → A to apply",
        "help.note": "Single-letter shortcuts type into the path field while it has focus; press Tab to leave it.",
    },
}


class Translator:
    """Looks strings up in :data:`CATALOG` for the currently selected language."""

    def __init__(self, language: str = DEFAULT_LANGUAGE) -> None:
        self.language = language if language in LANGUAGES else DEFAULT_LANGUAGE

    def __call__(self, key: str, **kwargs: object) -> str:
        """Return the translated string for ``key``, formatted with ``kwargs``.

        An unknown key returns itself, so a missing translation shows up in the UI
        instead of crashing it.
        """
        template = CATALOG[self.language].get(key)
        if template is None:
            return key
        if not kwargs:
            return template
        return template.format(**kwargs)

    def toggle(self) -> str:
        """Switch to the next language and return it."""
        index = LANGUAGES.index(self.language)
        self.language = LANGUAGES[(index + 1) % len(LANGUAGES)]
        return self.language
