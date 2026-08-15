const translations = {
  zh: {
    brandTagline: "本地智能匹配工作台",
    localOnly: "仅本机运行",
    localOnlyHint: "文件不会上传或离开此设备。",
    switchLanguage: "切换到英文界面",
    darkTheme: "暗黑模式",
    lightTheme: "日间模式",
    switchToLightTheme: "切换到日间模式",
    switchToDarkTheme: "切换到暗黑模式",
    eyebrow: "SMART LOCAL WORKFLOW",
    heroTitle: "让字幕和视频，自然对齐。",
    heroCopy: "先预览每一项匹配，再安全地执行重命名。所有操作都在你的电脑上完成。",
    tryDemo: "体验虚拟样例",
    stepOneTitle: "选择文件夹",
    stepOneCopy: "输入存放视频和字幕的本地路径。",
    stepTwoTitle: "查看预览",
    stepTwoCopy: "核对匹配方式、目标名称和跳过项。",
    stepThreeTitle: "确认执行",
    stepThreeCopy: "只勾选需要的项目，再进行确认。",
    settingsEyebrow: "SETUP",
    settingsTitle: "准备一次安全预览",
    safeBadge: "先预览，后执行",
    directoryLabel: "本地文件夹",
    directoryPlaceholder: "粘贴本机文件夹完整路径",
    directoryHint: "选择后先生成预览；确认结果后即可执行重命名。",
    chooseDirectory: "选择本地文件夹",
    recursiveTitle: "扫描子文件夹",
    recursiveHint: "按每个文件夹独立匹配",
    strictTitle: "严格命名",
    strictHint: "冲突时跳过，不添加后缀",
    matchLevelLabel: "匹配严格程度",
    matchLevelRelaxedTitle: "宽松",
    matchLevelRelaxedHint: "尽量多找可能的匹配",
    matchLevelBalancedTitle: "推荐",
    matchLevelBalancedHint: "兼顾匹配数量和可靠性",
    matchLevelCautiousTitle: "谨慎",
    matchLevelCautiousHint: "只保留更可靠的匹配",
    matchLevelHint: "只影响没有剧集编号的文件；带编号的剧集会自动匹配。",
    previewButton: "生成重命名预览",
    resultsEyebrow: "REVIEW",
    resultsTitle: "匹配结果",
    emptyTitle: "等待一次预览",
    emptyCopy: "选择一个文件夹，或先加载虚拟样例来查看完整流程。",
    summaryVideos: "视频",
    summarySubtitles: "字幕",
    summaryMatched: "可重命名",
    summarySkipped: "已跳过",
    operationsTitle: "建议执行的操作",
    operationsCopy: "取消勾选不想执行的项目。",
    selectAll: "全选",
    selectColumn: "选择",
    sourceColumn: "原字幕文件",
    targetColumn: "新文件名",
    matchColumn: "匹配方式",
    skippedTitle: "未执行的字幕",
    skippedCopy: "这些项目不会被修改；可调整设置后重新预览。",
    safetyTitle: "安全执行",
    safetyCopy: "仅会执行本次预览中的已勾选项目；执行前会再次检查文件是否发生变化。",
    applyButton: "确认并执行选中的重命名",
    confirmEyebrow: "FINAL CHECK",
    confirmTitle: "准备执行重命名？",
    confirmCopy: "请确认已检查目标文件名。此操作会修改本地文件名。",
    cancelButton: "返回检查",
    confirmButton: "执行重命名",
    demoMode: "虚拟样例",
    liveMode: "可执行预览",
    previewMode: "本地预览",
    demoBanner: "这是虚拟数据演示，不会读取或修改本地文件。",
    previewReady: "预览已生成。请核对文件名后再执行。",
    previewNeedsReview: "预览已生成，部分字幕被安全跳过。",
    previewInvalidated: "设置已改变。请重新生成预览后再执行重命名。",
    noOperations: "没有可执行的重命名。可切换匹配严格程度或检查文件夹内容。",
    noVideoFiles: "未找到可识别的视频文件。",
    noSubtitleFiles: "未找到可识别的字幕文件。",
    folderPickerUnavailable: "无法打开文件夹选择器，请直接粘贴本机路径。",
    folderPickerCancelled: "未选择文件夹。",
    openingFolderPicker: "正在打开系统文件夹选择器…",
    folderSelected: "已选择文件夹。下一步：生成重命名预览。",
    selectAtLeastOne: "请至少勾选一项重命名操作。",
    confirmCount: "将执行 {count} 项本地重命名。请确认目标文件名正确。",
    appliedSuccess: "已完成 {count} 项重命名。",
    appliedPartial: "已完成 {count} 项；其余项目未能执行，请重新预览后检查。",
    operationApplied: "已完成",
    operationNotApplied: "未完成",
    episodeMatch: "剧集编号 {detail}",
    fuzzyMatch: "模糊匹配 {score}%",
    skipUnmatched: "未找到可靠匹配",
    skipAlreadyMatching: "已符合命名",
    skipStrictCollision: "严格模式下发生冲突",
    skipNoVideo: "同目录没有视频",
    skipCollision: "目标名称冲突",
    errorInvalidDirectory: "无法读取该文件夹。请检查路径和访问权限。",
    errorInvalidOption: "设置无效，请检查后再试。",
    errorInvalidOrExpiredPlan: "预览已失效，请重新生成预览。",
    errorPlanAlreadyUsed: "该预览已经执行过，请重新生成预览。",
    errorPlanChanged: "文件状态发生变化。为确保安全，请重新生成预览。",
    errorNoOperationsSelected: "请至少勾选一项重命名操作。",
    errorInvalidOperationSelection: "选择的项目无效，请重新生成预览。",
    errorRequestTooLarge: "请求内容过大，请缩短文件夹路径后重试。",
    errorGeneric: "操作未完成，请稍后重试。",
  },
  en: {
    brandTagline: "Local matching studio",
    localOnly: "Runs locally",
    localOnlyHint: "Files never leave this device.",
    switchLanguage: "Switch to Simplified Chinese",
    darkTheme: "Dark mode",
    lightTheme: "Light mode",
    switchToLightTheme: "Switch to light mode",
    switchToDarkTheme: "Switch to dark mode",
    eyebrow: "SMART LOCAL WORKFLOW",
    heroTitle: "Make subtitles match naturally.",
    heroCopy: "Review every match first, then rename safely. Every operation stays on your computer.",
    tryDemo: "Try virtual demo",
    stepOneTitle: "Choose a folder",
    stepOneCopy: "Enter the local path containing videos and subtitles.",
    stepTwoTitle: "Review the preview",
    stepTwoCopy: "Check match methods, target names, and skipped files.",
    stepThreeTitle: "Confirm changes",
    stepThreeCopy: "Select only the items you want, then confirm.",
    settingsEyebrow: "SETUP",
    settingsTitle: "Prepare a safe preview",
    safeBadge: "Preview before changes",
    directoryLabel: "Local folder",
    directoryPlaceholder: "Paste a full local folder path",
    directoryHint: "Build a preview first, then confirm the selected renames.",
    chooseDirectory: "Choose local folder",
    recursiveTitle: "Scan subfolders",
    recursiveHint: "Match files separately in each folder",
    strictTitle: "Strict naming",
    strictHint: "Skip collisions instead of adding suffixes",
    matchLevelLabel: "Match strictness",
    matchLevelRelaxedTitle: "Flexible",
    matchLevelRelaxedHint: "Find more possible matches",
    matchLevelBalancedTitle: "Recommended",
    matchLevelBalancedHint: "Balance coverage and reliability",
    matchLevelCautiousTitle: "Careful",
    matchLevelCautiousHint: "Keep only stronger matches",
    matchLevelHint: "Only affects files without episode numbers; numbered episodes match automatically.",
    previewButton: "Build rename preview",
    resultsEyebrow: "REVIEW",
    resultsTitle: "Match results",
    emptyTitle: "Waiting for a preview",
    emptyCopy: "Choose a folder, or load the virtual demo to see the complete workflow.",
    summaryVideos: "Videos",
    summarySubtitles: "Subtitles",
    summaryMatched: "Ready to rename",
    summarySkipped: "Skipped",
    operationsTitle: "Recommended changes",
    operationsCopy: "Uncheck any item you do not want to run.",
    selectAll: "Select all",
    selectColumn: "Select",
    sourceColumn: "Current subtitle",
    targetColumn: "New filename",
    matchColumn: "Match method",
    skippedTitle: "Subtitles not changed",
    skippedCopy: "These files stay untouched. Adjust settings and preview again if needed.",
    safetyTitle: "Safe execution",
    safetyCopy: "Only selected items from this preview can run. File state is checked again before renaming.",
    applyButton: "Confirm and rename selected files",
    confirmEyebrow: "FINAL CHECK",
    confirmTitle: "Ready to rename files?",
    confirmCopy: "Confirm that you reviewed the target filenames. This changes local filenames.",
    cancelButton: "Review again",
    confirmButton: "Rename files",
    demoMode: "Virtual demo",
    liveMode: "Ready to apply",
    previewMode: "Local preview",
    demoBanner: "This is virtual demo data. No local files are read or changed.",
    previewReady: "Preview ready. Review filenames before applying changes.",
    previewNeedsReview: "Preview ready. Some subtitles were safely skipped for review.",
    previewInvalidated: "Settings changed. Build a new preview before applying renames.",
    noOperations: "No renames are ready. Change match strictness or check this folder's contents.",
    noVideoFiles: "No supported video files were found.",
    noSubtitleFiles: "No supported subtitle files were found.",
    folderPickerUnavailable: "The folder picker is unavailable. Paste a local path instead.",
    folderPickerCancelled: "No folder was selected.",
    openingFolderPicker: "Opening the system folder picker…",
    folderSelected: "Folder selected. Next: build the rename preview.",
    selectAtLeastOne: "Select at least one rename operation.",
    confirmCount: "This will rename {count} local file(s). Confirm that target filenames are correct.",
    appliedSuccess: "Completed {count} rename operation(s).",
    appliedPartial: "Completed {count} operation(s); other files could not be renamed. Preview again to review them.",
    operationApplied: "Completed",
    operationNotApplied: "Not completed",
    episodeMatch: "Episode {detail}",
    fuzzyMatch: "Fuzzy match {score}%",
    skipUnmatched: "No reliable match",
    skipAlreadyMatching: "Already matches",
    skipStrictCollision: "Collision in strict mode",
    skipNoVideo: "No video in this folder",
    skipCollision: "Target name collision",
    errorInvalidDirectory: "This folder cannot be read. Check the path and its permissions.",
    errorInvalidOption: "One of the settings is invalid. Check it and try again.",
    errorInvalidOrExpiredPlan: "This preview expired. Build a new preview to continue.",
    errorPlanAlreadyUsed: "This preview was already used. Build a new preview to continue.",
    errorPlanChanged: "File state changed. Build a new preview to keep the operation safe.",
    errorNoOperationsSelected: "Select at least one rename operation.",
    errorInvalidOperationSelection: "The selected items are invalid. Build a new preview to continue.",
    errorRequestTooLarge: "The request is too large. Use a shorter folder path and try again.",
    errorGeneric: "The operation could not be completed. Please try again.",
  },
};

