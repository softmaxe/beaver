from __future__ import annotations

import re

from rename_subtitles import web


def _asset_text(name: str) -> str:
    return (web.ASSET_DIR / name).read_text(encoding="utf-8")


def _translation_keys(script: str, language: str) -> set[str]:
    match = re.search(
        rf"^  {language}: \{{(?P<body>.*?)^  \}},",
        script,
        re.DOTALL | re.MULTILINE,
    )
    assert match, f"could not find {language} translations"
    return set(re.findall(r"^    (\w+):", match.group("body"), re.MULTILINE))


def test_workspace_uses_named_match_levels_instead_of_a_numeric_slider() -> None:
    page = _asset_text("index.html")
    script = _asset_text("app.js")

    assert 'name="match-level"' in page
    assert 'value="relaxed"' in page
    assert 'value="balanced" checked' in page
    assert 'value="cautious"' in page
    assert 'type="range"' not in page
    assert 'id="min-score"' not in page
    assert "模糊匹配阈值" not in page
    assert "const MATCH_LEVEL_SCORES" in script
    assert "min_score: selectedMatchLevelScore()," in script


def test_workspace_keeps_its_required_accessible_controls() -> None:
    page = _asset_text("index.html")

    assert '<form id="preview-form"' in page
    assert '<input id="directory"' in page
    assert '<button id="preview-button"' in page
    assert '<button id="choose-directory-button"' in page
    assert '<input id="recursive" type="checkbox">\n              <span class="switch"' in page
    assert '<input id="strict" type="checkbox">\n              <span class="switch"' in page
    assert '<button id="theme-toggle" class="theme-toggle" type="button" aria-pressed="true"' in page
    assert '<span id="theme-label">暗黑</span>' in page
    assert '<tbody id="operations-body"></tbody>' in page
    assert '<dialog id="confirm-dialog"' in page
    assert 'id="cancel-apply-button" class="button secondary" type="button"' in page
    assert 'id="confirm-apply-button" class="button primary" type="button"' in page
    assert 'role="status" aria-live="polite"' in page
    assert '<th scope="col"' in page


def test_translations_cover_every_markup_key_in_both_languages() -> None:
    page = _asset_text("index.html")
    script = _asset_text("app.js")
    markup_keys = set(re.findall(r'data-i18n="([^"]+)"', page))
    markup_keys.update(re.findall(r'data-i18n-(?:placeholder|title)="([^"]+)"', page))

    assert markup_keys <= _translation_keys(script, "zh")
    assert markup_keys <= _translation_keys(script, "en")


def test_ui_uses_safe_rendering_and_invalidates_stale_live_previews() -> None:
    script = _asset_text("app.js")

    assert "innerHTML" not in script
    assert "function invalidatePreview()" in script
    assert 'elements.directory.addEventListener("input", invalidatePreview);' in script
    assert 'input.addEventListener("change", invalidatePreview);' in script
    assert 'invalidatePreview();\n    setAlert(errorKey(error.message), "error");' in script
    assert 'state.payload.mode !== "live" || state.planConsumed || selectedIds.length === 0' in script


def test_theme_choice_is_explicit_persisted_and_accessible() -> None:
    page = _asset_text("index.html")
    script = _asset_text("app.js")
    styles = _asset_text("styles.css")

    assert '<html lang="zh-CN" data-theme="dark">' in page
    assert '<meta name="color-scheme" content="dark" id="color-scheme">' in page
    assert '<meta name="theme-color" content="#181715" id="theme-color">' in page
    assert 'localStorage.getItem("rename-subtitles-theme")' in page
    assert 'document.documentElement.dataset.theme = theme;' in page
    assert "function readTheme()" in script
    assert 'localStorage.getItem("rename-subtitles-theme")' in script
    assert 'localStorage.setItem("rename-subtitles-theme", state.theme)' in script
    assert "function applyTheme(theme, persist = true)" in script
    assert "document.documentElement.dataset.theme = state.theme;" in script
    assert 'elements.themeColor.setAttribute("content", THEME_COLORS[state.theme]);' in script
    assert 'elements.themeToggle.addEventListener("click"' in script
    assert "function setTheme(theme)" in script
    assert "prefersReducedMotion()" in script
    assert "switchToLightTheme" in script
    assert "switchToDarkTheme" in script
    assert "@media (prefers-color-scheme: dark)" not in styles
    assert "color-scheme: dark;" in styles
    assert ':root[data-theme="light"]' in styles
    assert "--canvas: #181715;" in styles
    assert "--canvas: #faf9f5;" in styles
    assert ".theme-toggle" in styles
    assert "--primary: #cc785c;" in styles
    assert "--primary-active: #a9583e;" in styles
    assert "--primary-disabled: #e6dfd8;" in styles
    assert "--surface-dark: #181715;" in styles
    assert "--teal: #5db8a6;" in styles
    assert "--success: #5db872;" in styles
    assert "--warning: #d4a017;" in styles
    assert "--error: #c64545;" in styles
    assert ".page-wash" not in styles
    assert "radial-gradient" not in styles
    assert "linear-gradient" not in styles
    assert "backdrop-filter" not in styles
    assert "@media (prefers-reduced-motion: no-preference)" in styles
    assert "@media (forced-colors: active)" in styles


def test_static_asset_map_does_not_reference_unserved_local_files() -> None:
    page = _asset_text("index.html")
    referenced_paths = set(re.findall(r'(?:href|src)="(/[^"]+)"', page))

    assert referenced_paths <= set(web.STATIC_FILES)
    assert {"/styles.css", "/app.js"} <= referenced_paths