const MATCH_LEVEL_SCORES = Object.freeze({
  relaxed: 0.6,
  balanced: 0.72,
  cautious: 0.84,
});

const THEME_COLORS = Object.freeze({
  dark: "#181715",
  light: "#faf9f5",
});

const elements = {
  themeToggle: document.querySelector("#theme-toggle"),
  themeLabel: document.querySelector("#theme-label"),
  themeColor: document.querySelector("#theme-color"),
  colorScheme: document.querySelector("#color-scheme"),
  languageToggle: document.querySelector("#language-toggle"),
  languageLabel: document.querySelector("#language-label"),
  previewForm: document.querySelector("#preview-form"),
  previewButton: document.querySelector("#preview-button"),
  directory: document.querySelector("#directory"),
  chooseDirectoryButton: document.querySelector("#choose-directory-button"),
  demoButton: document.querySelector("#demo-button"),
  recursive: document.querySelector("#recursive"),
  strict: document.querySelector("#strict"),
  matchLevels: document.querySelectorAll('input[name="match-level"]'),
  flowSteps: document.querySelectorAll(".flow-step"),
  resultMode: document.querySelector("#result-mode"),
  alertBanner: document.querySelector("#alert-banner"),
  emptyResults: document.querySelector("#empty-results"),
  resultsBoard: document.querySelector("#results-board"),
  summaryVideos: document.querySelector("#summary-videos"),
  summarySubtitles: document.querySelector("#summary-subtitles"),
  summaryMatched: document.querySelector("#summary-matched"),
  summarySkipped: document.querySelector("#summary-skipped"),
  operationsBody: document.querySelector("#operations-body"),
  selectAll: document.querySelector("#select-all"),
  skippedSection: document.querySelector("#skipped-section"),
  skippedList: document.querySelector("#skipped-list"),
  applyButton: document.querySelector("#apply-button"),
  confirmDialog: document.querySelector("#confirm-dialog"),
  confirmCopy: document.querySelector("#confirm-copy"),
  cancelApplyButton: document.querySelector("#cancel-apply-button"),
  confirmApplyButton: document.querySelector("#confirm-apply-button"),
};

const state = {
  theme: readTheme(),
  language: readLanguage(),
  payload: null,
  alert: null,
  planConsumed: false,
  applying: false,
  appliedOperationIds: new Set(),
  failedOperationIds: new Set(),
  selectedOperationIds: new Set(),
  selectionInitialized: false,
};

function prefersReducedMotion() {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

function readTheme() {
  try {
    const theme = localStorage.getItem("rename-subtitles-theme");
    return theme === "light" || theme === "dark" ? theme : "dark";
  } catch {
    return "dark";
  }
}

function readLanguage() {
  try {
    return localStorage.getItem("rename-subtitles-language") === "en" ? "en" : "zh";
  } catch {
    return "zh";
  }
}

function translate(key, values = {}) {
  const message = translations[state.language][key] ?? translations.en[key] ?? key;
  return message.replace(/\{(\w+)\}/g, (_, name) => String(values[name] ?? ""));
}

function updateThemeControl() {
  const isDark = state.theme === "dark";
  const targetKey = isDark ? "switchToLightTheme" : "switchToDarkTheme";
  const targetLabel = translate(targetKey);
  elements.themeLabel.textContent = translate(isDark ? "darkTheme" : "lightTheme");
  elements.themeToggle.setAttribute("aria-pressed", String(isDark));
  elements.themeToggle.setAttribute("aria-label", targetLabel);
  elements.themeToggle.setAttribute("title", targetLabel);
}

function applyTheme(theme, persist = true) {
  state.theme = theme === "light" ? "light" : "dark";
  document.documentElement.dataset.theme = state.theme;
  document.documentElement.style.colorScheme = state.theme;
  elements.colorScheme.setAttribute("content", state.theme);
  elements.themeColor.setAttribute("content", THEME_COLORS[state.theme]);
  updateThemeControl();
  if (!persist) {
    return;
  }
  try {
    localStorage.setItem("rename-subtitles-theme", state.theme);
  } catch {
    // The theme still changes when browser storage is unavailable.
  }
}

function setTheme(theme) {
  if (typeof document.startViewTransition !== "function" || prefersReducedMotion()) {
    applyTheme(theme);
    return;
  }
  document.startViewTransition(() => applyTheme(theme));
}

function applyLanguage(language) {
  state.language = language === "en" ? "en" : "zh";
  document.documentElement.lang = state.language === "zh" ? "zh-CN" : "en";
  document.title = state.language === "zh" ? "字幕重命名工作台" : "Subtitle Renamer";
  document.querySelectorAll("[data-i18n]").forEach((node) => {
    node.textContent = translate(node.dataset.i18n);
  });
  document.querySelectorAll("[data-i18n-placeholder]").forEach((node) => {
    node.setAttribute("placeholder", translate(node.dataset.i18nPlaceholder));
  });
  document.querySelectorAll("[data-i18n-title]").forEach((node) => {
    const label = translate(node.dataset.i18nTitle);
    node.setAttribute("title", label);
    node.setAttribute("aria-label", label);
  });
  elements.languageLabel.textContent = state.language === "zh" ? "EN" : "中";
  updateThemeControl();
  renderAlert();
  if (state.payload) {
    renderResults(state.payload);
  }
  try {
    localStorage.setItem("rename-subtitles-language", state.language);
  } catch {
    return;
  }
}

/* Cross-fades the whole document so the label swap does not read as a jump. */
function setLanguage(language) {
  if (typeof document.startViewTransition !== "function" || prefersReducedMotion()) {
    applyLanguage(language);
    return;
  }
  document.startViewTransition(() => applyLanguage(language));
}

function setAlert(key, type = "info", values = {}) {
  state.alert = { key, type, values };
  renderAlert();
}

function clearAlert() {
  state.alert = null;
  renderAlert();
}

function renderAlert() {
  if (!state.alert) {
    elements.alertBanner.hidden = true;
    elements.alertBanner.className = "alert-banner";
    elements.alertBanner.textContent = "";
    return;
  }
  elements.alertBanner.hidden = false;
  elements.alertBanner.className = `alert-banner ${state.alert.type}`;
  elements.alertBanner.textContent = translate(state.alert.key, state.alert.values);
}

async function requestJson(path, payload) {
  let response;
  try {
    response = await fetch(path, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });
  } catch {
    throw new Error("network_error");
  }

  let responsePayload = {};
  try {
    responsePayload = await response.json();
  } catch {
    throw new Error("network_error");
  }
  if (!response.ok) {
    throw new Error(responsePayload.error?.code ?? "unknown_error");
  }
  return responsePayload;
}

async function getJson(path) {
  let response;
  try {
    response = await fetch(path, { headers: { Accept: "application/json" } });
  } catch {
    throw new Error("network_error");
  }

  let responsePayload = {};
  try {
    responsePayload = await response.json();
  } catch {
    throw new Error("network_error");
  }
  if (!response.ok) {
    throw new Error(responsePayload.error?.code ?? "unknown_error");
  }
  return responsePayload;
}

function errorKey(code) {
  const keys = {
    invalid_directory: "errorInvalidDirectory",
    invalid_option: "errorInvalidOption",
    invalid_or_expired_plan: "errorInvalidOrExpiredPlan",
    plan_already_used: "errorPlanAlreadyUsed",
    plan_changed: "errorPlanChanged",
    no_operations_selected: "errorNoOperationsSelected",
    invalid_operation_selection: "errorInvalidOperationSelection",
    request_too_large: "errorRequestTooLarge",
  };
  return keys[code] ?? "errorGeneric";
}

function setLoading(button, isLoading) {
  button.disabled = isLoading;
  button.classList.toggle("is-loading", isLoading);
  button.setAttribute("aria-busy", String(isLoading));
}

function resetResultState() {
  state.planConsumed = false;
  state.appliedOperationIds = new Set();
  state.failedOperationIds = new Set();
  state.selectedOperationIds = new Set();
  state.selectionInitialized = false;
}

function invalidatePreview() {
  if (!state.payload) {
    updateFlowState();
    return;
  }
  state.payload = null;
  resetResultState();
  elements.resultsBoard.hidden = true;
  elements.emptyResults.hidden = false;
  elements.resultMode.hidden = true;
  elements.operationsBody.replaceChildren();
  elements.skippedList.replaceChildren();
  elements.skippedSection.hidden = true;
  elements.selectAll.checked = false;
  elements.selectAll.indeterminate = false;
  elements.selectAll.disabled = true;
  elements.applyButton.disabled = true;
  setAlert("previewInvalidated", "info");
  updateFlowState();
}

/* Marks the three workflow steps as done / current / upcoming. */
function updateFlowState() {
  const hasDirectory = elements.directory.value.trim().length > 0;
  const hasPayload = Boolean(state.payload);
  const readyToApply =
    hasPayload &&
    state.payload.mode === "live" &&
    !state.planConsumed &&
    state.selectedOperationIds.size > 0;
  const stepStates = [
    hasDirectory || hasPayload ? "done" : "current",
    hasPayload ? "done" : hasDirectory ? "current" : "upcoming",
    state.planConsumed ? "done" : readyToApply ? "current" : "upcoming",
  ];
  elements.flowSteps.forEach((step, index) => {
    step.dataset.state = stepStates[index] ?? "upcoming";
  });
}

/* Eases a summary figure to its new value; skipped when nothing changed. */
function setCount(element, value) {
  const target = Number(value) || 0;
  const previous = Number.parseInt(element.textContent ?? "", 10);
  element.dataset.count = String(target);
  if (!Number.isFinite(previous) || previous === target || prefersReducedMotion()) {
    element.textContent = String(target);
    return;
  }
  const startedAt = performance.now();
  const duration = 460;
  const step = (now) => {
    if (element.dataset.count !== String(target)) {
      return;
    }
    const progress = Math.min(1, (now - startedAt) / duration);
    const eased = 1 - (1 - progress) ** 3;
    element.textContent = String(Math.round(previous + (target - previous) * eased));
    if (progress < 1) {
      requestAnimationFrame(step);
    }
  };
  requestAnimationFrame(step);
}

function stagger(element, index) {
  element.classList.add("stagger");
  element.style.setProperty("--i", String(Math.min(index, 10)));
}

function formatMatch(operation) {
  if (operation.match_type === "episode") {
    return translate("episodeMatch", { detail: operation.detail });
  }
  const score = Math.round((operation.score ?? Number(operation.detail) ?? 0) * 100);
  return translate("fuzzyMatch", { score });
}

function formatSkipReason(item) {
  const keys = {
    unmatched: "skipUnmatched",
    already_matching: "skipAlreadyMatching",
    strict_collision: "skipStrictCollision",
    no_video: "skipNoVideo",
    collision: "skipCollision",
  };
  return translate(keys[item.reason_code] ?? "skipCollision");
}

function appendCell(row, className, content) {
  const cell = document.createElement("td");
  if (className) {
    cell.className = className;
  }
  if (content instanceof Node) {
    cell.append(content);
  } else {
    cell.textContent = content;
  }
  row.append(cell);
  return cell;
}

function renderOperations(payload, animate) {
  elements.operationsBody.replaceChildren();
  const isEditable = payload.mode === "live" && !state.planConsumed;
  if (isEditable && !state.selectionInitialized) {
    state.selectedOperationIds = new Set(payload.operations.map((operation) => operation.id));
    state.selectionInitialized = true;
  }
  if (payload.operations.length === 0) {
    const row = document.createElement("tr");
    const cell = document.createElement("td");
    cell.colSpan = 4;
    cell.className = "empty-table-cell";
    cell.textContent = translate("noOperations");
    row.append(cell);
    elements.operationsBody.append(row);
    elements.selectAll.checked = false;
    elements.selectAll.indeterminate = false;
    elements.selectAll.disabled = true;
    elements.applyButton.disabled = true;
    return;
  }

  payload.operations.forEach((operation, index) => {
    const row = document.createElement("tr");
    if (animate) {
      stagger(row, index);
    }
    const checkbox = document.createElement("input");
    const wasApplied = state.appliedOperationIds.has(operation.id);
    const failed = state.failedOperationIds.has(operation.id);
    checkbox.type = "checkbox";
    checkbox.className = "operation-checkbox";
    checkbox.value = operation.id;
    checkbox.checked = isEditable && state.selectedOperationIds.has(operation.id);
    checkbox.disabled = !isEditable;
    checkbox.setAttribute("aria-label", operation.source);
    checkbox.addEventListener("change", updateSelectionState);
    appendCell(row, "", checkbox);

    const source = document.createElement("span");
    source.className = "file-name";
    source.title = operation.source;
    source.textContent = operation.source;
    appendCell(row, "", source);

    const target = document.createElement("span");
    target.className = "file-name target";
    target.title = operation.target;
    target.textContent = operation.target;
    appendCell(row, "", target);

    const badge = document.createElement("span");
    badge.className = `match-badge ${operation.match_type}`;
    badge.textContent = wasApplied
      ? translate("operationApplied")
      : failed
        ? translate("operationNotApplied")
        : formatMatch(operation);
    appendCell(row, "", badge);
    elements.operationsBody.append(row);
  });
  updateSelectionState();
}

function renderSkipped(payload, animate) {
  elements.skippedList.replaceChildren();
  elements.skippedSection.hidden = payload.skipped.length === 0;
  payload.skipped.forEach((item, index) => {
    const entry = document.createElement("li");
    if (animate) {
      stagger(entry, index);
    }
    const source = document.createElement("span");
    source.className = "skip-file";
    source.title = item.source;
    source.textContent = item.source;
    const reason = document.createElement("span");
    reason.className = "skip-reason";
    reason.textContent = formatSkipReason(item);
    entry.append(source, reason);
    elements.skippedList.append(entry);
  });
}

function renderResults(payload, animate = false) {
  elements.emptyResults.hidden = true;
  elements.resultsBoard.hidden = false;
  elements.resultMode.hidden = false;
  elements.resultMode.className = `mode-badge ${payload.mode === "demo" ? "demo" : ""}`;
  const modeKey =
    payload.mode === "demo" ? "demoMode" : payload.summary.matched > 0 ? "liveMode" : "previewMode";
  elements.resultMode.textContent = translate(modeKey);
  setCount(elements.summaryVideos, payload.summary.videos);
  setCount(elements.summarySubtitles, payload.summary.subtitles);
  setCount(elements.summaryMatched, payload.summary.matched);
  setCount(elements.summarySkipped, payload.summary.skipped);
  renderOperations(payload, animate);
  renderSkipped(payload, animate);
  updateFlowState();
}

function updateSelectionState() {
  const checkboxes = [...document.querySelectorAll(".operation-checkbox")];
  const enabledCheckboxes = checkboxes.filter((checkbox) => !checkbox.disabled);
  const selectedCount = enabledCheckboxes.filter((checkbox) => checkbox.checked).length;
  if (state.payload?.mode === "live" && !state.planConsumed) {
    state.selectedOperationIds = new Set(
      enabledCheckboxes.filter((checkbox) => checkbox.checked).map((checkbox) => checkbox.value),
    );
    state.selectionInitialized = true;
  }
  elements.selectAll.disabled = enabledCheckboxes.length === 0;
  elements.selectAll.checked = enabledCheckboxes.length > 0 && selectedCount === enabledCheckboxes.length;
  elements.selectAll.indeterminate = selectedCount > 0 && selectedCount < enabledCheckboxes.length;
  elements.applyButton.disabled = selectedCount === 0 || !state.payload || state.payload.mode !== "live";
  updateFlowState();
}

function selectedOperationIds() {
  return [...document.querySelectorAll(".operation-checkbox:checked")].map((checkbox) => checkbox.value);
}

function selectedMatchLevelScore() {
  const selectedLevel = [...elements.matchLevels].find((input) => input.checked)?.value;
  return MATCH_LEVEL_SCORES[selectedLevel] ?? MATCH_LEVEL_SCORES.balanced;
}

async function previewDirectory(event) {
  event.preventDefault();
  const directory = elements.directory.value.trim();
  if (!directory) {
    setAlert("errorInvalidDirectory", "error");
    elements.directory.focus();
    return;
  }

  clearAlert();
  setLoading(elements.previewButton, true);
  try {
    const payload = await requestJson("/api/preview", {
      directory,
      recursive: elements.recursive.checked,
      strict: elements.strict.checked,
      min_score: selectedMatchLevelScore(),
    });
    state.payload = payload;
    resetResultState();
    renderResults(payload, true);
    if (payload.summary.videos === 0) {
      setAlert("noVideoFiles", "error");
    } else if (payload.summary.subtitles === 0) {
      setAlert("noSubtitleFiles", "error");
    } else if (payload.summary.matched === 0) {
      setAlert("noOperations", "info");
    } else if (payload.summary.skipped > 0) {
      setAlert("previewNeedsReview", "info");
    } else {
      setAlert("previewReady", "success");
    }
  } catch (error) {
    invalidatePreview();
    setAlert(errorKey(error.message), "error");
  } finally {
    setLoading(elements.previewButton, false);
  }
}

async function loadDemo() {
  clearAlert();
  setLoading(elements.demoButton, true);
  try {
    const payload = await getJson("/api/demo");
    state.payload = payload;
    resetResultState();
    renderResults(payload, true);
    setAlert("demoBanner", "info");
    document.querySelector("#workspace").scrollIntoView({ behavior: "smooth", block: "start" });
  } catch (error) {
    setAlert(errorKey(error.message), "error");
  } finally {
    setLoading(elements.demoButton, false);
  }
}

async function chooseDirectory() {
  setAlert("openingFolderPicker", "info");
  setLoading(elements.chooseDirectoryButton, true);
  try {
    const payload = await requestJson("/api/choose-directory", {});
    if (payload.path) {
      elements.directory.value = payload.path;
      invalidatePreview();
      setAlert("folderSelected", "success");
      elements.directory.focus();
    } else if (payload.available) {
      setAlert("folderPickerCancelled", "info");
    } else {
      setAlert("folderPickerUnavailable", "info");
    }
  } catch (error) {
    setAlert(errorKey(error.message), "error");
  } finally {
    setLoading(elements.chooseDirectoryButton, false);
  }
}

function openConfirmDialog() {
  const selectedIds = selectedOperationIds();
  if (!state.payload || state.payload.mode !== "live" || state.planConsumed || selectedIds.length === 0) {
    setAlert("selectAtLeastOne", "error");
    return;
  }
  elements.confirmCopy.textContent = translate("confirmCount", { count: selectedIds.length });
  elements.confirmDialog.showModal();
}

async function applyPlan() {
  const selectedIds = selectedOperationIds();
  if (!state.payload || state.payload.mode !== "live" || state.planConsumed || selectedIds.length === 0) {
    setAlert("selectAtLeastOne", "error");
    return;
  }

  state.applying = true;
  setLoading(elements.confirmApplyButton, true);
  elements.cancelApplyButton.disabled = true;
  try {
    const result = await requestJson("/api/apply", {
      plan_id: state.payload.plan_id,
      operation_ids: selectedIds,
    });
    state.planConsumed = true;
    state.appliedOperationIds = new Set(result.applied.map((item) => item.id));
    state.failedOperationIds = new Set(result.failures.map((item) => item.id));
    elements.confirmDialog.close();
    renderResults(state.payload);
    if (result.failures.length === 0) {
      setAlert("appliedSuccess", "success", { count: result.applied.length });
    } else {
      setAlert("appliedPartial", "error", { count: result.applied.length });
    }
  } catch (error) {
    elements.confirmDialog.close();
    setAlert(errorKey(error.message), "error");
  } finally {
    state.applying = false;
    elements.cancelApplyButton.disabled = false;
    setLoading(elements.confirmApplyButton, false);
  }
}

let serverShutdownRequested = false;

function requestServerShutdown() {
  if (serverShutdownRequested) {
    return;
  }
  serverShutdownRequested = true;
  if (typeof navigator.sendBeacon === "function" && navigator.sendBeacon("/api/shutdown")) {
    return;
  }
  fetch("/api/shutdown", { method: "POST", keepalive: true }).catch(() => {});
}

function bindEvents() {
  elements.themeToggle.addEventListener("click", () => {
    setTheme(state.theme === "dark" ? "light" : "dark");
  });
  elements.languageToggle.addEventListener("click", () => {
    setLanguage(state.language === "zh" ? "en" : "zh");
  });
  elements.previewForm.addEventListener("submit", previewDirectory);
  elements.demoButton.addEventListener("click", loadDemo);
  elements.chooseDirectoryButton.addEventListener("click", chooseDirectory);
  elements.directory.addEventListener("input", invalidatePreview);
  [elements.recursive, elements.strict, ...elements.matchLevels].forEach((input) => {
    input.addEventListener("change", invalidatePreview);
  });
  elements.selectAll.addEventListener("change", () => {
    document.querySelectorAll(".operation-checkbox:not(:disabled)").forEach((checkbox) => {
      checkbox.checked = elements.selectAll.checked;
    });
    updateSelectionState();
  });
  elements.applyButton.addEventListener("click", openConfirmDialog);
  elements.cancelApplyButton.addEventListener("click", () => elements.confirmDialog.close());
  elements.confirmApplyButton.addEventListener("click", applyPlan);
  elements.confirmDialog.addEventListener("click", (event) => {
    if (event.target === elements.confirmDialog && !state.applying) {
      elements.confirmDialog.close();
    }
  });
  /* Escape must not look like a cancellation while the rename is already running. */
  elements.confirmDialog.addEventListener("cancel", (event) => {
    if (state.applying) {
      event.preventDefault();
    }
  });
  window.addEventListener("pagehide", (event) => {
    if (!event.persisted) {
      requestServerShutdown();
    }
  });
}

applyTheme(state.theme, false);
applyLanguage(state.language);
updateFlowState();
bindEvents();
